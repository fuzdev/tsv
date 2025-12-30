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
import { diffLines, formatDiffForTerminal } from './lib/diff.ts';
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
	help?: boolean;
}

interface LanguageStats {
	total: number;
	match: number;
	differ: number;
	errors: number;
}

interface CompareResult {
	file: SourceFile;
	status: 'match' | 'differ' | 'error';
	error?: string;
	ours?: string;
	prettier?: string;
}

/** Get relative path from base directory */
function relPath(filePath: string, basePath: string): string {
	return filePath.startsWith(basePath + '/') ? filePath.slice(basePath.length + 1) : filePath;
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
  --help            Show this help message

Examples:
  deno task corpus:compare ~/dev/my-project
  deno task corpus:compare ~/dev/my-project --filter svelte
  deno task corpus:compare ~/dev/my-project --limit 50 --verbose
  deno task corpus:compare ~/dev/my-project --diff --diff-limit 3
`);
}

async function main(): Promise<void> {
	const args = parseArgs(Deno.args, {
		string: ['filter'],
		boolean: ['verbose', 'help', 'diff'],
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
		stats.set(lang, { total: 0, match: 0, differ: 0, errors: 0 });
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

			try {
				// Format with both
				const ours = native.format(file.content, lang);
				const prettier = await canonical.formatAsync(file.content, lang);

				if (ours === prettier) {
					langStats.match++;
					langResults.push({ file, status: 'match' });
				} else {
					langStats.differ++;
					langResults.push({ file, status: 'differ', ours, prettier });
				}
			} catch (e) {
				langStats.errors++;
				langResults.push({
					file,
					status: 'error',
					error: e instanceof Error ? e.message : String(e),
				});
			}
		}
	}

	// Print results
	console.log('Results:');

	let totalMatch = 0;
	let totalDiffer = 0;
	let totalErrors = 0;
	let totalCount = 0;

	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;

		totalMatch += s.match;
		totalDiffer += s.differ;
		totalErrors += s.errors;
		totalCount += s.total;

		const pct = ((s.match / s.total) * 100).toFixed(1);
		const matchStr = `${s.match}/${s.total} match (${pct}%)`.padEnd(24);
		const differStr = `${s.differ} differ`.padEnd(12);
		const errorStr = `${s.errors} errors`;

		console.log(`  ${lang.padEnd(12)} ${matchStr} | ${differStr} | ${errorStr}`);
	}

	if (totalCount > 0) {
		console.log('  ' + '─'.repeat(60));
		const pct = ((totalMatch / totalCount) * 100).toFixed(1);
		const matchStr = `${totalMatch}/${totalCount} match (${pct}%)`.padEnd(24);
		const differStr = `${totalDiffer} differ`.padEnd(12);
		const errorStr = `${totalErrors} errors`;
		console.log(`  ${'total'.padEnd(12)} ${matchStr} | ${differStr} | ${errorStr}`);
	}

	// Show first N mismatches
	const allDiffers = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'differ')
	);

	if (allDiffers.length > 0) {
		if (showDiff) {
			// Show diffs for mismatches
			const toShow = allDiffers.slice(0, diffLimit);
			for (const r of toShow) {
				console.log(`\n${'═'.repeat(70)}`);
				console.log(`File: ${relPath(r.file.path, resolvedPath)}`);
				console.log('─'.repeat(70));

				// prettier = expected, ours = actual
				const diff = diffLines(r.prettier!, r.ours!);
				for (const line of formatDiffForTerminal(diff)) {
					console.log(line);
				}
			}
			if (allDiffers.length > diffLimit) {
				console.log(`\n... and ${allDiffers.length - diffLimit} more mismatches`);
			}
		} else {
			console.log(`\nFirst ${Math.min(5, allDiffers.length)} mismatches:`);
			for (const r of allDiffers.slice(0, 5)) {
				console.log(`  ${relPath(r.file.path, resolvedPath)}`);
			}
			if (allDiffers.length > 5) {
				console.log(`  ... and ${allDiffers.length - 5} more`);
			}
		}
	}

	// Show first N errors
	const allErrors = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.status === 'error')
	);

	if (allErrors.length > 0) {
		console.log(`\nFirst ${Math.min(3, allErrors.length)} errors:`);
		for (const r of allErrors.slice(0, 3)) {
			console.log(`  ${relPath(r.file.path, resolvedPath)}: ${r.error?.slice(0, 80)}`);
		}
		if (allErrors.length > 3) {
			console.log(`  ... and ${allErrors.length - 3} more`);
		}
	}

	// Cleanup
	canonical.dispose();
	native.dispose();
}

main();
