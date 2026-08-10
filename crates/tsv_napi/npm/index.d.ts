/**
 * Hand-written types for `@fuzdev/tsv_napi` — mirrors `@fuzdev/tsv_wasm`'s
 * surface (same option interfaces, same overloads, same `tsv_ast` re-export)
 * minus the bench-only `parse_internal_*` family, so the two packages
 * type-check interchangeably.
 */
export type * from './tsv_ast';

/**
 * Options accepted by `parse_svelte` / `parse_css` (and their `_json`
 * siblings). The parse goal is TypeScript's alone, so it is declared here as
 * `undefined`-only rather than omitted: a set `goal` throws, but spelling the
 * inapplicable goal `undefined` forwards one bag to whichever parser, exactly
 * as the runtime does.
 */
export interface ParseOptions {
	/**
	 * Emit per-node `loc` (line/column) — the drop-in acorn/svelte wire.
	 * `false` emits the span-only wire (~46% smaller; Svelte also omits
	 * `name_loc`): `loc` stays derivable from `start`/`end` plus the source
	 * (`@fuzdev/tsv_parse_wasm` ships a pure-JS `reconstruct_locations` that
	 * works on this package's output too — the wire is identical). Inert for
	 * CSS (its wire has no `loc`).
	 * @default true
	 */
	locations?: boolean | undefined;
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `goal` throws. See `TypeScriptParseOptions`.
	 */
	goal?: undefined;
}

/** The TypeScript parsers' bag: the same keys, with `goal` settable. */
export interface TypeScriptParseOptions {
	/** As `ParseOptions.locations`. @default true */
	locations?: boolean | undefined;
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module' | undefined;
}

/**
 * Options accepted by `format_svelte` / `format_css`. Formatting itself is
 * non-configurable and the parse goal is TypeScript's alone, so these carry no
 * settable key. Every unknown key throws, `locations` included: that option
 * shapes the parse wire, and format emits no wire.
 */
export interface FormatOptions {
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `goal` throws. Declared (as `undefined`) rather than
	 * omitted so one bag still forwards to whichever formatter: spell the
	 * inapplicable goal `undefined` and this type accepts it, exactly as the
	 * runtime does.
	 */
	goal?: undefined;
}

/** The TypeScript formatter's bag: the same key, settable. */
export interface TypeScriptFormatOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module' | undefined;
}

export function parse_svelte(source: string, options: ParseOptions & { locations: false }): any;
export function parse_svelte(source: string, options?: ParseOptions): import('./tsv_ast').Root;
export function parse_svelte_json(source: string, options?: ParseOptions): string;

export function parse_typescript(
	source: string,
	options: TypeScriptParseOptions & { locations: false }
): any;
export function parse_typescript(
	source: string,
	options?: TypeScriptParseOptions
): import('./tsv_ast').Program;
export function parse_typescript_json(source: string, options?: TypeScriptParseOptions): string;

export function parse_css(
	source: string,
	options?: ParseOptions
): import('./tsv_ast').StyleSheetFile;
export function parse_css_json(source: string, options?: ParseOptions): string;

export function format_svelte(source: string, options?: FormatOptions): string;
export function format_typescript(source: string, options?: TypeScriptFormatOptions): string;
export function format_css(source: string, options?: FormatOptions): string;
