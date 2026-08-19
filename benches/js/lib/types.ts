/**
 * Shared types for benchmark infrastructure, plus the small `BaseImplementation`
 * every impl wrapper extends (the one piece of behavior all of them had verbatim).
 */

/** Logger function type */
export type Logger = (...args: unknown[]) => void;

/** Supported source file languages */
export type Language = 'svelte' | 'typescript' | 'css';

/** All supported languages as an array */
export const LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

/** File extensions for each language */
export const LANGUAGE_EXTENSIONS: Record<Language, string> = {
	svelte: '.svelte',
	typescript: '.ts',
	css: '.css'
};

/** Prettier parser names for each language */
export const LANGUAGE_PRETTIER_PARSERS: Record<Language, string> = {
	svelte: 'svelte',
	typescript: 'typescript',
	css: 'css'
};

/**
 * The canonical PARSER's row name per language — the oracle every parse row is
 * measured against, and the per-group table's baseline. CSS shares svelte's entry
 * because the oracle there is svelte's own `parseCss`.
 *
 * Here rather than beside either consumer: `lib/implementations.ts` registers the
 * row under this name and `lib/report.ts` looks the row up by it, and those two
 * cannot import from each other (implementations.ts already `import type`s report
 * types, so a value import back would close a runtime cycle). As two spellings
 * they were a rename away from the report silently failing to find the baseline
 * row it names.
 */
export const CANONICAL_PARSER_ROWS: Record<Language, string> = {
	svelte: 'svelte/compiler',
	typescript: 'acorn-typescript',
	css: 'svelte/compiler'
};

/** The canonical FORMATTER's row name — one tool for every language. */
export const CANONICAL_FORMATTER_ROW = 'prettier';

/**
 * The TypeScript/JS parse goal (`sourceType`). Only test262 fixtures carry a
 * non-default goal — a `flags: [module]` test is `module`, everything else is a
 * strict `script` (where `await` is an ordinary identifier and top-level
 * `import`/`export` are errors). Every other corpus is module (Svelte `<script>`
 * and real TS), so `SourceFile.goal` is left undefined there and treated as
 * `module`. Threaded ONLY through the conformance-coverage preflight so that
 * corpus scores each tool on the goal test262 declares — see
 * `docs/benchmarks.md` §Fairness caveats (Conformance-surface semantics).
 */
export type ParseGoal = 'script' | 'module';

/** A source file loaded into memory for benchmarking. */
export interface SourceFile {
	/** Absolute path to the file */
	path: string;
	/** File content (pre-loaded) */
	content: string;
	/** Detected language based on extension */
	language: Language;
	/**
	 * UTF-8 size — the denominator of every MB/s figure, so it must be BYTES and
	 * not `content.length` (UTF-16 code units, which under-counts every non-ASCII
	 * file). Measured with `Buffer.byteLength`, which computes the length without
	 * materializing the encoded copy a `TextEncoder().encode(…).length` would
	 * allocate for each of the corpus's thousands of files.
	 */
	bytes: number;
	/**
	 * True when this file comes from a version-pinned, `pins:audit`-tracked
	 * checkout (the `framework` + `prettier_fixture` tiers) rather than a live dev
	 * repo. The format gate's count pins (match/unknown/partial) are enforced over
	 * the reproducible subset only, so live-repo churn can't shift them; SAFETY
	 * still gates over every file. Set by `DevReposLoader`; undefined for a
	 * `DirectoryLoader` single-repo run (which isn't gated). See `lib/corpus.ts`.
	 */
	reproducible?: boolean;
	/**
	 * Which `CORPUS_ENTRIES` entry this file came from (its `path`/`files_from`,
	 * project-root-relative) — the key the conformance report's per-source coverage
	 * breakdown groups by, so an entry whose reading is special (the tsc corpus,
	 * where `tsc` is the oracle rather than a competitor) can be read on its own
	 * instead of averaged into the group. Set by `DevReposLoader`; undefined for a
	 * `DirectoryLoader` single-repo run.
	 */
	source?: string;
	/**
	 * The declared parse goal (test262 only; undefined = `module`). The
	 * conformance preflight parses each tool at this goal so a script-goal
	 * `await`-identifier test isn't scored as a failure against a module parse.
	 */
	goal?: ParseGoal;
}

/** Common interface for parser/formatter implementations */
export interface TsvImplementation {
	/** Initialize the implementation (load WASM, open FFI library, etc.) */
	init(): Promise<void>;

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean;

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean;

	/**
	 * Parse source and return AST (as object or JSON string). `goal` (TS only;
	 * default `module`) selects the parse goal for the conformance surface's
	 * test262 files; ignored for svelte/css and by tools without a goal axis.
	 */
	parse(source: string, language: Language, goal?: ParseGoal): unknown;

	/** Parse source without JSON serialization (native/wasm only, for measuring pure parse speed) */
	parse_internal?(source: string, language: Language, goal?: ParseGoal): void;

	/**
	 * Parse source dropping per-node `loc` (the span-only `no-locations` wire) —
	 * the payload-matched comparison against oxc-parser's span-only default AST.
	 * Native/wasm only; TypeScript + Svelte only (CSS emits no `loc`).
	 */
	parse_no_locations?(source: string, language: Language, goal?: ParseGoal): unknown;

	/** Format source synchronously (native, wasm) */
	format?(source: string, language: Language): string;

	/** Format source asynchronously (canonical/prettier) */
	format_async?(source: string, language: Language): Promise<string>;

	/** Clean up resources */
	dispose(): void;
}

/**
 * Shared base for every impl wrapper: language support declared as DATA rather
 * than re-implemented as a predicate pair per class.
 *
 * Each wrapper states which languages it can parse and which it can format; `[]`
 * means "none", which is how a parse-only tool (yuku, oxc's wasm binding) and a
 * format-only one (biome, dprint, rsvelte-fmt) say so. Every class previously
 * carried a `static PARSE_LANGUAGES`/`FORMAT_LANGUAGES` pair plus two one-line
 * `.includes()` methods, and the "none" case had drifted into two spellings — an
 * empty array in most, a hand-rolled `return false` in others — so the same fact
 * read two different ways depending on which file you opened.
 *
 * Deliberately minimal. It holds ONLY what was identical everywhere; how a binding
 * loads, what it does per call, and its fairness corrections stay in the wrapper,
 * where the differences that matter live. A wrapper needing different support
 * logic can still override the two methods.
 */
export abstract class BaseImplementation implements TsvImplementation {
	/** Languages this impl can parse; `[]` for a format-only tool. */
	abstract readonly parse_languages: ReadonlyArray<Language>;

	/** Languages this impl can format; `[]` for a parse-only tool. */
	abstract readonly format_languages: ReadonlyArray<Language>;

	supports_parse_language(language: Language): boolean {
		return this.parse_languages.includes(language);
	}

	supports_format_language(language: Language): boolean {
		return this.format_languages.includes(language);
	}

	abstract init(): Promise<void>;

	abstract parse(source: string, language: Language, goal?: ParseGoal): unknown;

	abstract dispose(): void;
}
