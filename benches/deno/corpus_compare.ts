/**
 * Corpus comparison tool - compares our formatting against Prettier's on arbitrary codebases.
 *
 * Usage:
 *   deno task corpus:compare ~/dev/some-project
 *   deno task corpus:compare ~/dev/some-project --filter svelte
 *   deno task corpus:compare ~/dev/some-project --limit 100
 *   deno task corpus:compare ~/dev/some-project --verbose
 *   deno task corpus:compare ~/dev/some-project --safety-only
 *   deno task corpus:compare ~/dev/some-project --explain
 *   deno task corpus:compare ~/dev/some-project --summary   # Compact output (no diffs)
 *   deno task corpus:compare --all                          # All SvelteKit repos from ~/dev
 */

import { parseArgs } from '@std/cli/parse-args';
import { DevReposLoader, DirectoryLoader, groupByLanguage } from './lib/corpus.ts';
import { CanonicalImplementation } from './lib/canonical.ts';
import {
	type DiffHunk,
	diffLines,
	extractHunks,
	filterDiffContext,
	formatDiffForTerminal,
} from './lib/diff.ts';
import { NativeImplementation } from './lib/ffi.ts';
import { type Language, LANGUAGES, type SourceFile } from './lib/types.ts';
import { loadAllVersions } from './lib/versions.ts';
import {
	checkSafety,
	detectDivergences,
	type HunkCoverageResult,
	type SafetyViolation,
} from './lib/divergence/mod.ts';

interface Args {
	_: string[];
	all?: boolean;
	filter?: string;
	limit?: number;
	verbose?: boolean;
	'exit-on-first'?: boolean;
	'safety-only'?: boolean;
	explain?: boolean;
	strict?: boolean;
	'audit-patterns'?: boolean;
	summary?: boolean;
	help?: boolean;
}

interface LanguageStats {
	total: number;
	match: number;
	knownDivergence: number;
	partialDivergence: number;
	unknownDiff: number;
	safetyViolation: number;
	errors: number;
}

interface CompareResult {
	file: SourceFile;
	status:
		| 'match'
		| 'known_divergence'
		| 'partial_divergence'
		| 'unknown_diff'
		| 'safety_violation'
		| 'error';
	error?: string;
	ours?: string;
	prettier?: string;
	coverage?: HunkCoverageResult;
	safetyViolations?: SafetyViolation[];
}

/** Get relative path from base directory */
function relPath(filePath: string, basePath: string): string {
	return filePath.startsWith(basePath + '/') ? filePath.slice(basePath.length + 1) : filePath;
}

/** Format bytes as human-readable string */
function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes}B`;
	const kb = bytes / 1024;
	if (kb < 10) return `${kb.toFixed(1)}KB`;
	return `${Math.round(kb)}KB`;
}

/** Get a brief diff summary for unknown differences (for agent comprehension) */
function getDiffSummary(prettier: string, ours: string): string {
	const diff = diffLines(prettier, ours);
	const removals = diff.filter((d) => d.type === 'remove');
	const additions = diff.filter((d) => d.type === 'add');

	// Check for blank line differences
	const blankRemovals = removals.filter((d) => !d.line.trim()).length;
	const blankAdditions = additions.filter((d) => !d.line.trim()).length;

	// Check for line count difference (we break more/less)
	const prettierLineCount = prettier.split('\n').length;
	const oursLineCount = ours.split('\n').length;
	const lineDiff = oursLineCount - prettierLineCount;

	// Find the first meaningful (non-empty) change
	const firstRemoval = removals.find((d) => d.line.trim())?.line.trim();
	const firstAddition = additions.find((d) => d.line.trim())?.line.trim();

	// Describe the difference
	if (lineDiff !== 0 && firstRemoval && firstAddition) {
		// Line count changed - likely a breaking difference
		const direction = lineDiff > 0 ? 'we break' : 'prettier breaks';
		const snippet = firstRemoval.slice(0, 40);
		return `${direction} (+${Math.abs(lineDiff)} lines): "${snippet}..."`;
	} else if (firstRemoval && firstAddition) {
		// Same line count, content differs
		const r = firstRemoval.slice(0, 35);
		const a = firstAddition.slice(0, 35);
		return `"${r}..." → "${a}..."`;
	} else if (blankRemovals !== blankAdditions) {
		// Only blank line differences
		if (blankAdditions > blankRemovals) {
			return `prettier adds ${blankAdditions - blankRemovals} blank line(s)`;
		} else {
			return `ours adds ${blankRemovals - blankAdditions} blank line(s)`;
		}
	} else if (firstRemoval) {
		return `prettier has: "${firstRemoval.slice(0, 50)}"`;
	} else if (firstAddition) {
		return `ours has: "${firstAddition.slice(0, 50)}"`;
	}
	return `${removals.length} line(s) differ`;
}

function printUsage(): void {
	console.log(`
Usage: deno task corpus:compare <path> [options]
       deno task corpus:compare --all [options]

Arguments:
  path              Directory to scan for source files
  --all             Compare all SvelteKit repos from ~/dev (from web.code-workspace)

Options:
  --filter <lang>   Only compare files of this language (svelte, typescript, css)
  --limit <n>       Limit to first n files per language
  --verbose         Show each file as it's processed
  --exit-on-first   Stop after finding the first mismatch or error (shows diff)
  --safety-only     Only check for safety violations (data loss), skip formatting comparison
  --explain         Show detected divergence patterns for each difference
  --summary         Compact output (no diffs, just file lists with brief descriptions)
  --strict          Fail on any difference (disable divergence detection)
  --audit-patterns  Show per-pattern corpus coverage with sample diffs for spot-checking
  --help            Show this help message

Examples:
  deno task corpus:compare ~/dev/my-project
  deno task corpus:compare ~/dev/my-project --filter svelte
  deno task corpus:compare ~/dev/my-project --limit 50 --verbose
  deno task corpus:compare ~/dev/my-project --exit-on-first
  deno task corpus:compare ~/dev/my-project --safety-only
  deno task corpus:compare ~/dev/my-project --explain
  deno task corpus:compare --all --audit-patterns
  deno task corpus:compare --all --summary
`);
}

async function main(): Promise<void> {
	const args = parseArgs(Deno.args, {
		string: ['filter'],
		boolean: [
			'all',
			'verbose',
			'help',
			'exit-on-first',
			'safety-only',
			'explain',
			'strict',
			'audit-patterns',
			'summary',
		],
		alias: { h: 'help', v: 'verbose', f: 'filter', l: 'limit', a: 'all' },
	}) as Args;

	if (args.help) {
		printUsage();
		return;
	}

	const useAllRepos = args.all ?? false;
	const path = args._[0]?.toString();

	if (!path && !useAllRepos) {
		console.error('Error: No path provided (use --all for all repos)\n');
		printUsage();
		Deno.exit(1);
	}

	// Resolve path (handle ~)
	const resolvedPath = path
		? path.startsWith('~') ? path.replace('~', Deno.env.get('HOME') || '') : path
		: '~/dev';

	// Validate filter
	const filterLang = args.filter as Language | undefined;
	if (filterLang && !LANGUAGES.includes(filterLang)) {
		console.error(`Error: Invalid filter "${filterLang}". Must be one of: ${LANGUAGES.join(', ')}`);
		Deno.exit(1);
	}

	const limit = args.limit ? Number(args.limit) : undefined;
	const verbose = args.verbose ?? false;
	const exitOnFirst = args['exit-on-first'] ?? false;
	const safetyOnly = args['safety-only'] ?? false;
	const explain = args.explain ?? false;
	const strict = args.strict ?? false;
	const auditPatterns = args['audit-patterns'] ?? false;
	const summary = args.summary ?? false;

	if (useAllRepos) {
		console.log('Comparing: All SvelteKit repos from ~/dev');
	} else {
		console.log(`Comparing: ${resolvedPath}`);
	}
	if (filterLang) console.log(`Filter: ${filterLang} only`);
	if (limit) console.log(`Limit: ${limit} files per language`);
	if (safetyOnly) console.log(`Mode: safety-only (checking for data loss)`);
	if (strict) console.log(`Mode: strict (no divergence detection)`);
	if (explain) console.log(`Mode: explain (show divergence patterns)`);
	if (auditPatterns) console.log(`Mode: audit-patterns (per-pattern coverage report)`);
	if (summary) console.log(`Mode: summary (compact output, no diffs)`);
	console.log();

	// Load corpus
	const loader = useAllRepos ? new DevReposLoader() : new DirectoryLoader({ path: resolvedPath });
	const { files } = await loader.load(verbose ? console.log : () => {});

	if (files.length === 0) {
		console.log('No files found.');
		return;
	}

	// Group and optionally filter/limit
	let grouped = groupByLanguage(files);
	if (filterLang) {
		grouped = { svelte: [], typescript: [], css: [], [filterLang]: grouped[filterLang] };
	}
	if (limit) {
		for (const lang of LANGUAGES) {
			grouped[lang] = grouped[lang].slice(0, limit);
		}
	}

	const totalFiles = LANGUAGES.reduce((sum, lang) => sum + grouped[lang].length, 0);
	const counts = LANGUAGES.map((lang) => `${grouped[lang].length} ${lang}`).join(', ');
	console.log(`Found: ${totalFiles} files (${counts})\n`);

	// Initialize implementations
	const versions = await loadAllVersions();
	const canonical = new CanonicalImplementation(versions.canonical);
	const native = new NativeImplementation();

	try {
		await canonical.init();
	} catch (e) {
		console.error(`Failed to initialize prettier: ${e}`);
		Deno.exit(1);
	}

	try {
		await native.init();
	} catch (e) {
		console.error(`Failed to initialize native formatter: ${e}`);
		console.error('Run: deno task build:ffi');
		Deno.exit(1);
	}

	// Compare files
	const results: Map<Language, CompareResult[]> = new Map();
	const stats: Map<Language, LanguageStats> = new Map();

	for (const lang of LANGUAGES) {
		results.set(lang, []);
		stats.set(lang, {
			total: 0,
			match: 0,
			knownDivergence: 0,
			partialDivergence: 0,
			unknownDiff: 0,
			safetyViolation: 0,
			errors: 0,
		});
	}

	// Track divergence pattern counts
	const divergenceCounts: Map<string, number> = new Map();

	// Track per-pattern file claims for audit (pattern → file samples with hunk info)
	interface PatternAuditEntry {
		path: string;
		hunkIndices: number[];
		hunkPreview: string; // first hunk's first changed line
	}
	const patternAuditMap: Map<string, PatternAuditEntry[]> = new Map();

	/** Record a pattern match into the audit map */
	function recordAuditEntry(
		patternName: string,
		filePath: string,
		hunkIndices: number[],
		hunks: DiffHunk[],
	): void {
		const entries = patternAuditMap.get(patternName) ?? [];
		const firstHunk = hunks[hunkIndices[0]];
		const preview = (firstHunk?.addedLines[0] || firstHunk?.removedLines[0] || '')
			.trim().slice(0, 60);
		entries.push({
			path: relPath(filePath, resolvedPath),
			hunkIndices,
			hunkPreview: preview,
		});
		patternAuditMap.set(patternName, entries);
	}

	for (const lang of LANGUAGES) {
		const langFiles = grouped[lang];
		const langStats = stats.get(lang)!;
		const langResults = results.get(lang)!;

		for (const file of langFiles) {
			langStats.total++;

			if (verbose) {
				console.log(`  ${file.path}`);
			}

			let shouldExit = false;
			try {
				// Format with both
				const ours = native.format(file.content, lang);
				const prettier = await canonical.formatAsync(file.content, lang);

				// Safety check FIRST (always) - compare source vs OUR output
				const safetyViolations = checkSafety(file.content, ours);
				if (safetyViolations.length > 0) {
					langStats.safetyViolation++;
					langResults.push({
						file,
						status: 'safety_violation',
						ours,
						prettier,
						safetyViolations,
					});
					if (exitOnFirst) {
						const rel = relPath(file.path, resolvedPath);
						console.log(`\nSafety violation: ${rel}`);
						for (const v of safetyViolations) {
							console.log(`  ${v.type}: ${v.summary}`);
						}
						shouldExit = true;
					}
				} else if (safetyOnly) {
					// In safety-only mode, we're done after safety check passes
					langStats.match++;
					langResults.push({ file, status: 'match' });
				} else if (ours === prettier) {
					// Exact match
					langStats.match++;
					langResults.push({ file, status: 'match' });
				} else {
					// Difference detected
					const rel = relPath(file.path, resolvedPath);
					if (strict) {
						// Strict mode: any difference is a failure
						langStats.unknownDiff++;
						langResults.push({ file, status: 'unknown_diff', ours, prettier });
						if (exitOnFirst) {
							console.log(`\nDifference (strict mode): ${rel}`);
							shouldExit = true;
						}
					} else {
						// Detect known divergence patterns (hunk-aware)
						const diff = diffLines(prettier, ours);
						const hunks = extractHunks(diff);
						const coverage = detectDivergences({
							source: file.content,
							ours,
							prettier,
							diff,
							hunks,
							language: lang,
						});

						if (coverage.classification === 'all_explained') {
							// All hunks explained by known patterns
							langStats.knownDivergence++;
							langResults.push({
								file,
								status: 'known_divergence',
								ours,
								prettier,
								coverage,
							});
							for (const d of coverage.matches) {
								divergenceCounts.set(d.pattern, (divergenceCounts.get(d.pattern) || 0) + 1);
								if (auditPatterns) {
									recordAuditEntry(d.pattern, file.path, d.hunkIndices, hunks);
								}
							}
						} else if (coverage.classification === 'partial') {
							// Some hunks explained, some not
							langStats.partialDivergence++;
							langResults.push({
								file,
								status: 'partial_divergence',
								ours,
								prettier,
								coverage,
							});
							for (const d of coverage.matches) {
								divergenceCounts.set(d.pattern, (divergenceCounts.get(d.pattern) || 0) + 1);
								if (auditPatterns) {
									recordAuditEntry(d.pattern, file.path, d.hunkIndices, hunks);
								}
							}
						} else {
							// No hunks explained - unknown difference
							langStats.unknownDiff++;
							langResults.push({ file, status: 'unknown_diff', ours, prettier, coverage });
							if (exitOnFirst) {
								console.log(`\nUnknown difference: ${rel}`);
								console.log('─'.repeat(70));
								const removals = diff.filter((d) => d.type === 'remove').length;
								const additions = diff.filter((d) => d.type === 'add').length;
								console.log(
									`Diff: \x1b[31m- Prettier\x1b[0m → \x1b[32m+ Ours\x1b[0m  (${removals} prettier-only, ${additions} ours-only)`,
								);
								console.log('');
								for (const line of formatDiffForTerminal(filterDiffContext(diff))) {
									console.log(line);
								}
								shouldExit = true;
							}
						}
					}
				}
			} catch (e) {
				langStats.errors++;
				const errorMsg = e instanceof Error ? e.message : String(e);
				langResults.push({
					file,
					status: 'error',
					error: errorMsg,
				});
				if (exitOnFirst) {
					console.log(`\nError: ${relPath(file.path, resolvedPath)}`);
					console.log(`  ${errorMsg}`);
					shouldExit = true;
				}
			}

			if (shouldExit) {
				canonical.dispose();
				native.dispose();
				Deno.exit(1);
			}
		}
	}

	// Print results
	console.log('Results:');

	let totalMatch = 0;
	let totalKnownDivergence = 0;
	let totalPartialDivergence = 0;
	let totalUnknownDiff = 0;
	let totalSafetyViolation = 0;
	let totalErrors = 0;
	let totalCount = 0;

	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;

		totalMatch += s.match;
		totalKnownDivergence += s.knownDivergence;
		totalPartialDivergence += s.partialDivergence;
		totalUnknownDiff += s.unknownDiff;
		totalSafetyViolation += s.safetyViolation;
		totalErrors += s.errors;
		totalCount += s.total;

		const pct = s.total > 0 ? ((s.match / s.total) * 100).toFixed(1) : '100.0';
		const matchStr = `${s.match}/${s.total} match (${pct}%)`.padEnd(24);

		const parts: string[] = [];
		if (s.knownDivergence > 0) parts.push(`${s.knownDivergence} known`);
		if (s.partialDivergence > 0) parts.push(`\x1b[33m${s.partialDivergence} partial\x1b[0m`);
		if (s.unknownDiff > 0) parts.push(`${s.unknownDiff} unknown`);
		if (s.safetyViolation > 0) parts.push(`\x1b[31m${s.safetyViolation} SAFETY\x1b[0m`);
		if (s.errors > 0) parts.push(`${s.errors} errors`);

		const detailStr = parts.length > 0 ? parts.join(' | ') : 'all match';
		console.log(`  ${lang.padEnd(12)} ${matchStr} | ${detailStr}`);
	}

	if (totalCount > 0) {
		console.log('  ' + '─'.repeat(72));
		const pct = totalCount > 0 ? ((totalMatch / totalCount) * 100).toFixed(1) : '100.0';
		const matchStr = `${totalMatch}/${totalCount} match (${pct}%)`.padEnd(24);

		const parts: string[] = [];
		if (totalKnownDivergence > 0) parts.push(`${totalKnownDivergence} known`);
		if (totalPartialDivergence > 0) {
			parts.push(`\x1b[33m${totalPartialDivergence} partial\x1b[0m`);
		}
		if (totalUnknownDiff > 0) parts.push(`${totalUnknownDiff} unknown`);
		if (totalSafetyViolation > 0) parts.push(`\x1b[31m${totalSafetyViolation} SAFETY\x1b[0m`);
		if (totalErrors > 0) parts.push(`${totalErrors} errors`);

		const detailStr = parts.length > 0 ? parts.join(' | ') : 'all match';
		console.log(`  ${'total'.padEnd(12)} ${matchStr} | ${detailStr}`);
	}

	// Show divergence pattern breakdown if any detected
	if (divergenceCounts.size > 0 && (explain || verbose)) {
		console.log('\nKnown Divergence Patterns:');
		const sorted = [...divergenceCounts.entries()].sort((a, b) => b[1] - a[1]);
		for (const [pattern, count] of sorted) {
			console.log(`  ${pattern}: ${count} files`);
		}
	}

	// Show per-pattern audit report with sample diffs
	if (auditPatterns && patternAuditMap.size > 0) {
		console.log('\nPattern Audit (per-pattern corpus coverage)');
		console.log('─'.repeat(70));

		const sorted = [...patternAuditMap.entries()].sort((a, b) => b[1].length - a[1].length);
		for (const [pattern, entries] of sorted) {
			console.log(`\n${pattern}: ${entries.length} files`);
			const samples = entries.slice(0, 3);
			for (const sample of samples) {
				const hunkStr = sample.hunkIndices.length === 1
					? `hunk ${sample.hunkIndices[0]}`
					: `hunks ${sample.hunkIndices.join(',')}`;
				console.log(`  ${sample.path} (${hunkStr})`);
				if (sample.hunkPreview) {
					console.log(`    "${sample.hunkPreview}"`);
				}
			}
			if (entries.length > 3) {
				console.log(`  ... and ${entries.length - 3} more`);
			}
		}
	}

	// Show safety violations (CRITICAL)
	const allSafetyViolations = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'safety_violation')
	);

	if (allSafetyViolations.length > 0) {
		console.log(`\n\x1b[31mSAFETY VIOLATIONS (${allSafetyViolations.length} files):\x1b[0m`);
		for (const r of allSafetyViolations) {
			console.log(`  ${relPath(r.file.path, resolvedPath)}`);
			for (const v of r.safetyViolations!) {
				console.log(`    - ${v.type}: ${v.summary}`);
			}
		}
	}

	// Show partial divergences (some hunks unexplained)
	const allPartial = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'partial_divergence')
	).sort((a, b) => a.file.bytes - b.file.bytes);

	// Show unknown differences (needs investigation)
	const allUnknown = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'unknown_diff')
	).sort((a, b) => a.file.bytes - b.file.bytes);

	// Default: show unexplained diffs (partial hunks + unknown files)
	// --summary: compact output without diffs
	if (summary) {
		// Compact partial divergence listing
		if (allPartial.length > 0) {
			console.log(
				`\nPartial Divergences (${allPartial.length} files):`,
			);
			for (const r of allPartial.slice(0, 10)) {
				const coverage = r.coverage!;
				const patterns = coverage.matches.map((d) => d.pattern).join(', ');
				console.log(
					`  ${
						relPath(r.file.path, resolvedPath)
					}: ${patterns} (${coverage.unexplainedHunks.length} unexplained hunks)`,
				);
			}
			if (allPartial.length > 10) {
				console.log(`  ... and ${allPartial.length - 10} more`);
			}
		}

		// Compact unknown differences listing
		if (allUnknown.length > 0) {
			console.log(
				`\nUnknown Differences (${allUnknown.length} files, needs investigation):`,
			);
			for (const r of allUnknown.slice(0, 10)) {
				const sizeStr = formatBytes(r.file.bytes);
				const diffSummary = getDiffSummary(r.prettier!, r.ours!);
				console.log(`  ${relPath(r.file.path, resolvedPath)} (${sizeStr})`);
				console.log(`    ${diffSummary}`);
			}
			if (allUnknown.length > 10) {
				console.log(`  ... and ${allUnknown.length - 10} more`);
			}
		}
	} else {
		// Default: show all unexplained diffs
		const totalUnexplainedFiles = allPartial.length + allUnknown.length;
		if (totalUnexplainedFiles > 0) {
			console.log(
				`\nUnexplained Differences (${allPartial.length} partial + ${allUnknown.length} unknown = ${totalUnexplainedFiles} files):`,
			);
		}

		// Partial files: show only unexplained hunks with diffs
		if (allPartial.length > 0) {
			console.log(`\n${'─'.repeat(70)}`);
			console.log(`Partial files (${allPartial.length} — unexplained hunks only):`);
			for (const r of allPartial) {
				const coverage = r.coverage!;
				const patterns = coverage.matches.map((d) => d.pattern).join(', ');
				const explainedCount = coverage.explainedHunks.size;
				const totalHunks = coverage.hunks.length;
				console.log(`\n  ${relPath(r.file.path, resolvedPath)}:`);
				console.log(
					`    explained ${explainedCount}/${totalHunks} hunks: ${patterns}`,
				);
				for (const idx of coverage.unexplainedHunks) {
					const hunk = coverage.hunks[idx];
					const oursLabel = hunk.oursRange ? `ours:${hunk.oursRange.start}` : '';
					const prettierLabel = hunk.prettierRange ? `prettier:${hunk.prettierRange.start}` : '';
					console.log(
						`    \x1b[33mhunk ${idx}\x1b[0m: @@ ${oursLabel} / ${prettierLabel} @@`,
					);
					for (const line of formatDiffForTerminal(hunk.lines)) {
						console.log(`      ${line}`);
					}
				}
			}
		}

		// Unknown files: show full diffs
		if (allUnknown.length > 0) {
			console.log(`\n${'─'.repeat(70)}`);
			console.log(`Unknown files (${allUnknown.length} — full diffs):`);
			for (const r of allUnknown) {
				const diff = diffLines(r.prettier!, r.ours!);
				const removals = diff.filter((d) => d.type === 'remove').length;
				const additions = diff.filter((d) => d.type === 'add').length;
				console.log(`\n  ${relPath(r.file.path, resolvedPath)} (${formatBytes(r.file.bytes)}):`);
				console.log(
					`    \x1b[31m-${removals} prettier-only\x1b[0m, \x1b[32m+${additions} ours-only\x1b[0m`,
				);
				for (const line of formatDiffForTerminal(filterDiffContext(diff))) {
					console.log(`      ${line}`);
				}
			}
		}
	}

	// Show known divergences with explanations if --explain
	if (explain) {
		const allKnown = LANGUAGES.flatMap((lang) =>
			results.get(lang)!.filter((r) => r.status === 'known_divergence')
		);

		if (allKnown.length > 0) {
			console.log(`\nKnown Divergences (${allKnown.length} files):`);
			for (const r of allKnown) {
				const patterns = r.coverage!.matches.map((d) => d.pattern).join(', ');
				console.log(`  ${relPath(r.file.path, resolvedPath)}: ${patterns}`);
			}
		}
	}

	// Show errors
	const allErrors = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'error')
	).sort((a, b) => a.file.bytes - b.file.bytes);

	if (allErrors.length > 0) {
		console.log(`\nErrors (${allErrors.length} files):`);
		for (const r of allErrors.slice(0, 3)) {
			const sizeStr = formatBytes(r.file.bytes);
			console.log(`  ${relPath(r.file.path, resolvedPath)} (${sizeStr}): ${r.error?.slice(0, 80)}`);
		}
		if (allErrors.length > 3) {
			console.log(`  ... and ${allErrors.length - 3} more`);
		}
	}

	// Final status
	console.log();
	if (totalSafetyViolation > 0) {
		console.log(
			`\x1b[31mFAIL: ${totalSafetyViolation} safety violations (data loss detected)\x1b[0m`,
		);
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	} else if ((totalUnknownDiff > 0 || totalPartialDivergence > 0) && strict) {
		const issues = totalUnknownDiff + totalPartialDivergence;
		console.log(`\x1b[31mFAIL: ${issues} unexplained differences (strict mode)\x1b[0m`);
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	} else if (totalUnknownDiff > 0 || totalPartialDivergence > 0) {
		const parts: string[] = [];
		if (totalUnknownDiff > 0) parts.push(`${totalUnknownDiff} unknown`);
		if (totalPartialDivergence > 0) parts.push(`${totalPartialDivergence} partial`);
		console.log(
			`\x1b[33mWARN: ${parts.join(', ')} differences (may need investigation)\x1b[0m`,
		);
	} else if (totalErrors > 0) {
		console.log(`\x1b[33mWARN: ${totalErrors} errors occurred\x1b[0m`);
	} else {
		console.log('\x1b[32mPASS: No safety violations or unknown differences\x1b[0m');
	}

	// Cleanup
	canonical.dispose();
	native.dispose();
}

main();
