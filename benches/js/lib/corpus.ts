/**
 * Corpus loading for benchmarks and comparison.
 *
 * One tagged entry list (`corpus_entries` — the `../corpora` snapshot's collections,
 * each placed in a tier by `COLLECTION_TIERS` and spelled from the snapshot's own
 * manifest, plus the pinned suite checkouts and the derived caches), four views:
 *
 * - `perf` — real-world code only (app + upstream framework source from the
 *   `../corpora` snapshot, whose manifest already leaves the upstreams' test
 *   fixtures behind; `*.test.ts` stays). The
 *   `deno task bench` corpus, so throughput reflects real code rather than formatter
 *   edge-case suites — and one pinned commit names all of it.
 * - `gates` — every snapshot tier (`perf`'s two plus the `third_party` libraries) +
 *   the prettier fixture suites. The correctness gates (`corpus:compare:*` `--all`,
 *   `skip_triage`, `wasm_json_probe`) keep this scope — their sanction lists and
 *   coverage were reviewed against it. Every file here comes from a pinned checkout,
 *   so every count pin gates over the whole view.
 * - `conformance` — the hard parse cases only: the prettier fixture suites plus
 *   the parse-conformance suites (Svelte's compiler tests, the wpt-css harvest
 *   cache, test262 graded positives). Deliberately EXCLUDES the `real` perf tier,
 *   so the conformance coverage surface and the perf corpus are mutually exclusive:
 *   `perf` is the "every in-scope tool must fully process it" corpus (`bench.ts`
 *   hard-fails an unlisted failure there — see `perf_omit.ts`), `conformance` is
 *   where sub-100% coverage is the metric. The per-tool parse coverage surface
 *   (`deno task bench:conformance`).
 * - `robustness` — the real-code robustness sweeps' scope (`audit:corpus`,
 *   `idempotency:sweep`): the WHOLE snapshot — every collection `../corpora` vendors,
 *   placed in a tier or not, as one root, since a sweep grades an invariant rather than
 *   a pinned count — plus the `svelte_styles` harvest cache, PLUS the live DIFF — the
 *   files of the `real` repos' working trees (the `live` tier) whose bytes differ from,
 *   or are absent in, their collection, minus what the snapshot's manifest excludes and
 *   what git ignores. That is where new syntax shows up before any refresh, at a cost
 *   proportional to the drift. The snapshot root and the cache as directories plus the
 *   live file list (`corpus_robustness_seeds`), nothing counted or pinned.
 *
 * - CorpusLoader: loads one view of the entries (the `../corpora` snapshot's
 *   collections, the pinned suite checkouts beside it, and the derived harvest caches)
 * - DirectoryLoader: loads from a single directory path
 *
 * Both support `load()` (collect all) and `stream()` (async generator for GC).
 */

import { fs_exists } from '@fuzdev/fuz_util/fs.ts';
import { Buffer } from 'node:buffer';
import { execFile } from 'node:child_process';
import { readdir, readFile } from 'node:fs/promises';
import { basename, dirname, extname, join, relative, resolve, sep } from 'node:path';
import { promisify } from 'node:util';

import {
	CORPORA_COLLECTIONS,
	CORPORA_MANIFEST,
	CORPORA_ROOT,
	type CorporaCollection,
	is_under,
	load_corpora_manifest
} from './corpora.ts';
import { clone_hint } from './corpus_repos.ts';
import type { Language, Logger, ParseGoal, SourceFile } from './types.ts';

const exec_file = promisify(execFile);

//
// Shared Utilities
//

/** Detect language from file extension */
function detect_language(path: string): Language | null {
	const ext = extname(path).toLowerCase();
	switch (ext) {
		// `.html` → svelte for `../prettier-plugin-svelte/test`, whose printer samples
		// are `.html` files holding Svelte components — the extension is that suite's
		// convention, not a claim about HTML. It reaches one other entry,
		// `../prettier/tests/format/html`, where the files really are HTML documents:
		// svelte/compiler rejects 40 of the 124 (harvested into the reject cache) and
		// parses the rest, so 84 plain HTML files sit in the Svelte parse denominator
		// at ~1.8% of it. Tolerated, not intended — Svelte's grammar is an HTML
		// superset, so an accept there is a weaker claim than the rest of the set
		// makes. The per-source coverage table keeps it separable.
		case '.svelte':
		case '.html':
			return 'svelte';
		// The whole JS/TS family `tsv format` discovers, all parsed as TypeScript — the
		// module/commonjs spellings included, so a collection that gains one is graded
		// rather than silently skipped (the discovery audit grades tsv against the
		// snapshot, not this loader against tsv).
		case '.ts':
		case '.mts':
		case '.cts':
		case '.js':
		case '.mjs':
		case '.cjs':
			return 'typescript';
		case '.css':
			return 'css';
		default:
			return null;
	}
}

/**
 * Exclusion patterns applied to EVERY walk. `.d.ts` is deliberately NOT here:
 * the product formats declaration files (`tsv format` discovers them like any
 * `.ts`), so the corpus must exercise them — declaration-heavy shapes (overload
 * chains, huge unions, `declare` blocks) demonstrably carry divergence signal.
 */
const DEFAULT_EXCLUSIONS = [
	'/node_modules/',
	'/.svelte-kit/',
	'/.gro/',
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
	'/cursor/'
];

/**
 * Build-output patterns, applied only to UNCURATED walks (`DirectoryLoader` —
 * arbitrary project scans that may contain compiled artifacts). The curated
 * corpus entries point at reviewed `src/` trees where a `build/` segment is
 * real source (kit's `src/exports/vite/build/`), so `CorpusLoader` opts out.
 */
const BUILD_OUTPUT_EXCLUSIONS = ['/build/', '/dist/'];

/** The uncurated-walk pattern set, precomposed once (`should_exclude` runs per file). */
const UNCURATED_EXCLUSIONS = [...DEFAULT_EXCLUSIONS, ...BUILD_OUTPUT_EXCLUSIONS];

/** Every extension `tsv format` discovers — what an entry without its own `extensions` walks. */
const DEFAULT_EXTENSIONS = ['svelte', 'ts', 'mts', 'cts', 'js', 'mjs', 'cjs', 'css'];

/** Check if file should be excluded. `prune_build_output` adds the `/build/`+`/dist/` patterns (uncurated walks only). */
function should_exclude(path: string, prune_build_output: boolean): boolean {
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
	const patterns = prune_build_output ? UNCURATED_EXCLUSIONS : DEFAULT_EXCLUSIONS;
	for (const pattern of patterns) {
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
	/** Apply `BUILD_OUTPUT_EXCLUSIONS` (default true — curated entry walks opt out). */
	prune_build_output?: boolean;
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
	options: WalkOptions = {}
): AsyncGenerator<SourceFile> {
	const extensions = options.extensions ?? DEFAULT_EXTENSIONS;
	const ext_set = new Set(extensions.map((e) => `.${e.toLowerCase()}`));

	const relative_paths = await readdir(dir_path, { recursive: true });
	relative_paths.sort();

	for (const relative of relative_paths) {
		const path = join(dir_path, relative);
		if (!ext_set.has(extname(path).toLowerCase())) continue;
		if (should_exclude(path, options.prune_build_output ?? true)) continue;

		const language = detect_language(path);
		if (!language) continue;

		if (options.skip && (await options.skip(path, relative))) continue;

		try {
			const content = await readFile(path, 'utf8');
			yield {
				path,
				content,
				language,
				bytes: Buffer.byteLength(content, 'utf8')
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
 * still dropped, and an entry's declared `extensions` are enforced exactly as
 * `walk_corpus` enforces them — a declaration the loader ignored would be one a
 * per-language probe could trust and the corpus could silently contradict.
 */
async function* load_file_list(
	list_path: string,
	extensions?: string[]
): AsyncGenerator<SourceFile> {
	const ext_set =
		extensions === undefined ? null : new Set(extensions.map((e) => `.${e.toLowerCase()}`));
	let raw: Array<string | { path: string; goal?: ParseGoal }>;
	try {
		raw = JSON.parse(await readFile(list_path, 'utf8'));
	} catch (e) {
		console.warn(`Warning: Could not read file list ${list_path}: ${e}`);
		return;
	}
	// Accept both the `{path, goal}` shape (test262, goal-aware) and a bare
	// `string[]` (older caches / other file lists — goal defaults to module).
	const entries = raw.map((e) => (typeof e === 'string' ? { path: e } : e));
	entries.sort((a, b) => a.path.localeCompare(b.path));
	for (const { path: relative, goal } of entries) {
		const path = resolve(relative);
		if (ext_set !== null && !ext_set.has(extname(path).toLowerCase())) continue;
		const language = detect_language(path);
		if (!language) continue;
		try {
			const content = await readFile(path, 'utf8');
			yield {
				path,
				content,
				language,
				bytes: Buffer.byteLength(content, 'utf8'),
				goal
			};
		} catch (e) {
			console.warn(`Warning: Could not read ${path}: ${e}`);
		}
	}
}

/**
 * Format bytes as MB with one decimal — DECIMAL (1e6), the convention the report's
 * `MB/s` throughput already uses, so a corpus size and a rate over it stay
 * commensurable. Always MB, even below 1 MB (renders as e.g. `0.4 MB`), so a
 * column of sizes scans uniformly without unit-switching mid-table.
 *
 * EVERY printer of a corpus SIZE routes here: this module's loader summary, the
 * terminal corpus block and the markdown report's `**Corpus:**` line in
 * `bench.ts`, and `diagnostics/corpus_stats.ts`'s MB tier. They all describe the
 * same bytes, so a second spelling is a second answer — dividing by 1024² under
 * this same `MB` label makes a LARGER corpus print as fewer MB than a smaller one
 * measured decimally, which is a disagreement no reader can resolve from the
 * output.
 */
export function format_mb(bytes: number): string {
	return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

/** Log corpus summary */
function log_corpus_summary(files: SourceFile[], logger: Logger): void {
	const total_bytes = files.reduce((sum, f) => sum + f.bytes, 0);
	const by_lang = { svelte: 0, typescript: 0, css: 0 };
	for (const f of files) by_lang[f.language]++;
	logger(`\nCorpus loaded:`);
	logger(`  Total: ${files.length} files, ${format_mb(total_bytes)}`);
	logger(`  Svelte: ${by_lang.svelte} files`);
	logger(`  TypeScript: ${by_lang.typescript} files`);
	logger(`  CSS: ${by_lang.css} files`);
}

/** Group files by language for targeted benchmarks */
export function group_by_language(files: SourceFile[]): Record<Language, SourceFile[]> {
	return {
		svelte: files.filter((f) => f.language === 'svelte'),
		typescript: files.filter((f) => f.language === 'typescript'),
		css: files.filter((f) => f.language === 'css')
	};
}

//
// Corpus Entries
//

/**
 * Which concern an entry serves — the axis the views select on.
 *
 * - `real` — the author's own application/library source (zzz, the fuz ecosystem,
 *   gro, the personal sites), read from the `../corpora` snapshot
 *   (`collections/<name>/<subpath>`, the subpaths from its manifest). Every collection
 *   there is pinned at once by the id of the snapshot's `collections/` tree, recorded in
 *   `GATE_CHECKOUT_IDS['../corpora']` and verified by `deno task pins:audit:checkouts` —
 *   real code the perf numbers reflect, AND a reproducible tier the count pins hold over.
 * - `framework` — upstream framework source (kit, svelte, the svelte.dev subpaths),
 *   from the same snapshot. Real code in the perf view, reproducible in the gate.
 * - `third_party` — third-party Svelte libraries and tooling (flowbite-svelte,
 *   layerchart, layercake, svelte-ux, svelte-maplibre, language-tools), from the same
 *   snapshot: the gates' breadth — most of the snapshot's `.svelte`, and prettier-shaped
 *   code whose divergences the ecosystem repos never carry — and reproducible there. NOT
 *   the perf view: the throughput headline is ecosystem + framework code by design
 *   (flowbite alone would be a large share of its `.svelte`), and the perf invariant
 *   that every in-scope tool fully processes every file is unmeasured over these.
 * - `live` — the `real` repos as WORKING TREES (`../zzz/src`, …): unpinned,
 *   machine-dependent, and in no bench or gate view. Read only by the real-code
 *   robustness sweeps (the `robustness` view), and only as a DIFF against the
 *   snapshot (`live_diff_files`): a working tree is where new syntax shows up before
 *   any refresh does, and a content-loss or panic finding is a bug wherever it occurs
 *   — but the files the snapshot already holds byte-for-byte it has already swept.
 * - `prettier_fixture` — Prettier's and prettier-plugin-svelte's test suites
 *   (pinned checkouts): deliberately tricky edge cases the formatting-conformance
 *   gates need but that skew throughput toward hard cases. Reproducible.
 * - `suite` — parse-conformance suites (Svelte compiler tests, wpt-css
 *   harvest, test262 graded positives): per-tool parse coverage measurement
 *   only, never timed as "typical code".
 *
 * Every tier a bench or gate view holds is version-pinned (`GATE_CHECKOUT_IDS`),
 * so the count pins gate over whole views and no per-file reproducibility flag is
 * needed; `live` is the one unpinned tier and sits only in `robustness`.
 */
export type CorpusTier =
	'real' | 'framework' | 'third_party' | 'live' | 'prettier_fixture' | 'suite';

/** A named subset of the corpus entries — see the module doc for what each view is for. */
export type CorpusView = 'perf' | 'gates' | 'conformance' | 'robustness';

/** Fields shared by every corpus entry, whatever its file source. */
interface CorpusEntryBase {
	tier: CorpusTier;
	extensions?: string[];
	skip?: SkipFn;
	/**
	 * Tolerate absence with a warning instead of failing the run. Only for the
	 * derived harvest caches — wpt/test262 because their source checkouts are
	 * legitimately machine-dependent (their harvest tasks warn-and-skip the same
	 * way), svelte_styles because it's generated from the always-required snapshot
	 * and just may not have been harvested yet — and for the `live` working trees,
	 * which are whichever dev repos this machine has cloned. `corpus_sources`
	 * disclosure covers the smaller corpus. Everything else is required: a missing
	 * snapshot or suite checkout fails fast (see `stream`).
	 */
	optional?: boolean;
	/** Remedy appended to the missing-entry warning/error (e.g. the harvest task to run). */
	hint?: string;
}

/**
 * A corpus entry plus its tier, carrying exactly one file source: a directory to
 * walk (`path`, relative to project root) or a harvest-produced JSON path list
 * (`files_from`, also project-root-relative). The union enforces the
 * "exactly one of" invariant (which a doc comment alone cannot enforce) —
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
 * Stream a directory exactly as `CorpusLoader` would load it as a path entry: the
 * curated exclusions, no build-output prune, and NO fixture prune — the loader
 * prunes nothing at load time, because the snapshot's manifest leaves each
 * upstream's test fixtures behind by explicit `exclude` prefix. This is the
 * primitive for sizing a candidate BEFORE it becomes a collection (a live
 * `../<repo>/src`, or a materialized `../corpora/collections/<name>/…`): the
 * numbers are what the bench would measure, and any fixture-like bulk it reports
 * is the human's cue to spell an `exclude`, not a heuristic's to guess. Unlike a
 * `DirectoryLoader` walk, which applies the build-output prune an arbitrary project
 * scan needs. See `diagnostics/corpus_stats.ts`.
 */
export async function* stream_entry_candidate(dir: string): AsyncGenerator<SourceFile> {
	yield* walk_corpus(dir, { prune_build_output: false });
}

/** The tiers a snapshot collection can land in — the `CorpusTier`s read from `../corpora`. */
type CollectionTier = Extract<CorpusTier, 'real' | 'framework' | 'third_party'>;

/**
 * The tier of every snapshot collection a bench or gate view reads, by collection
 * name — the ONE table that places a collection (`TIERS_BY_VIEW` says which views each
 * tier reaches). The entries themselves are DERIVED from the snapshot's manifest
 * (`corpus_entries`): one per `subpath` the manifest names for the collection, so an
 * upstream's layout is spelled once, in the snapshot's own recipe (language-tools alone
 * has six subpaths), and a collection whose layout moves upstream re-spells nothing here.
 *
 * Every collection the manifest vendors is placed here today, but the table need not
 * name them all: a collection vendored ahead of its triage stays out of the views (as
 * earbetter and cosmicplayground once did) and is read only by the consumers that pin
 * no count (`corpus_snapshot_dir`); `corpus_untiered_collections` names such
 * collections for the doctor. The reverse — a name here the manifest no longer vendors —
 * is a refusal, since a view that quietly lost a collection would grade less than its
 * pins claim.
 *
 * - `real` and `framework` land in `perf` too, so an add there moves the throughput
 *   headline and the styles harvest as well as every corpus count pin.
 * - `third_party` lands in the gates only, so an add there re-pins the corpus counts and
 *   nothing else — see `CorpusTier`.
 */
const COLLECTION_TIERS: Record<string, CollectionTier> = {
	// Large apps
	zzz: 'real',
	// Fuz ecosystem
	fuz_app: 'real',
	fuz_blog: 'real',
	fuz_code: 'real',
	fuz_css: 'real',
	fuz_docs: 'real',
	fuz_gitops: 'real',
	fuz_mastodon: 'real',
	fuz_template: 'real',
	fuz_ui: 'real',
	fuz_util: 'real',
	mdz: 'real',
	// Build tooling
	gro: 'real',
	'svelte-docinfo': 'real',
	'tsv.fuz.dev': 'real',
	// Personal sites (public repos beyond the fuz ecosystem)
	'ryanatkn.com': 'real',
	'webdevladder.net': 'real',
	// Personal apps (public, The Unlicense; prettier-shaped, so their divergences carry
	// third-party-shaped signal the fuz repos cannot)
	earbetter: 'real',
	cosmicplayground: 'real',
	// Upstream framework source — each subpath a reviewed package `src/` tree, never a
	// whole monorepo, so build output and scaffolding stay out
	kit: 'framework',
	svelte: 'framework',
	'svelte.dev': 'framework',
	// Third-party Svelte libraries and tooling — the gates' breadth, not the perf corpus
	'flowbite-svelte': 'third_party',
	layerchart: 'third_party',
	layercake: 'third_party',
	'svelte-ux': 'third_party',
	'svelte-maplibre': 'third_party',
	'language-tools': 'third_party'
};

/**
 * Which collections' WORKING TREES the `live` tier diffs against the snapshot
 * (`live_diff_files`), by tier: a tier here claims its sibling checkouts (`../<name>`)
 * are edited ahead of the snapshot, which is what makes the diff an early warning. The
 * author's `real` repos are; the `framework` and `third_party` checkouts are pulled,
 * not edited, so a diff over them is empty until an upstream pull — when it is a whole
 * release at once, which `pins:audit:checkouts` already names for `../svelte`. Adding a
 * tier here is the entire change if that trade-off is ever wanted.
 */
const LIVE_DIFF_TIERS: readonly CollectionTier[] = ['real'];

/** A `live` working tree standing in for one collection subpath (`live_diff_files`). */
interface LiveTree {
	/** The collection name. */
	name: string;
	/** The upstream-relative subpath both sides are compared under (`<name>/<subpath>`). */
	subpath: string;
	/** The collection's manifest `exclude` prefixes. */
	exclude: string[];
	/** The sibling checkout standing in for the collection, `../<name>` (project-root-relative). */
	checkout: string;
	/** The tree to walk, `<checkout>/<subpath>` — also the `live` entry's path. */
	path: string;
}

/** A collection the tier table places, joined with what the manifest says it vendors. */
interface PlacedCollection extends CorporaCollection {
	name: string;
	tier: CollectionTier;
}

/** Everything derived from the snapshot's manifest plus the static entries, built once. */
interface CorpusCatalog {
	/** The tagged entry list every view selects from. */
	entries: CorpusEntry[];
	/** The `live` tier's trees, one per subpath of each `LIVE_DIFF_TIERS` collection. */
	live_trees: LiveTree[];
	/** Manifest collections `COLLECTION_TIERS` does not place — vendored, in no view. */
	untiered: string[];
}

let catalog_promise: Promise<CorpusCatalog> | undefined;

/**
 * The catalog, built once per process from the manifest (`load_corpora_manifest`).
 *
 * With the snapshot ABSENT (no checkout, so no manifest), every placed collection is
 * emitted as a single entry at its collection root — a path that does not exist, so each
 * consumer refuses or discloses it in its own terms (the loader's fail-fast with the
 * clone hint, the doctor's missing-entry list), and the views that read no collection
 * (`conformance`) are unaffected. A checkout WITHOUT a manifest, or one this reader
 * can't take, is drift rather than absence and throws: nothing can say what such a
 * checkout vendors, and walking a collection root on a guess would grade a corpus the
 * pins never described.
 */
function corpus_catalog(): Promise<CorpusCatalog> {
	catalog_promise ??= build_catalog();
	return catalog_promise;
}

async function build_catalog(): Promise<CorpusCatalog> {
	const read = await load_corpora_manifest();
	if (read.status === 'unreadable') {
		throw new Error(
			`${CORPORA_MANIFEST} ${read.reason} — no snapshot collection's entries can be derived from it`
		);
	}
	if (read.status === 'absent') {
		if (await fs_exists(resolve(CORPORA_ROOT))) {
			throw new Error(
				`${CORPORA_ROOT} is checked out but has no ${basename(CORPORA_MANIFEST)} — the snapshot's ` +
					'recipe is what says which subpaths each collection vendors, so nothing can be graded ' +
					'from it (restore the checkout, or clone it afresh)'
			);
		}
		// Absent snapshot: one placeholder per placed collection, at its root.
		const placeholders = (tier: CollectionTier): CorpusEntry[] =>
			placed_names(tier).map((name) => ({ path: `${CORPORA_COLLECTIONS}/${name}`, tier }));
		return { entries: assemble_entries(placeholders, []), live_trees: [], untiered: [] };
	}
	const { collections } = read;
	const placed: PlacedCollection[] = [];
	const dropped: string[] = [];
	for (const [name, tier] of Object.entries(COLLECTION_TIERS)) {
		const collection = collections.get(name);
		if (collection) placed.push({ name, tier, ...collection });
		else dropped.push(name);
	}
	if (dropped.length > 0) {
		throw new Error(
			`COLLECTION_TIERS places collections the snapshot's manifest no longer vendors: ` +
				`${dropped.join(', ')} — remove them from the table, or restore them in ${CORPORA_ROOT}`
		);
	}
	// One entry per manifest subpath, in table order then manifest order — the ORDER is
	// part of the perf view's stamped entry list (`corpus_view_paths`), so it is a
	// deliberate property of this derivation, not an incidental one.
	const snapshot_entries = (tier: CollectionTier): CorpusEntry[] =>
		placed
			.filter((c) => c.tier === tier)
			.flatMap((c) =>
				c.subpaths.map((subpath) => ({ path: `${CORPORA_COLLECTIONS}/${c.name}/${subpath}`, tier }))
			);
	const live_trees = placed
		.filter((c) => LIVE_DIFF_TIERS.includes(c.tier))
		.flatMap(({ name, subpaths, exclude }) =>
			subpaths.map((subpath): LiveTree => ({
				name,
				subpath,
				exclude,
				checkout: `../${name}`,
				path: `../${name}/${subpath}`
			}))
		);
	const untiered = [...collections.keys()].filter((name) => !(name in COLLECTION_TIERS));
	return { entries: assemble_entries(snapshot_entries, live_trees), live_trees, untiered };
}

/** The names `COLLECTION_TIERS` places in `tier`, in table order. */
function placed_names(tier: CollectionTier): string[] {
	return Object.entries(COLLECTION_TIERS)
		.filter(([, t]) => t === tier)
		.map(([name]) => name);
}

/**
 * The tagged corpus entry list, relative to project root (cwd): the snapshot tiers
 * (derived), the derived caches, the same `real` repos as live working trees, and the
 * pinned suite checkouts. A missing entry fails the load unless marked `optional` — see
 * `CorpusLoader`.
 */
function assemble_entries(
	snapshot_entries: (tier: CollectionTier) => CorpusEntry[],
	live_trees: LiveTree[]
): CorpusEntry[] {
	return [
		...snapshot_entries('real'),
		// Real-authored CSS extracted from the perf-view `.svelte` files' <style>
		// blocks, concatenated per source collection (bench:harvest:svelte-styles).
		// Derived but real content: ~3×es the otherwise-tiny standalone-CSS sample with
		// naturally-sized files, and in the gates view exercises the *standalone* CSS
		// path on real content (embedded CSS rides EmbedContext — a different path).
		// The same bytes are also timed in the svelte rows; rows are never summed.
		{
			path: 'benches/js/.cache/svelte_styles',
			tier: 'real',
			extensions: ['css'],
			optional: true,
			hint: 'run `deno task bench:harvest:svelte-styles`'
		},
		...snapshot_entries('framework'),
		...snapshot_entries('third_party'),
		// The `real` repos as live working trees — the `live` tier, `robustness` view only,
		// one entry per collection subpath. Optional: whichever of them this machine has cloned.
		...live_trees.map(({ path }): CorpusEntry => ({
			path,
			tier: 'live',
			optional: true,
			hint: 'a sibling working tree — absent is fine'
		})),
		// prettier-plugin-svelte test cases (.html treated as Svelte, skip non-default options)
		{
			path: '../prettier-plugin-svelte/test',
			tier: 'prettier_fixture',
			extensions: ['html'],
			skip: has_companion_options
		},
		// Prettier test cases (formatting edge cases and regression tests)
		{ path: '../prettier/tests/format/typescript', tier: 'prettier_fixture' },
		{ path: '../prettier/tests/format/js', tier: 'prettier_fixture' },
		{ path: '../prettier/tests/format/css', tier: 'prettier_fixture' },
		{ path: '../prettier/tests/format/html', tier: 'prettier_fixture', extensions: ['html'] },
		// '../prettier/tests/format/jsx' is deliberately absent: tsv rejects JSX by design
		// (drop-in for Svelte's parser; acorn without the JSX plugin rejects it too), so the
		// suite's 91 files would grade as always-reject noise, not conformance signal.
		// Parse-conformance suites (`conformance` view only)
		{ path: '../svelte/packages/svelte/tests', tier: 'suite', skip: svelte_tests_skip },
		{
			path: 'benches/js/.cache/wpt_css',
			tier: 'suite',
			extensions: ['css'],
			optional: true,
			hint: 'run `deno task bench:harvest:wpt` (needs ../wpt)'
		},
		{
			files_from: 'benches/js/.cache/test262_files.json',
			tier: 'suite',
			// Declared on the path-list entries too (the harvests write homogeneous
			// lists), so a per-language probe (`corpus_missing_entries`) can tell which
			// absent cache withholds which language; `load_file_list` enforces it.
			extensions: ['js'],
			optional: true,
			hint: 'run `deno task bench:harvest:test262` (needs ../test262)'
		},
		// The tsc corpus — TypeScript's own test cases, filtered to what tsc's parser
		// AND tsc's `.errors.txt` baselines both call well-formed (harvest_ts_repo.ts).
		// The TypeScript-SPECIFIC conformance inputs: without it the `parse/typescript`
		// group is ~95% test262, i.e. ECMAScript, with prettier's ~800 format fixtures
		// as its only TS. A bare path list, NOT goal-tagged like test262: tsc's
		// module-vs-script reading is semantic and never gates syntax, so feeding it to
		// parsers that take `sourceType` as a grammar switch would score them for
		// something tsc doesn't do (the measurement is in the harvest). Its REJECTS
		// sibling cache is deliberately not an entry — see the harvest.
		{
			files_from: 'benches/js/.cache/ts_repo_files.json',
			tier: 'suite',
			extensions: ['ts'],
			optional: true,
			hint: 'run `deno task bench:harvest:ts-repo` (needs ../typescript)'
		}
	];
}

/** The tagged corpus entry list — see `assemble_entries`; built once, from the snapshot's manifest. */
export async function corpus_entries(): Promise<CorpusEntry[]> {
	return (await corpus_catalog()).entries;
}

/**
 * Collections the snapshot vendors that `COLLECTION_TIERS` does not place — in no bench
 * or gate view, read only by the whole-snapshot consumers (`corpus_snapshot_dir`). For
 * `scripts/doctor.ts`, so a collection waiting on its triage is listed rather than
 * forgotten. Empty when the snapshot is absent.
 */
export async function corpus_untiered_collections(): Promise<string[]> {
	return (await corpus_catalog()).untiered;
}

const TIERS_BY_VIEW: Record<CorpusView, CorpusTier[]> = {
	// Both are real code from the pinned snapshot — the throughput headline. NOT
	// `third_party`: see `CorpusTier`.
	perf: ['real', 'framework'],
	// Every snapshot tier plus the prettier suites. Every file in this view comes
	// from a pinned checkout, so every count pin gates over the whole view.
	gates: ['real', 'framework', 'third_party', 'prettier_fixture'],
	// deliberately NO snapshot tier: the conformance coverage surface and the perf corpus
	// are mutually exclusive sets. perf is the "every in-scope tool must fully
	// process it" corpus (bench.ts hard-fails an unlisted failure); conformance is
	// the hard-cases-only surface where sub-100% coverage is the measurement.
	conformance: ['prettier_fixture', 'suite'],
	// The real-code robustness sweeps: the WHOLE snapshot AND the live working trees'
	// diff. Not a bench or gate view — nothing here is counted or pinned, so the sweeps
	// read the snapshot ROOT (every collection, placed in a tier or not) rather than the
	// tiers' entries; the tiers listed here declare the view's scope and contribute only
	// what lies OUTSIDE that root (the `svelte_styles` harvest cache, a `real` entry) and
	// the `live` diff's FILES (`corpus_robustness_seeds`). The `live` seat is also what
	// refuses loading the view whole (`CorpusLoader.stream`) or walking it as directories
	// (`corpus_present_dirs_for_tiers`).
	robustness: ['real', 'framework', 'third_party', 'live']
};

/**
 * Canonical-reject cache for the conformance view's Svelte set — absolute paths
 * of files `svelte/compiler` rejects, written by `bench:harvest:svelte-rejects`
 * (`diagnostics/svelte_reject_harvest.ts`). Excluded from the `conformance` view
 * so the parse-COVERAGE headline measures fidelity on *valid* Svelte, not
 * permissiveness over an adversarial corpus (svelte's own error fixtures + the
 * non-Svelte prettier HTML). Svelte only: svelte/compiler is the parser tsv is a
 * strict drop-in for, so its verdict defines validity; acorn-ts (trails modern
 * TS) and parseCss (lenient) are not validity oracles, so TS/CSS get no cache.
 */
const SVELTE_REJECT_CACHE = 'benches/js/.cache/svelte_parse_rejects.json';

/**
 * Load the canonical-reject cache as an absolute-path set, or `null` if absent.
 * Absent is fail-open: the conformance corpus stays un-filtered (the pre-harvest
 * numbers), matching the wpt/test262 optional-cache posture — disclosed in the
 * loader's log rather than silently assumed.
 */
async function load_svelte_reject_set(): Promise<Set<string> | null> {
	const cache_path = resolve(SVELTE_REJECT_CACHE);
	if (!(await fs_exists(cache_path))) return null;
	const paths = JSON.parse(await readFile(cache_path, 'utf8')) as string[];
	return new Set(paths);
}

//
// Corpus Loader
//

/**
 * A corpus source's GitHub origin — detected at report-build time
 * (`lib/corpus_repos.ts`: from the snapshot's manifest for a collection, so it names
 * the UPSTREAM the snapshot vendored; from git for any other checkout), so the report
 * links straight to the exact code the numbers were measured against without a
 * hand-maintained path→URL map.
 */
export interface CorpusRepoRef {
	/** Canonical https GitHub URL, e.g. `https://github.com/sveltejs/svelte`. */
	url: string;
	/** `owner/name` (e.g. `sveltejs/svelte`) — the linkified label. */
	slug: string;
	/**
	 * The commit the corpus was loaded at (full SHA), so links pin to it; `''`
	 * for a harvested cache linked at its canonical upstream root (no pin).
	 */
	commit: string;
	/** Path within the repo to this source (`''` = repo root); drives `/tree/<commit>/<subpath>`. */
	subpath: string;
}

// `clone_hint` is a pure helper; the reverse import (`corpus_repos.ts` → this
// module) is type-only, so there is no runtime import cycle.

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
	/**
	 * The source's GitHub origin, detected by `enrich_source_repos` at report-build
	 * time (manifest for a snapshot collection, git otherwise). `undefined` for a
	 * source with no GitHub remote (the local `svelte_styles` cache) or when detection
	 * fails (absent checkout).
	 */
	repo?: CorpusRepoRef;
}

/**
 * Whether an entry can contribute files of `language` — the one predicate behind
 * both the check-only probe below and the loader's `{ complete_for }` refusal, so
 * the two can never disagree about which absent entry withholds which language.
 *
 * Deliberately conservative in one direction: an entry that declares no
 * `extensions` walks every language and therefore answers YES for all three. That
 * over-counts (`../prettier/tests/format/typescript` ships no CSS), and it is the
 * safe side — the alternative is knowing an entry's language mix without walking
 * it, which is exactly what an ABSENT entry forbids.
 */
function entry_holds_language(entry: CorpusEntry, language: Language): boolean {
	return (
		entry.extensions === undefined ||
		entry.extensions.some((ext) => detect_language(`x.${ext}`) === language)
	);
}

/**
 * A view's entry paths as declared, present or not — the view's COMPOSITION, for a
 * stamp that must record what a harvest read: a collection joining a perf tier
 * changes the perf view without moving any checkout, and a stamp keyed on checkouts
 * alone would skip the re-harvest that change requires. The ORDER is part of the
 * stamp, and `corpus_entries` keeps it stable (table order, then manifest order).
 */
export async function corpus_view_paths(view: CorpusView): Promise<string[]> {
	const tiers = TIERS_BY_VIEW[view];
	return (await corpus_entries()).filter((e) => tiers.includes(e.tier)).map(entry_source);
}

/**
 * Check-only probe of a view's entries: the paths a default (`'fail'`) `stream()`
 * would refuse — non-`optional` and absent — plus the `optional` ones currently
 * absent, so a caller can apply whichever reading its own policy takes. Used by
 * `scripts/doctor.ts` — kept here so the doctor and the loader can't drift — and
 * by a grader that must degrade a pin to "not graded" rather than abort (the
 * bench's `enforce_css_reject_pin`), which passes `language` to ask only about the
 * entries that can hold it: an absent test262 cache withholds no CSS, so it must
 * not read as a partial CSS corpus.
 *
 * A grader that LOADS the corpus should not call this and then load — it should
 * load with `{ missing: { complete_for: language } }` and let the refusal come
 * from the loader, so the question is asked by the code that acts on the answer.
 * The one caller that legitimately asks first is a stamp-guarded grade, which has
 * to answer "may I trust the stamp?" BEFORE the stamp short-circuits the load
 * (`diagnostics/css_over_acceptance.ts`).
 */
export async function corpus_missing_entries(
	view: CorpusView,
	language?: Language
): Promise<{ missing: string[]; optional_missing: string[]; total: number }> {
	const tiers = TIERS_BY_VIEW[view];
	const entries = (await corpus_entries()).filter(
		(e) => tiers.includes(e.tier) && (language === undefined || entry_holds_language(e, language))
	);
	const missing: string[] = [];
	const optional_missing: string[] = [];
	for (const entry of entries) {
		const entry_path = entry_source(entry);
		if (await fs_exists(resolve(entry_path))) continue;
		(entry.optional ? optional_missing : missing).push(
			entry_path + (entry.hint ? ` (${entry.hint})` : '')
		);
	}
	return { missing, optional_missing, total: entries.length };
}

/**
 * The present, on-disk **directory** entries of a tier set — the paths to hand a
 * Rust-side tool that does its own walking (`tsv_debug`'s `fuzz` /
 * `roundtrip_audit` / `swallow_audit` all take dirs). `files_from` entries (the
 * test262 path list) have no directory to walk and are skipped.
 *
 * Selected by TIER rather than by view because neither consumer's scope is one:
 * `conformance`'s `render:audit` leg wants the `suite` tier (`../svelte`'s test
 * tree) beside the whole snapshot (`corpus_snapshot_dir`), and the robustness
 * sweeps (`corpus_robustness_seeds`) want the `robustness` view's pinned tiers beside
 * that same root, keeping only what lies outside it. A tier set holding `live` is
 * refused: that tier is swept as a diff, never as directories. A caller that already
 * seeds a whole root passes it as `outside` to skip the entries beneath it: a
 * collection is walked by the root when present and disclosed by the root's own
 * warning when absent, so probing it here again would only repeat that warning per
 * collection.
 *
 * Absent entries are skipped with a warning rather than failing the run: the
 * snapshot may not be cloned, and a sweep over what IS present is still worth
 * running. A gate that grades a corpus must use `stream()` (fail-fast) instead — a
 * silently smaller corpus can't be allowed to pass a gate.
 */
export async function corpus_present_dirs_for_tiers(
	tiers: readonly CorpusTier[],
	logger: Logger = console.log,
	options?: { outside?: string }
): Promise<string[]> {
	if (tiers.includes('live')) {
		throw new Error(
			'the `live` tier is swept as a diff against the snapshot, not as directories — ' +
				'use `corpus_robustness_seeds`'
		);
	}
	const outside = options?.outside;
	const dirs: string[] = [];
	for (const entry of (await corpus_entries()).filter((e) => tiers.includes(e.tier))) {
		if (entry.path === undefined) continue;
		if (outside !== undefined && is_under(entry.path, outside)) continue;
		if (await fs_exists(resolve(entry.path))) {
			dirs.push(entry.path);
		} else {
			logger(`  ⚠ corpus entry missing, skipped: ${entry.path}`);
		}
	}
	return dirs;
}

/**
 * The WHOLE snapshot as one directory seed — every collection `../corpora` vendors,
 * placed in a tier or not (`COLLECTION_TIERS`) — for a Rust-side sweep that grades an
 * invariant rather than a pinned count: the count pins are why the bench and gate views
 * read the tiers' entries (every collection there re-pins every corpus count, so a
 * collection waits on its triage before it is placed), and a no-panic / render /
 * round-trip verdict carries no such cost, so it reads everything the snapshot holds as
 * one root rather than thirty subpaths — `render:audit`'s conformance leg, and the
 * robustness sweeps through `corpus_robustness_seeds`. `null`, with a warning, when the
 * snapshot is not checked out.
 */
export async function corpus_snapshot_dir(logger: Logger = console.log): Promise<string | null> {
	if (await fs_exists(resolve(CORPORA_COLLECTIONS))) return CORPORA_COLLECTIONS;
	logger(
		`  ⚠ ${CORPORA_COLLECTIONS} absent — the snapshot is not checked out (${clone_hint(CORPORA_COLLECTIONS)})`
	);
	return null;
}

/** What the `live` tier contributes to a robustness sweep — see `live_diff_files`. */
export interface LiveDiff {
	/** Absolute paths to sweep: live files modified against, or absent from, their collection. */
	files: string[];
	/** Working trees walked (the present `live` entries). */
	trees: number;
	/** Live files byte-identical to their collection counterpart — the snapshot already sweeps them. */
	unchanged: number;
	/** Live files under a manifest `exclude` prefix (the upstream's own fixtures) — never corpus. */
	excluded: number;
	/** Live files git ignores (`*.local.*`, generated output) — scratch the snapshot can't hold, never corpus. */
	ignored: number;
}

/**
 * The files under `subdir` of the working tree at `repo_root` that git IGNORES
 * (`.gitignore` and friends), repo-relative with `/` separators. The snapshot is
 * materialized from git objects, so it can never hold one — a `*.local.ts` scratch
 * file or a generated bundle under `src/` reads as "absent from the snapshot" to a
 * plain walk and would be swept as new code. `null` when git can't answer (not a
 * repo, or `git` not runnable under the task's permissions); the caller discloses
 * that rather than sweeping them silently.
 */
async function git_ignored_files(repo_root: string, subdir: string): Promise<Set<string> | null> {
	try {
		const { stdout } = await exec_file('git', [
			'-C',
			repo_root,
			'ls-files',
			'--others',
			'--ignored',
			'--exclude-standard',
			'-z',
			'--',
			subdir
		]);
		return new Set(stdout.split('\0').filter((p) => p !== ''));
	} catch {
		return null;
	}
}

/**
 * The `live` tier as a DIFF against the snapshot: every file of a present working tree
 * (`../<name>/<subpath>`, one tree per subpath of each `LIVE_DIFF_TIERS` collection)
 * whose bytes differ from, or have no counterpart at, `collections/<name>/<same
 * upstream-relative path>` — minus that collection's manifest `exclude` prefixes, so the
 * fixtures the snapshot leaves out stay out here too, and minus what git ignores in the
 * tree, which the snapshot (materialized from git objects) cannot hold by construction
 * and which is scratch, not new code. This is the whole early-warning value of the
 * working trees (code written since the last refresh) at a cost proportional to the
 * drift: sweeping the trees whole re-grades thousands of files the snapshot has just
 * graded for the few dozen that differ. The trees are spelled from the snapshot's
 * manifest, so without it there are none to diff (disclosed); with the manifest but no
 * materialized counterparts every live file is a candidate and the trees are swept
 * whole, as before the snapshot — disclosed in the log rather than silently.
 */
export async function live_diff_files(logger: Logger = console.log): Promise<LiveDiff> {
	const { live_trees } = await corpus_catalog();
	const diff: LiveDiff = { files: [], trees: 0, unchanged: 0, excluded: 0, ignored: 0 };
	let absent = 0;
	let counterparts = 0;
	let git_unanswered = 0;
	for (const { name, subpath, exclude, checkout, path } of live_trees) {
		// Both sides are compared by the path beneath the checkout — the upstream-relative
		// path the manifest's `exclude` prefixes are spelled against, however deep the
		// subpath (`packages/kit/src`) sits.
		const repo_root = resolve(checkout);
		const tree = resolve(path);
		if (!(await fs_exists(tree))) {
			absent++;
			continue;
		}
		diff.trees++;
		const ignored = await git_ignored_files(repo_root, subpath);
		if (ignored === null) git_unanswered++;
		for await (const file of walk_corpus(tree, { prune_build_output: false })) {
			const rel = relative(repo_root, file.path).split(sep).join('/');
			if (ignored?.has(rel)) {
				diff.ignored++;
				continue;
			}
			if (exclude.some((prefix) => is_under(rel, prefix))) {
				diff.excluded++;
				continue;
			}
			let same = false;
			try {
				const snapshot = await readFile(resolve(CORPORA_COLLECTIONS, name, rel));
				counterparts++;
				// Bytes, not decoded text: the live side re-encodes, so a file that is not
				// valid UTF-8 compares unequal and is swept — the safe direction.
				same = snapshot.equals(Buffer.from(file.content, 'utf8'));
			} catch {
				// no counterpart in the snapshot — a new file, or an unmaterialized collection
			}
			if (same) diff.unchanged++;
			else diff.files.push(file.path);
		}
	}
	if (live_trees.length === 0) {
		logger('  live diff: no snapshot manifest, so no working trees to diff against it');
	} else if (diff.trees === 0) {
		logger(`  live diff: no working trees present (${absent} declared) — snapshot only`);
	} else {
		const notes = [
			counterparts === 0 ? '⚠ no snapshot counterparts, so the trees are swept WHOLE' : null,
			git_unanswered > 0
				? `⚠ git could not list the ignored files of ${git_unanswered} tree(s), so theirs are swept too`
				: null
		].filter((n) => n !== null);
		logger(
			`  live diff: ${diff.files.length} files to sweep across ${diff.trees} working trees ` +
				`(${diff.unchanged} byte-identical to the snapshot, ${diff.excluded} under a manifest exclude, ` +
				`${diff.ignored} gitignored${absent > 0 ? `, ${absent} trees absent` : ''})` +
				notes.map((n) => ` — ${n}`).join('')
		);
	}
	return diff;
}

/**
 * The seeds of a real-code robustness sweep (`audit:corpus`, `idempotency:sweep`): the
 * WHOLE snapshot as one directory (`corpus_snapshot_dir` — every collection, placed in
 * a tier or not), the `robustness` view's pinned-tier directories that lie OUTSIDE that
 * root (today the `svelte_styles` harvest cache — a collection directory is already
 * walked by the root), plus the `live` tier's diff FILES (`live_diff_files`). Hand all
 * of them to a Rust audit — its seed resolution takes directories and files alike.
 * Nothing here is counted or pinned.
 *
 * With the snapshot absent (warned once by `corpus_snapshot_dir`) what IS here — the
 * cache, the live diff — is still swept.
 */
export async function corpus_robustness_seeds(
	logger: Logger = console.log
): Promise<{ dirs: string[]; live_files: string[] }> {
	const root = await corpus_snapshot_dir(logger);
	const outside = await corpus_present_dirs_for_tiers(
		TIERS_BY_VIEW.robustness.filter((t) => t !== 'live'),
		logger,
		{ outside: CORPORA_COLLECTIONS }
	);
	const dirs = root === null ? outside : [root, ...outside];
	const { files: live_files } = await live_diff_files(logger);
	return { dirs, live_files };
}

/**
 * What an ABSENT corpus entry means to this run — one value rather than a boolean,
 * because the three answers are not two: a grader that pins an exact count needs a
 * tolerance the other two cannot express, and spelling it as
 * `allow_missing: true` is what let one leg silently grade a partial corpus.
 *
 * - `'fail'` (default) — a non-`optional` absent entry throws; an `optional` one
 *   (the derived harvest caches) warns. The ordinary gate posture.
 * - `'tolerate'` — every absent entry warns. The explicit opt-in behind
 *   `BENCH_ALLOW_MISSING=1`; the resulting numbers are not comparable and the run
 *   says so.
 * - `{ complete_for: Language }` — the PIN posture: an absent entry that can hold
 *   that language throws **whether or not it is `optional`**, and one that cannot
 *   warns. `optional` marks an entry a normal run may proceed without, which is a
 *   different claim from "this count still describes the whole corpus" — an exact
 *   pin graded over a corpus one harvest short is not a smaller measurement, it is
 *   a wrong one, and it reports as a moved pin.
 *
 * Note the asymmetry `{ complete_for }` exists to state: it is STRICTER than
 * `'fail'` on the optional entries and LOOSER on the required ones it cannot be
 * affected by. Neither boolean value is a substitute for it in either direction.
 */
export type MissingEntryPolicy = 'fail' | 'tolerate' | { complete_for: Language };

/**
 * Loads one view of the corpus entries (`corpus_entries`).
 * Paths are relative to cwd. Missing entries FAIL FAST (before any file is
 * yielded) per the `missing` policy above — a silently smaller corpus makes perf
 * numbers non-comparable and lets a correctness gate pass while grading less than
 * it claims. The view is required — it's load-bearing (it decides what a number
 * or a gate verdict means), so every construction site picks one explicitly:
 * `gates` for anything gate-like (the pre-split corpus the sanction lists and
 * divergence coverage were reviewed against), `perf`/`conformance` for the
 * bench surfaces.
 */
export class CorpusLoader {
	readonly view: CorpusView;
	readonly missing: MissingEntryPolicy;
	/**
	 * Whether the `conformance` view applies the Svelte canonical-reject cache
	 * (`SVELTE_REJECT_CACHE`). Default true. The reject **harvest** itself must set
	 * this false — it loads the conformance corpus to *produce* that cache, so
	 * applying it would exclude the very files it needs to grade (and, on a second
	 * run, overwrite the cache with an empty set).
	 */
	readonly apply_reject_cache: boolean;

	/**
	 * Per-entry file counts from the most recent `stream()`/`load()` — the
	 * report's `corpus_sources` disclosure, so a run tolerating a missing
	 * optional suite (`../wpt`, `../test262`) produces a report that says so
	 * instead of silently shrinking.
	 */
	sources: CorpusSource[] = [];

	constructor(
		view: CorpusView,
		options?: { missing?: MissingEntryPolicy; apply_reject_cache?: boolean }
	) {
		this.view = view;
		this.missing = options?.missing ?? 'fail';
		this.apply_reject_cache = options?.apply_reject_cache ?? true;
	}

	async *stream(logger: Logger = console.log): AsyncGenerator<SourceFile> {
		const tiers = TIERS_BY_VIEW[this.view];
		if (tiers.includes('live')) {
			// The one view holding the working trees is never LOADED: loading would walk
			// them whole, which is exactly what the diff exists to avoid, and would hand a
			// bench or gate unpinned files. The sweeps take seeds instead.
			throw new Error(
				`the \`${this.view}\` view holds the \`live\` tier, which is swept as a diff against ` +
					'the snapshot (`corpus_robustness_seeds`), never loaded whole'
			);
		}
		const entries = (await corpus_entries()).filter((e) => tiers.includes(e.tier));

		// Fail fast on missing entries — all existence checks up front, before
		// any file is yielded, so a partial corpus can't be half-processed. Which
		// absence is fatal is `this.missing`'s question, asked per entry so the
		// `{ complete_for }` policy can refuse an OPTIONAL entry (a pin's corpus is
		// not complete without it) while waving through a REQUIRED one that holds
		// none of its language.
		const complete_for = typeof this.missing === 'object' ? this.missing.complete_for : null;
		// Prefer an entry's own remedy (a harvest task for the derived caches);
		// otherwise a concrete `git clone` line for a known suite/framework checkout.
		// Shared by the tolerated line and the refusal, because a reader who is TOLD
		// about an absence wants the fix just as much as one who is stopped by it.
		const remedy_for = (entry: CorpusEntry): string | null =>
			entry.hint ?? clone_hint(entry_source(entry));
		const suffix = (remedy: string | null): string => (remedy ? ` (${remedy})` : '');
		const present: CorpusEntry[] = [];
		const missing: string[] = [];
		for (const entry of entries) {
			const entry_path = entry_source(entry);
			if (await fs_exists(resolve(entry_path))) {
				present.push(entry);
				continue;
			}
			const fatal =
				complete_for !== null
					? entry_holds_language(entry, complete_for)
					: this.missing === 'fail' && !entry.optional;
			if (!fatal) {
				// Each tolerated absence names WHY it was tolerated: `optional` is the
				// standing one, a `{ complete_for }` pass-through is a language claim,
				// and a `'tolerate'` pass-through is the explicit opt-in that makes the
				// run's numbers incomparable — the loudest of the three, and the one a
				// reader must not mistake for the standing case.
				const why = entry.optional
					? 'optional'
					: complete_for !== null
						? `holds no ${complete_for}`
						: 'PARTIAL CORPUS — BENCH_ALLOW_MISSING';
				logger(`  ⚠ corpus entry missing (${why}): ${entry_path}${suffix(remedy_for(entry))}`);
				continue;
			}
			missing.push(entry_path + suffix(remedy_for(entry)));
		}
		if (missing.length > 0) {
			throw new Error(
				`Missing corpus entr${missing.length === 1 ? 'y' : 'ies'} (${this.view} view): ` +
					`${missing.join(', ')} — clone the missing repo(s) or run the named harvest` +
					// The remedy a PIN grader must never be offered: tolerating the gap is
					// what makes the count wrong, so it is told to restore the input instead.
					(complete_for === null
						? ". Or opt into a partial corpus with `missing: 'tolerate'` " +
							'(BENCH_ALLOW_MISSING=1 for the bench).'
						: `. This run grades an exact ${complete_for} pin, so a partial corpus ` +
							'has no tolerant mode — it would report as a moved pin.')
			);
		}

		this.sources = [];
		logger(`Loading ${present.length} corpus paths (${this.view} view)`);

		// Conformance view: exclude the Svelte files svelte/compiler rejects (the
		// canonical-reject cache) so parse coverage measures fidelity on valid
		// Svelte, not permissiveness over the suite's error fixtures. Fail-open
		// when the cache is absent (pre-harvest), disclosed here.
		const apply_rejects = this.view === 'conformance' && this.apply_reject_cache;
		const reject_set = apply_rejects ? await load_svelte_reject_set() : null;
		if (apply_rejects) {
			logger(
				reject_set
					? `  canonical-reject cache: excluding ${reject_set.size} Svelte files rejected by svelte/compiler`
					: `  ⚠ canonical-reject cache absent — Svelte coverage counts svelte/compiler's rejects ` +
							`(run \`deno task bench:harvest:svelte-rejects\`)`
			);
		}
		let reject_excluded = 0;

		for (const entry of present) {
			const entry_path = entry_source(entry);
			const resolved_path = resolve(entry_path);
			let count = 0;
			const by_language: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
			if (entry.files_from !== undefined) {
				for await (const file of load_file_list(resolved_path, entry.extensions)) {
					count++;
					by_language[file.language]++;
					file.source = entry_path;
					yield file;
				}
			} else {
				const base_skip = entry.skip;
				// `reject_set` holds only Svelte paths (harvested Svelte-only), so a
				// bare membership test is inherently language-scoped.
				const skip: SkipFn | undefined = reject_set
					? async (path, relative) => {
							if (reject_set.has(path)) {
								reject_excluded++;
								return true;
							}
							return (await base_skip?.(path, relative)) ?? false;
						}
					: base_skip;
				for await (const file of walk_corpus(resolved_path, {
					extensions: entry.extensions,
					skip,
					prune_build_output: false
				})) {
					count++;
					by_language[file.language]++;
					file.source = entry_path;
					yield file;
				}
			}

			if (count > 0) {
				this.sources.push({ path: entry_path, files: count, by_language });
				logger(`  ${entry_path}: ${count} files`);
			}
		}

		if (reject_set && reject_excluded !== reject_set.size) {
			// Stale cache: it names more paths than this corpus still yields (fewer
			// hit than cached). Only detects cache-names-a-gone-path; a NEW reject the
			// corpus grew but the cache doesn't name isn't counted here (it's simply
			// not excluded) — a re-harvest, chained ahead of `bench:conformance`,
			// picks those up. Not fatal — disclose the drift so a re-harvest is
			// prompted for the `:run` (skip-harvest) path.
			logger(
				`  ⚠ canonical-reject cache drift: excluded ${reject_excluded} of ${reject_set.size} ` +
					`cached paths — re-run \`deno task bench:harvest:svelte-rejects\``
			);
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

/**
 * One view's files for ONE language, loaded under the posture an exact count pin
 * requires: `{ complete_for: language }`, so any absent entry that could hold that
 * language THROWS — `optional` ones included, since a pin is a claim about the
 * whole corpus and `optional` only says a normal run may proceed without it.
 *
 * This exists so the posture is not a thing each grader has to remember. Every
 * pinned-count grader does the same two steps — load the view, keep one language —
 * and the two that spelled them out separately BOTH got the tolerance wrong in the
 * same direction, tolerating an absent contributor and then reporting the shortfall
 * as a moved pin. Reach for this rather than `new CorpusLoader` whenever the
 * number that comes out is compared against a constant; the throw is the caller's
 * to translate (a `--if-present` warn-skip, usually).
 *
 * NOT a substitute for `corpus_missing_entries` in a STAMPED grade: a stamp is
 * consulted before anything loads, so "may I trust the stamp?" has to be asked
 * before this is ever called (see `diagnostics/css_over_acceptance.ts`).
 */
export async function load_pinned_language_corpus(
	view: CorpusView,
	language: Language,
	options?: { logger?: Logger; apply_reject_cache?: boolean }
): Promise<SourceFile[]> {
	const files = await new CorpusLoader(view, {
		missing: { complete_for: language },
		apply_reject_cache: options?.apply_reject_cache
	}).load(options?.logger ?? console.log);
	return files.filter((f) => f.language === language);
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
