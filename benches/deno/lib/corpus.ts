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
import { basename, dirname, extname, join, resolve } from 'node:path';

import type { Language, Logger, SourceFile } from './types.ts';

//
// Shared Utilities
//

/** Detect language from file extension */
function detect_language(path: string): Language | null {
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
	// Prettier test fixtures that aren't representative of standard parsing:
	// `_errors_/` contains intentionally-malformed inputs prettier tracks for
	// error-recovery testing, `front-matter/` files embed YAML front-matter
	// (a prettier feature, not a property of the host language), and `cursor/`
	// files contain `<|>` markers for prettier's formatWithCursor() API tests
	// (syntactically invalid for every parser; also triggers stderr noise from
	// prettier-plugin-svelte's parser-fallback path). The `multiparser*` family
	// is excluded separately in `should_exclude` (segment-prefix match).
	'/_errors_/',
	'/front-matter/',
	'/cursor/',
];

const DEFAULT_EXTENSIONS = ['svelte', 'ts', 'js', 'css'];

/** Check if file should be excluded */
function should_exclude(path: string): boolean {
	const name = basename(path);
	const segments = path.split('/');
	// The `multiparser*` family — prettier's embedded-language tests. The bare
	// `multiparser/` dir routes `<script type="text/X">` HTML content to a
	// matching language parser (prettier-plugin-svelte has no equivalent, so
	// markdown/unknown-language script content flows into babel and throws); the
	// `js`/`typescript` suites' `multiparser-css` (CSS-in-JS/styled-components),
	// `-graphql`, `-markdown`, `-html` (lit-html), `-comments` (language-hint
	// comments), `-text`, and `-invalid` dirs reformat languages embedded in
	// tagged/identified template literals. tsv preserves template-literal content
	// verbatim — embedded-language reformatting is Out of Scope (see
	// docs/checklist_css.md) — so these are divergences, not bugs; drop the whole
	// family rather than counting it against conformance. Segment-prefix match so
	// new `multiparser-*` dirs from a prettier upgrade are caught automatically.
	if (segments.some((s) => s === 'multiparser' || s.startsWith('multiparser-'))) {
		return true;
	}
	for (const pattern of DEFAULT_EXCLUSIONS) {
		if (pattern.startsWith('/')) {
			// Directory patterns (`/node_modules/`) anchor on path SEGMENTS, not raw
			// substring — otherwise any absolute path that merely contains the text
			// (e.g. a `.../svelte.dev/.../build.../` dir) would be over-excluded.
			if (segments.includes(pattern.slice(1, -1))) return true;
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
const options_dir_cache = new Map<string, boolean>();

async function has_companion_options(file_path: string): Promise<boolean> {
	const dir = dirname(file_path);

	// Check dir/options.json (cached per directory)
	if (options_dir_cache.has(dir)) {
		if (options_dir_cache.get(dir)) return true;
	} else {
		const dir_has_options = await exists(join(dir, 'options.json'));
		options_dir_cache.set(dir, dir_has_options);
		if (dir_has_options) return true;
	}

	// Check name.options.json (per-file, not cached)
	const name_without_ext = basename(file_path).replace(/\.[^.]+$/, '');
	return exists(join(dir, `${name_without_ext}.options.json`));
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
async function* walk_corpus(
	dir_path: string,
	options: WalkOptions = {},
): AsyncGenerator<SourceFile> {
	const extensions = options.extensions ?? DEFAULT_EXTENSIONS;

	for await (const entry of walk(dir_path, { exts: extensions, includeDirs: false })) {
		if (should_exclude(entry.path)) continue;

		const language = detect_language(entry.path);
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
function log_corpus_summary(files: SourceFile[], logger: Logger): void {
	const total_bytes = files.reduce((sum, f) => sum + f.bytes, 0);
	const by_lang = { svelte: 0, typescript: 0, css: 0 };
	for (const f of files) by_lang[f.language]++;
	logger(`\nCorpus loaded:`);
	logger(`  Total: ${files.length} files, ${(total_bytes / 1024 / 1024).toFixed(2)} MB`);
	logger(`  Svelte: ${by_lang.svelte} files`);
	logger(`  TypeScript: ${by_lang.typescript} files`);
	logger(`  CSS: ${by_lang.css} files`);
}

/** Group files by language for targeted benchmarks */
export function group_by_language(files: SourceFile[]): Record<Language, SourceFile[]> {
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
	'../tsv.fuz.dev/src',
	// External projects (monorepo subpaths)
	'../kit/packages/kit/src',
	'../svelte/packages/svelte/src',
	'../svelte.dev/apps/svelte.dev/src',
	'../svelte.dev/packages/repl/src',
	'../svelte.dev/packages/site-kit/src',
	// prettier-plugin-svelte test cases (.html treated as Svelte, skip non-default options)
	{ path: '../prettier-plugin-svelte/test', extensions: ['html'], skip: has_companion_options },
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
			const is_object = typeof entry !== 'string';
			const entry_path = is_object ? entry.path : entry;
			const extensions = is_object ? entry.extensions : undefined;
			const skip = is_object ? entry.skip : undefined;
			const resolved_path = resolve(entry_path);

			if (!(await exists(resolved_path))) {
				continue;
			}

			let count = 0;
			for await (const file of walk_corpus(resolved_path, { extensions, skip })) {
				count++;
				yield file;
			}

			if (count > 0) {
				logger(`  ${entry_path}: ${count} files`);
			}
		}
	}

	async load(logger: Logger = console.log): Promise<SourceFile[]> {
		const files: SourceFile[] = [];
		for await (const file of this.stream(logger)) {
			files.push(file);
		}
		log_corpus_summary(files, logger);
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
		const resolved_path = resolve(this.#path);

		if (!(await exists(resolved_path))) {
			throw new Error(`Directory not found: ${this.#path}`);
		}

		logger(`Loading from ${this.#path}`);
		yield* walk_corpus(resolved_path);
	}

	async load(logger: Logger = console.log): Promise<SourceFile[]> {
		const files: SourceFile[] = [];
		for await (const file of this.stream(logger)) {
			files.push(file);
		}
		log_corpus_summary(files, logger);
		return files;
	}
}
