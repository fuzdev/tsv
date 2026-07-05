/**
 * Corpus loading for benchmarks and comparison.
 *
 * One tagged entry list (`CORPUS_ENTRIES`), three views:
 *
 * - `perf` — real-world code only (app + upstream framework source, with fixture
 *   subtrees pruned; `*.test.ts` stays). The `deno task bench` corpus, so
 *   throughput reflects real code rather than formatter edge-case suites.
 * - `gates` — real + the prettier fixture suites: exactly the pre-split default
 *   corpus. The correctness gates (`corpus:compare:*` `--all`, `skip_triage`,
 *   `wasm_json_probe`) keep this scope — their sanction lists and coverage were
 *   reviewed against it.
 * - `conformance` — everything: `gates` plus the parse-conformance suites
 *   (Svelte's compiler tests, the wpt-css harvest cache, test262 graded
 *   positives). The per-tool parse coverage/throughput measurement surface
 *   (`deno task bench:conformance`).
 *
 * - DevReposLoader: loads one view of CORPUS_ENTRIES (hardcoded ~/dev/ repos)
 * - DirectoryLoader: loads from a single directory path
 *
 * Both support `load()` (collect all) and `stream()` (async generator for GC).
 */

import { fs_exists } from '@fuzdev/fuz_util/fs.ts';
import { readdir, readFile } from 'node:fs/promises';
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
		const dir_has_options = await fs_exists(join(dir, 'options.json'));
		options_dir_cache.set(dir, dir_has_options);
		if (dir_has_options) return true;
	}

	// Check name.options.json (per-file, not cached)
	const name_without_ext = basename(file_path).replace(/\.[^.]+$/, '');
	return fs_exists(join(dir, `${name_without_ext}.options.json`));
}

//
// Shared Walk
//

/** Per-file skip filter — return true to skip. `relative` is the path below the walk root. */
type SkipFn = (path: string, relative: string) => boolean | Promise<boolean>;

interface WalkOptions {
	extensions?: string[];
	skip?: SkipFn;
}

/** Walk a directory and yield source files one at a time.
 *
 * Uses `node:fs/promises` recursive `readdir` (identical output under Deno and
 * Node) for the directory traversal, then reads each file's content lazily so
 * the per-file content (the memory-heavy part) is yielded and released one at a
 * time. Paths are sorted for deterministic, runtime-independent ordering. The
 * `extensions` set replaces `@std/walk`'s `exts` filter; directories fall out
 * naturally (no matching extension), and `should_exclude` does the post-hoc
 * pruning exactly as before. */
async function* walk_corpus(
	dir_path: string,
	options: WalkOptions = {},
): AsyncGenerator<SourceFile> {
	const extensions = options.extensions ?? DEFAULT_EXTENSIONS;
	const ext_set = new Set(extensions.map((e) => `.${e.toLowerCase()}`));

	const relative_paths = await readdir(dir_path, { recursive: true });
	relative_paths.sort();

	for (const relative of relative_paths) {
		const path = join(dir_path, relative);
		if (!ext_set.has(extname(path).toLowerCase())) continue;
		if (should_exclude(path)) continue;

		const language = detect_language(path);
		if (!language) continue;

		if (options.skip && (await options.skip(path, relative))) continue;

		try {
			const content = await readFile(path, 'utf8');
			yield {
				path,
				content,
				language,
				bytes: new TextEncoder().encode(content).length,
			};
		} catch (e) {
			console.warn(`Warning: Could not read ${path}: ${e}`);
		}
	}
}

/**
 * Yield files from a harvest-produced JSON path list (an array of paths
 * relative to the project root — e.g. the test262 graded-positives list
 * written by `bench:harvest:test262`). The harvest already curated the set,
 * so `should_exclude` and entry skips don't apply; unknown extensions are
 * still dropped.
 */
async function* load_file_list(list_path: string): AsyncGenerator<SourceFile> {
	let paths: string[];
	try {
		paths = JSON.parse(await readFile(list_path, 'utf8'));
	} catch (e) {
		console.warn(`Warning: Could not read file list ${list_path}: ${e}`);
		return;
	}
	paths.sort();
	for (const relative of paths) {
		const path = resolve(relative);
		const language = detect_language(path);
		if (!language) continue;
		try {
			const content = await readFile(path, 'utf8');
			yield {
				path,
				content,
				language,
				bytes: new TextEncoder().encode(content).length,
			};
		} catch (e) {
			console.warn(`Warning: Could not read ${path}: ${e}`);
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
// Corpus Entries
//

/**
 * Which concern an entry serves — the axis the views select on.
 *
 * - `real` — application/library/framework source: what the perf numbers
 *   should reflect.
 * - `prettier_fixture` — Prettier's and prettier-plugin-svelte's test suites:
 *   deliberately tricky edge cases the formatting-conformance gates need but
 *   that skew throughput toward hard cases.
 * - `suite` — parse-conformance suites (Svelte compiler tests, wpt-css
 *   harvest, test262 graded positives): per-tool parse coverage measurement
 *   only, never timed as "typical code".
 */
export type CorpusTier = 'real' | 'prettier_fixture' | 'suite';

/** A named subset of `CORPUS_ENTRIES` — see the module doc for what each view is for. */
export type CorpusView = 'perf' | 'gates' | 'conformance';

/** Fields shared by every corpus entry, whatever its file source. */
interface CorpusEntryBase {
	tier: CorpusTier;
	extensions?: string[];
	skip?: SkipFn;
	/**
	 * Tolerate absence with a warning instead of failing the run. Only for the
	 * derived harvest caches, whose source checkouts (`../wpt`, `../test262`)
	 * are legitimately machine-dependent — the harvest tasks warn-and-skip the
	 * same way, and `corpus_sources` disclosure covers the smaller corpus.
	 * Everything else is required: a missing repo fails fast (see `stream`).
	 */
	optional?: boolean;
	/** Remedy appended to the missing-entry warning/error (e.g. the harvest task to run). */
	hint?: string;
}

/**
 * A corpus entry plus its tier, carrying exactly one file source: a directory to
 * walk (`path`, relative to project root) or a harvest-produced JSON path list
 * (`files_from`, also project-root-relative). The union enforces the
 * "exactly one of" invariant the type used to only assert in a doc comment —
 * so `entry_source` narrows to a plain `string` without a non-null assertion.
 */
type CorpusEntry =
	| (CorpusEntryBase & { path: string; files_from?: never })
	| (CorpusEntryBase & { files_from: string; path?: never });

/** The entry's declared file source (directory or file-list path). */
function entry_source(entry: CorpusEntry): string {
	return entry.path !== undefined ? entry.path : entry.files_from;
}

/**
 * Skip for the Svelte test-suite entry — shares the artifact *exclusions* of the
 * conformance gate (`diagnostics/svelte_fixtures_compare.ts`), though the two
 * scopes differ: that gate whitelists only the canonical `.svelte` inputs, while
 * this bench entry keeps every non-artifact file across all languages (the
 * per-tool parse-coverage surface). The shared exclusions: `_`-prefixed segments
 * are runner config/snapshot artifacts (`_config.js` boilerplate is the vast
 * majority of the suite's `.js` files; `_expected` dirs are snapshots),
 * `migrate/` holds Svelte-4 migrator inputs that are not modern-parse targets,
 * and `output.svelte` files are expected-output snapshots. Counting any of these
 * against per-tool coverage would misstate conformance with the modern parser.
 */
const svelte_tests_skip = (_path: string, relative: string): boolean => {
	const segments = relative.split('/');
	return (
		segments.some((s) => s.startsWith('_')) ||
		segments.includes('migrate') ||
		segments[segments.length - 1] === 'output.svelte'
	);
};

/**
 * Perf-view prune: fixture subtrees living inside the real repos' src.
 * `fixtures` segments anywhere (test fixtures in gro, svelte-docinfo,
 * fuz_gitops, kit), plus `samples` segments only when a `test` segment
 * precedes them (kit's `test/samples`) — a bare `samples` dir can be real app
 * code (fuz_code's sample routes). `*.test.ts` files stay: tests are real
 * code exercising real APIs.
 */
const should_prune_perf = (relative: string): boolean => {
	const segments = relative.split('/');
	if (segments.includes('fixtures')) return true;
	const samples_index = segments.indexOf('samples');
	return samples_index !== -1 && segments.slice(0, samples_index).includes('test');
};

/**
 * The tagged corpus entry list, relative to project root (cwd).
 * A missing entry fails the load unless marked `optional` — see `DevReposLoader`.
 */
const CORPUS_ENTRIES: CorpusEntry[] = [
	// Large apps
	{ path: '../zzz/src', tier: 'real' },
	// Fuz ecosystem
	{ path: '../fuz_app/src', tier: 'real' },
	{ path: '../fuz_blog/src', tier: 'real' },
	{ path: '../fuz_code/src', tier: 'real' },
	{ path: '../fuz_css/src', tier: 'real' },
	{ path: '../fuz_docs/src', tier: 'real' },
	{ path: '../fuz_gitops/src', tier: 'real' },
	{ path: '../fuz_mastodon/src', tier: 'real' },
	{ path: '../fuz_template/src', tier: 'real' },
	{ path: '../fuz_ui/src', tier: 'real' },
	{ path: '../fuz_util/src', tier: 'real' },
	// Build tooling
	{ path: '../gro/src', tier: 'real' },
	{ path: '../svelte-docinfo/src', tier: 'real' },
	{ path: '../tsv.fuz.dev/src', tier: 'real' },
	// External projects (monorepo subpaths)
	{ path: '../kit/packages/kit/src', tier: 'real' },
	{ path: '../svelte/packages/svelte/src', tier: 'real' },
	{ path: '../svelte.dev/apps/svelte.dev/src', tier: 'real' },
	{ path: '../svelte.dev/packages/repl/src', tier: 'real' },
	{ path: '../svelte.dev/packages/site-kit/src', tier: 'real' },
	// prettier-plugin-svelte test cases (.html treated as Svelte, skip non-default options)
	{
		path: '../prettier-plugin-svelte/test',
		tier: 'prettier_fixture',
		extensions: ['html'],
		skip: has_companion_options,
	},
	// Prettier test cases (formatting edge cases and regression tests)
	{ path: '../prettier/tests/format/typescript', tier: 'prettier_fixture' },
	{ path: '../prettier/tests/format/js', tier: 'prettier_fixture' },
	{ path: '../prettier/tests/format/css', tier: 'prettier_fixture' },
	{ path: '../prettier/tests/format/html', tier: 'prettier_fixture', extensions: ['html'] },
	// TODO: '../prettier/tests/format/jsx' (91 files — JSX formatting edge cases)
	// Parse-conformance suites (`conformance` view only)
	{ path: '../svelte/packages/svelte/tests', tier: 'suite', skip: svelte_tests_skip },
	{
		path: 'benches/js/.cache/wpt_css',
		tier: 'suite',
		extensions: ['css'],
		optional: true,
		hint: 'run `deno task bench:harvest:wpt` (needs ../wpt)',
	},
	{
		files_from: 'benches/js/.cache/test262_files.json',
		tier: 'suite',
		optional: true,
		hint: 'run `deno task bench:harvest:test262` (needs ../test262)',
	},
];

const TIERS_BY_VIEW: Record<CorpusView, CorpusTier[]> = {
	perf: ['real'],
	gates: ['real', 'prettier_fixture'],
	conformance: ['real', 'prettier_fixture', 'suite'],
};

//
// Dev Repos Loader
//

/** One loaded corpus entry's disclosure row — reported as `corpus_sources`. */
export interface CorpusSource {
	path: string;
	files: number;
	/**
	 * Per-language split of `files` (the svelte/typescript/css counts sum to
	 * `files`), so the composition disclosure shows each entry's language mix
	 * rather than only a bare total.
	 */
	by_language: Record<Language, number>;
}

/**
 * Loads one view of `CORPUS_ENTRIES`.
 * Paths are relative to cwd. Missing entries FAIL FAST (before any file is
 * yielded) unless the entry is `optional` (the derived harvest caches, warned)
 * or `allow_missing` is set — a silently smaller corpus makes perf numbers
 * non-comparable and lets a correctness gate pass while grading less than it
 * claims. The view is required — it's load-bearing (it decides what a number
 * or a gate verdict means), so every construction site picks one explicitly:
 * `gates` for anything gate-like (the pre-split corpus the sanction lists and
 * divergence coverage were reviewed against), `perf`/`conformance` for the
 * bench surfaces.
 */
export class DevReposLoader {
	readonly view: CorpusView;
	readonly allow_missing: boolean;

	/**
	 * Per-entry file counts from the most recent `stream()`/`load()` — the
	 * report's `corpus_sources` disclosure, so a run tolerating a missing
	 * optional suite (`../wpt`, `../test262`) produces a report that says so
	 * instead of silently shrinking.
	 */
	sources: CorpusSource[] = [];

	constructor(view: CorpusView, options?: { allow_missing?: boolean }) {
		this.view = view;
		this.allow_missing = options?.allow_missing ?? false;
	}

	async *stream(logger: Logger = console.log): AsyncGenerator<SourceFile> {
		const tiers = TIERS_BY_VIEW[this.view];
		const entries = CORPUS_ENTRIES.filter((e) => tiers.includes(e.tier));

		// Fail fast on missing entries — all existence checks up front, before
		// any file is yielded, so a partial corpus can't be half-processed.
		const present: CorpusEntry[] = [];
		const missing: string[] = [];
		for (const entry of entries) {
			const entry_path = entry_source(entry);
			if (await fs_exists(resolve(entry_path))) {
				present.push(entry);
			} else if (entry.optional) {
				logger(`  ⚠ optional corpus entry missing: ${entry_path}${entry.hint ? ` — ${entry.hint}` : ''}`);
			} else {
				missing.push(entry_path + (entry.hint ? ` (${entry.hint})` : ''));
			}
		}
		if (missing.length > 0) {
			if (this.allow_missing) {
				for (const m of missing) {
					logger(`  ⚠ corpus entry missing (allow_missing): ${m}`);
				}
			} else {
				throw new Error(
					`Missing corpus entr${missing.length === 1 ? 'y' : 'ies'} (${this.view} view): ` +
						`${missing.join(', ')} — clone the missing repo(s), or opt into a partial ` +
						`corpus with allow_missing (BENCH_ALLOW_MISSING=1 for the bench).`,
				);
			}
		}

		this.sources = [];
		logger(`Loading ${present.length} corpus paths (${this.view} view)`);

		for (const entry of present) {
			const entry_path = entry_source(entry);
			const resolved_path = resolve(entry_path);

			let count = 0;
			const by_language: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
			if (entry.files_from !== undefined) {
				for await (const file of load_file_list(resolved_path)) {
					count++;
					by_language[file.language]++;
					yield file;
				}
			} else {
				const skip: SkipFn | undefined = this.view === 'perf'
					? async (path, relative) =>
						should_prune_perf(relative) || ((await entry.skip?.(path, relative)) ?? false)
					: entry.skip;
				for await (
					const file of walk_corpus(resolved_path, { extensions: entry.extensions, skip })
				) {
					count++;
					by_language[file.language]++;
					yield file;
				}
			}

			if (count > 0) {
				this.sources.push({ path: entry_path, files: count, by_language });
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

		if (!(await fs_exists(resolved_path))) {
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
