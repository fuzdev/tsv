/**
 * Types for the parse API every parse-capable tsv npm package exports
 * (`api_parse.js`). The conventions every bag here follows — `| undefined` on each key,
 * `sourceType?: undefined` on the non-TypeScript bags, no `extends` — are stated in
 * `api.d.ts`.
 *
 * These declare the PUBLISHED functions — what each package entry re-exports by name
 * from `create_parse_api`'s result — not `api_parse.js`'s own export
 * (`create_parse_api`), which is the entries' plumbing. Hand-written, staged beside
 * `api_parse.js` into every package that parses.
 *
 * Relative specifiers carry the **`.js`** extension, as in every shipped `.d.ts`: under
 * `moduleResolution: node16`/`nodenext` an extensionless one is TS2834 at the consumer,
 * and TypeScript resolves `./tsv_ast.js` to `./tsv_ast.d.ts`.
 */

/**
 * Options accepted by `parse_svelte` / `parse_css`. The parse goal is TypeScript's
 * alone, so a set `sourceType` throws; spelling it `undefined` forwards one bag to
 * whichever parser, exactly as the runtime does.
 */
export interface ParseOptions {
	/**
	 * Add per-node `loc` (line/column) — and Svelte's `name_loc` — to the span-only
	 * tree, reconstructed in JS from the offsets plus the source by this package's
	 * `reconstruct_locations` (TypeScript's matches acorn's `locations: true` exactly).
	 * @default false
	 */
	locations?: boolean | undefined;
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no goal,
	 * so a set `sourceType` throws. See `TypeScriptParseOptions`.
	 */
	sourceType?: undefined;
}

/** The TypeScript parser's bag: the same keys, with `sourceType` settable. */
export interface TypeScriptParseOptions {
	/** As `ParseOptions.locations`. @default false */
	locations?: boolean | undefined;
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors. A script is also **sloppy**
	 * unless its own `"use strict"` directive prologue makes it strict, so `with` and
	 * the legacy octal literals/escapes parse there; a module is always strict.
	 * @default 'module'
	 */
	sourceType?: 'script' | 'module' | undefined;
}

/**
 * Options accepted by `parse_svelte_json` / `parse_css_json`: none settable. The JSON
 * string is the wire itself, so there is no `locations` (`loc` is a view over objects —
 * the key throws a `TypeError` pointing at the object parser); `sourceType` is declared
 * `undefined` so a forwarded bag still type-checks.
 */
export interface ParseJsonOptions {
	/** Not accepted here — see `ParseOptions.sourceType`. */
	sourceType?: undefined;
}

/** `parse_typescript_json`'s bag: the parse goal, as `TypeScriptParseOptions.sourceType`. */
export interface TypeScriptParseJsonOptions {
	/** As `TypeScriptParseOptions.sourceType`. @default 'module' */
	sourceType?: 'script' | 'module' | undefined;
}

/**
 * Parse a Svelte component into its AST — the span-only tree (`start`/`end` offsets);
 * `{locations: true}` adds `loc` / `name_loc`.
 */
export declare function parse_svelte(
	source: string,
	options?: ParseOptions
): import('./tsv_ast.js').Root;
/** Parse a Svelte component into its span-only wire, as a compact JSON string. */
export declare function parse_svelte_json(source: string, options?: ParseJsonOptions): string;

/**
 * Parse TypeScript (or JavaScript) into its ESTree AST — the span-only tree
 * (`start`/`end` offsets); `{locations: true}` adds `loc`.
 */
export declare function parse_typescript(
	source: string,
	options?: TypeScriptParseOptions
): import('./tsv_ast.js').Program;
/** Parse TypeScript into its span-only wire, as a compact JSON string. */
export declare function parse_typescript_json(
	source: string,
	options?: TypeScriptParseJsonOptions
): string;

/**
 * Parse CSS into its AST — the span-only tree (`start`/`end` offsets);
 * `{locations: true}` adds `loc`.
 */
export declare function parse_css(
	source: string,
	options?: ParseOptions
): import('./tsv_ast.js').StyleSheetFile;
/** Parse CSS into its span-only wire, as a compact JSON string. */
export declare function parse_css_json(source: string, options?: ParseJsonOptions): string;
