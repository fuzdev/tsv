/**
 * Corpus comparison tool - compares our formatting against Prettier's on arbitrary codebases.
 *
 * Usage:
 *   deno task corpus:compare ~/dev/some-project
 *   deno task corpus:compare ~/dev/some-project --filter svelte
 *   deno task corpus:compare ~/dev/some-project --limit 100
 *   deno task corpus:compare ~/dev/some-project --verbose
 *   deno task corpus:compare ~/dev/some-project --diff
 */

import { parseArgs } from '@std/cli/parse-args';
import { DirectoryLoader, groupByLanguage } from './lib/corpus.ts';
import { CanonicalImplementation } from './lib/canonical.ts';
import { diffLines, filterDiffContext, formatDiffForTerminal } from './lib/diff.ts';
import { NativeImplementation } from './lib/ffi.ts';
import { type Language, LANGUAGES, type SourceFile } from './lib/types.ts';
import { loadAllVersions } from './lib/versions.ts';

interface Args {
	_: string[];
	filter?: string;
	limit?: number;
	verbose?: boolean;
	diff?: boolean;
	'diff-limit'?: number;
	'exit-on-first'?: boolean;
	'include-divergences'?: boolean;
	help?: boolean;
}

interface LanguageStats {
	total: number;
	match: number;
	differ: number;
	errors: number;
	skipped: number;
}

interface CompareResult {
	file: SourceFile;
	status: 'match' | 'differ' | 'error' | 'skipped';
	error?: string;
	ours?: string;
	prettier?: string;
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

/** Path to skip list file (relative paths, one per line) */
const SKIP_LIST_PATH = new URL('./skip_divergences.txt', import.meta.url).pathname;

/** Load skip list from file (relative paths, one per line) */
async function loadSkipList(): Promise<Set<string>> {
	try {
		const content = await Deno.readTextFile(SKIP_LIST_PATH);
		const paths = content
			.split('\n')
			.map((line) => line.trim())
			.filter((line) => line && !line.startsWith('#'));
		return new Set(paths);
	} catch {
		return new Set();
	}
}

function printUsage(): void {
	console.log(`
Usage: deno task corpus:compare <path> [options]

Arguments:
  path              Directory to scan for source files

Options:
  --filter <lang>   Only compare files of this language (svelte, typescript, css)
  --limit <n>       Limit to first n files per language
  --verbose         Show each file as it's processed
  --diff            Show unified diffs for mismatches
  --diff-limit <n>  Max diffs to show (default: 5)
  --exit-on-first   Stop after finding the first mismatch or error
  --include-divergences  Include files in skip_divergences.txt (default: skipped)
  --help            Show this help message

Examples:
  deno task corpus:compare ~/dev/my-project
  deno task corpus:compare ~/dev/my-project --filter svelte
  deno task corpus:compare ~/dev/my-project --limit 50 --verbose
  deno task corpus:compare ~/dev/my-project --diff --diff-limit 3
  deno task corpus:compare ~/dev/my-project --exit-on-first --diff
  deno task corpus:compare ~/dev/my-project --include-divergences
`);
}

async function main(): Promise<void> {
	const args = parseArgs(Deno.args, {
		string: ['filter'],
		boolean: ['verbose', 'help', 'diff', 'exit-on-first', 'include-divergences'],
		alias: { h: 'help', v: 'verbose', f: 'filter', l: 'limit', d: 'diff' },
	}) as Args;

	if (args.help) {
		printUsage();
		return;
	}

	const path = args._[0]?.toString();
	if (!path) {
		console.error('Error: No path provided\n');
		printUsage();
		Deno.exit(1);
	}

	// Resolve path (handle ~)
	const resolvedPath = path.startsWith('~') ? path.replace('~', Deno.env.get('HOME') || '') : path;

	// Validate filter
	const filterLang = args.filter as Language | undefined;
	if (filterLang && !LANGUAGES.includes(filterLang)) {
		console.error(`Error: Invalid filter "${filterLang}". Must be one of: ${LANGUAGES.join(', ')}`);
		Deno.exit(1);
	}

	const limit = args.limit ? Number(args.limit) : undefined;
	const verbose = args.verbose ?? false;
	const showDiff = args.diff ?? false;
	const diffLimit = args['diff-limit'] ? Number(args['diff-limit']) : 5;
	const exitOnFirst = args['exit-on-first'] ?? false;
	const includeDivergences = args['include-divergences'] ?? false;

	// Load skip list by default (unless --include-divergences)
	const skipList = includeDivergences ? new Set<string>() : await loadSkipList();

	console.log(`Comparing: ${resolvedPath}`);
	if (filterLang) console.log(`Filter: ${filterLang} only`);
	if (limit) console.log(`Limit: ${limit} files per language`);
	console.log();

	// Load corpus
	const loader = new DirectoryLoader({ path: resolvedPath });
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
		stats.set(lang, { total: 0, match: 0, differ: 0, errors: 0, skipped: 0 });
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

				if (ours === prettier) {
					langStats.match++;
					langResults.push({ file, status: 'match' });
				} else {
					const rel = relPath(file.path, resolvedPath);
					if (skipList.has(rel)) {
						langStats.skipped++;
						langResults.push({ file, status: 'skipped', ours, prettier });
					} else {
						langStats.differ++;
						langResults.push({ file, status: 'differ', ours, prettier });
						if (exitOnFirst) {
							console.log(`\nFirst mismatch: ${rel}`);
							if (showDiff) {
								console.log('─'.repeat(70));
								const diff = diffLines(prettier, ours);
								const removals = diff.filter((d) => d.type === 'remove').length;
								const additions = diff.filter((d) => d.type === 'add').length;
								console.log(
									`Diff: \x1b[31m- Prettier\x1b[0m → \x1b[32m+ Ours\x1b[0m  (${removals} prettier-only, ${additions} ours-only)`,
								);
								console.log('');
								for (const line of formatDiffForTerminal(filterDiffContext(diff))) {
									console.log(line);
								}
							}
							shouldExit = true;
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
					console.log(`\nFirst error: ${relPath(file.path, resolvedPath)}`);
					console.log(`  ${errorMsg}`);
					shouldExit = true;
				}
			}

			if (shouldExit) {
				canonical.dispose();
				native.dispose();
				return;
			}
		}
	}

	// Print results
	console.log('Results:');

	let totalMatch = 0;
	let totalDiffer = 0;
	let totalErrors = 0;
	let totalSkipped = 0;
	let totalCount = 0;

	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;

		totalMatch += s.match;
		totalDiffer += s.differ;
		totalErrors += s.errors;
		totalSkipped += s.skipped;
		totalCount += s.total;

		const pct = ((s.match / s.total) * 100).toFixed(1);
		const matchStr = `${s.match}/${s.total} match (${pct}%)`.padEnd(24);
		const differStr = `${s.differ} differ`.padEnd(12);
		const skippedStr = s.skipped > 0 ? ` | ${s.skipped} skipped` : '';
		const errorStr = `${s.errors} errors`;

		console.log(`  ${lang.padEnd(12)} ${matchStr} | ${differStr} | ${errorStr}${skippedStr}`);
	}

	if (totalCount > 0) {
		console.log('  ' + '─'.repeat(68));
		const pct = ((totalMatch / totalCount) * 100).toFixed(1);
		const matchStr = `${totalMatch}/${totalCount} match (${pct}%)`.padEnd(24);
		const differStr = `${totalDiffer} differ`.padEnd(12);
		const skippedStr = totalSkipped > 0 ? ` | ${totalSkipped} skipped` : '';
		const errorStr = `${totalErrors} errors`;
		console.log(`  ${'total'.padEnd(12)} ${matchStr} | ${differStr} | ${errorStr}${skippedStr}`);
	}

	// Show first N mismatches (sorted by file size, smallest first for easier debugging)
	const allDiffers = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'differ')
	).sort((a, b) => a.file.bytes - b.file.bytes);

	if (allDiffers.length > 0) {
		if (showDiff) {
			// Show diffs for mismatches (smallest files first)
			const toShow = allDiffers.slice(0, diffLimit);
			for (const r of toShow) {
				console.log(`\n${'═'.repeat(70)}`);
				console.log(`File: ${relPath(r.file.path, resolvedPath)} (${formatBytes(r.file.bytes)})`);
				console.log('─'.repeat(70));

				// Show diff with clear labels (matches tsv_debug compare format)
				const diff = diffLines(r.prettier!, r.ours!);
				const removals = diff.filter((d) => d.type === 'remove').length;
				const additions = diff.filter((d) => d.type === 'add').length;
				console.log(
					`Diff: \x1b[31m- Prettier\x1b[0m → \x1b[32m+ Ours\x1b[0m  (${removals} prettier-only, ${additions} ours-only)`,
				);
				console.log('');
				for (const line of formatDiffForTerminal(filterDiffContext(diff))) {
					console.log(line);
				}
			}
			if (allDiffers.length > diffLimit) {
				console.log(`\n... and ${allDiffers.length - diffLimit} more mismatches`);
			}
		} else {
			console.log(`\nFirst ${Math.min(5, allDiffers.length)} mismatches (smallest first):`);
			for (const r of allDiffers.slice(0, 5)) {
				const sizeStr = formatBytes(r.file.bytes);
				console.log(`  ${relPath(r.file.path, resolvedPath)} (${sizeStr})`);
			}
			if (allDiffers.length > 5) {
				console.log(`  ... and ${allDiffers.length - 5} more`);
			}
		}
	}

	// Show first N errors (sorted by file size, smallest first)
	const allErrors = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'error')
	).sort((a, b) => a.file.bytes - b.file.bytes);

	if (allErrors.length > 0) {
		console.log(`\nFirst ${Math.min(3, allErrors.length)} errors (smallest first):`);
		for (const r of allErrors.slice(0, 3)) {
			const sizeStr = formatBytes(r.file.bytes);
			console.log(`  ${relPath(r.file.path, resolvedPath)} (${sizeStr}): ${r.error?.slice(0, 80)}`);
		}
		if (allErrors.length > 3) {
			console.log(`  ... and ${allErrors.length - 3} more`);
		}
	}

	// Show skipped files (known divergences)
	if (totalSkipped > 0) {
		const allSkipped = LANGUAGES.flatMap((lang) =>
			results.get(lang)!.filter((r) => r.status === 'skipped')
		);
		console.log(`\nSkipped (known divergences): ${allSkipped.length} files`);
		for (const r of allSkipped.slice(0, 5)) {
			console.log(`  ${relPath(r.file.path, resolvedPath)}`);
		}
		if (allSkipped.length > 5) {
			console.log(`  ... and ${allSkipped.length - 5} more`);
		}
	}

	// Cleanup
	canonical.dispose();
	native.dispose();
}

main();
