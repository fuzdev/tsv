/**
 * Shared types for benchmark infrastructure
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
	css: '.css',
};

/** Prettier parser names for each language */
export const LANGUAGE_PRETTIER_PARSERS: Record<Language, string> = {
	svelte: 'svelte',
	typescript: 'typescript',
	css: 'css',
};


/** A source file loaded into memory for benchmarking */
/**
 * The TypeScript/JS parse goal (`sourceType`). Only test262 fixtures carry a
 * non-default goal — a `flags: [module]` test is `module`, everything else is a
 * strict `script` (where `await` is an ordinary identifier and top-level
 * `import`/`export` are errors). Every other corpus is module (Svelte `<script>`
 * and real TS), so `SourceFile.goal` is left undefined there and treated as
 * `module`. Threaded ONLY through the conformance-coverage preflight so that
 * corpus scores each tool on the goal test262 declares — see
 * `benches/js/CLAUDE.md` §Conformance-surface semantics.
 */
export type ParseGoal = 'script' | 'module';

export interface SourceFile {
	/** Absolute path to the file */
	path: string;
	/** File content (pre-loaded) */
	content: string;
	/** Detected language based on extension */
	language: Language;
	/** Size in bytes */
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
	 * The declared parse goal (test262 only; undefined = `module`). The
	 * conformance preflight parses each tool at this goal so a script-goal
	 * `await`-identifier test isn't scored as a failure against a module parse.
	 */
	goal?: ParseGoal;
}

/** Implementation names for benchmarking */
export type ImplementationName =
	| 'canonical'
	| 'native'
	| 'napi'
	| 'wasm'
	| 'oxc'
	| 'oxc-wasm'
	| 'biome-wasm'
	| 'dprint-wasm';

/** Common interface for parser/formatter implementations */
export interface TsvImplementation {
	name: ImplementationName;

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
