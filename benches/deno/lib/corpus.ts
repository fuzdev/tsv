/**
 * Corpus loading for benchmarks and comparison.
 *
 * - DevReposLoader: loads from DEFAULT_CORPUS_PATHS (hardcoded ~/dev/ repos)
 * - DirectoryLoader: loads from a single directory path
 *
 * Both support `load()` (collect all) and `stream()` (async generator for GC).
 */

import { exists } from '@std/fs/exists';
import { walk } from '@std/fs/walk';
import { basename, dirname, extname, join, resolve } from '@std/path';

import type { Language, Logger, SourceFile } from './types.ts';

//
// Shared Utilities
//

/** Detect language from file extension */
function detectLanguage(path: string): Language | null {
	const ext = extname(path).toLowerCase();
	switch (ext) {
		case '.svelte':
		case '.html':
			return 'svelte';
		case '.ts':
		case '.js':
			return 'typescript';
		case '.css':
			return 'css';
		default:
			return null;
	}
}

/** Default exclusion patterns */
const DEFAULT_EXCLUSIONS = [
	'.d.ts', // Declaration files
	'/node_modules/',
	'/.svelte-kit/',
	'/.gro/',
	'/build/',
	'/dist/',
];

const DEFAULT_EXTENSIONS = ['svelte', 'ts', 'js', 'css'];

/** Check if file should be excluded */
function shouldExclude(path: string): boolean {
	const name = basename(path);
	for (const pattern of DEFAULT_EXCLUSIONS) {
		if (pattern.startsWith('/')) {
			if (path.includes(pattern)) return true;
		} else {
			if (name.includes(pattern)) return true;
		}
	}
	return false;
}

/**
 * Check if a file has a companion options.json (non-default prettier settings).
 * Checks two patterns:
 * - Same directory: `dir/options.json` (prettier-plugin-svelte formatting samples)
 * - Sibling file: `name.options.json` (prettier-plugin-svelte printer samples)
 *
 * Caches directory-level checks to avoid redundant filesystem calls.
 */
const optionsDirCache = new Map<string, boolean>();

async function hasCompanionOptions(filePath: string): Promise<boolean> {
	const dir = dirname(filePath);

	// Check dir/options.json (cached per directory)
	if (optionsDirCache.has(dir)) {
		if (optionsDirCache.get(dir)) return true;
	} else {
		const dirHasOptions = await exists(join(dir, 'options.json'));
		optionsDirCache.set(dir, dirHasOptions);
		if (dirHasOptions) return true;
	}

	// Check name.options.json (per-file, not cached)
	const nameWithoutExt = basename(filePath).replace(/\.[^.]+$/, '');
	return exists(join(dir, `${nameWithoutExt}.options.json`));
}

//
// Shared Walk
//

interface WalkOptions {
	extensions?: string[];
	/** Per-file filter — return true to skip */
	skip?: (path: string) => boolean | Promise<boolean>;
}

/** Walk a directory and yield source files one at a time */
async function* walkCorpus(
	dirPath: string,
	options: WalkOptions = {},
): AsyncGenerator<SourceFile> {
	const extensions = options.extensions ?? DEFAULT_EXTENSIONS;

	for await (const entry of walk(dirPath, { exts: extensions, includeDirs: false })) {
		if (shouldExclude(entry.path)) continue;

		const language = detectLanguage(entry.path);
		if (!language) continue;

		if (options.skip && (await options.skip(entry.path))) continue;

		try {
			const content = await Deno.readTextFile(entry.path);
			yield {
				path: entry.path,
				content,
				language,
				bytes: new TextEncoder().encode(content).length,
			};
		} catch (e) {
			console.warn(`Warning: Could not read ${entry.path}: ${e}`);
		}
	}
}

/** Log corpus summary */
function logCorpusSummary(files: SourceFile[], logger: Logger): void {
	const totalBytes = files.reduce((sum, f) => sum + f.bytes, 0);
	const byLang = { svelte: 0, typescript: 0, css: 0 };
	for (const f of files) byLang[f.language]++;
	logger(`\nCorpus loaded:`);
	logger(`  Total: ${files.length} files, ${(totalBytes / 1024 / 1024).toFixed(2)} MB`);
	logger(`  Svelte: ${byLang.svelte} files`);
	logger(`  TypeScript: ${byLang.typescript} files`);
	logger(`  CSS: ${byLang.css} files`);
}

/** Group files by language for targeted benchmarks */
export function groupByLanguage(files: SourceFile[]): Record<Language, SourceFile[]> {
	return {
		svelte: files.filter((f) => f.language === 'svelte'),
		typescript: files.filter((f) => f.language === 'typescript'),
		css: files.filter((f) => f.language === 'css'),
	};
}

//
// Corpus Path
//

/** A corpus entry: string path or object with path + extensions/skip override */
type CorpusPath = string | {
	path: string;
	extensions?: string[];
	skip?: (path: string) => boolean | Promise<boolean>;
};

//
// Dev Repos Loader
//

/**
 * Default corpus paths relative to project root (cwd).
 * Paths that don't exist are silently skipped at load time.
 */
const DEFAULT_CORPUS_PATHS: CorpusPath[] = [
	// Large apps
	'../zzz/src',
	// Fuz ecosystem
	'../fuz.dev/src',
	'../fuz_app/src',
	'../fuz_blog/src',
	'../fuz_code/src',
	'../fuz_css/src',
	'../fuz_docs/src',
	'../fuz_gitops/src',
	'../fuz_mastodon/src',
	'../fuz_template/src',
	'../fuz_ui/src',
	'../fuz_util/src',
	// Build tooling
	'../gro/src',
	'../svelte-docinfo/src',
	'../tsv.dev/src',
	// External projects (monorepo subpaths)
	'../kit/packages/kit/src',
	'../svelte/packages/svelte/src',
	'../svelte.dev/apps/svelte.dev/src',
	'../svelte.dev/packages/repl/src',
	'../svelte.dev/packages/site-kit/src',
	// prettier-plugin-svelte test cases (.html treated as Svelte, skip non-default options)
	{ path: '../prettier-plugin-svelte/test', extensions: ['html'], skip: hasCompanionOptions },
	// Prettier test cases (formatting edge cases and regression tests)
	'../prettier/tests/format/typescript',
	'../prettier/tests/format/js',
	'../prettier/tests/format/css',
	{ path: '../prettier/tests/format/html', extensions: ['html'] },
	// TODO: '../prettier/tests/format/jsx' (91 files — JSX formatting edge cases)
	// TODO: '../svelte/packages/svelte/tests' (7124 files)
];

/**
 * Loads corpus from DEFAULT_CORPUS_PATHS.
 * Paths are relative to cwd; non-existent paths are silently skipped.
 */
export class DevReposLoader {
	async *stream(logger: Logger = console.log): AsyncGenerator<SourceFile> {
		logger(`Loading ${DEFAULT_CORPUS_PATHS.length} corpus paths`);

		for (const entry of DEFAULT_CORPUS_PATHS) {
			const isObject = typeof entry !== 'string';
			const entryPath = isObject ? entry.path : entry;
			const extensions = isObject ? entry.extensions : undefined;
			const skip = isObject ? entry.skip : undefined;
			const resolvedPath = resolve(entryPath);

			if (!(await exists(resolvedPath))) {
				continue;
			}

			let count = 0;
			for await (const file of walkCorpus(resolvedPath, { extensions, skip })) {
				count++;
				yield file;
			}

			if (count > 0) {
				logger(`  ${entryPath}: ${count} files`);
			}
		}
	}

	async load(logger: Logger = console.log): Promise<SourceFile[]> {
		const files: SourceFile[] = [];
		for await (const file of this.stream(logger)) {
			files.push(file);
		}
		logCorpusSummary(files, logger);
		return files;
	}
}

//
// Directory Loader
//

/**
 * Loads corpus from a single directory (recursive).
 * Useful for comparing against a specific project.
 */
export class DirectoryLoader {
	readonly #path: string;

	constructor(path: string) {
		this.#path = path;
	}

	async *stream(logger: Logger = console.log): AsyncGenerator<SourceFile> {
		const resolvedPath = resolve(this.#path);

		if (!(await exists(resolvedPath))) {
			throw new Error(`Directory not found: ${this.#path}`);
		}

		logger(`Loading from ${this.#path}`);
		yield* walkCorpus(resolvedPath);
	}

	async load(logger: Logger = console.log): Promise<SourceFile[]> {
		const files: SourceFile[] = [];
		for await (const file of this.stream(logger)) {
			files.push(file);
		}
		logCorpusSummary(files, logger);
		return files;
	}
}
