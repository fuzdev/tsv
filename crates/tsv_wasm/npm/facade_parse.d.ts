/**
 * Types for the parse API every parse-capable tsv npm package exports (built by
 * `api_parse.js`'s `create_parse_api`). Every bag here follows the facade's conventions:
 * each key spells
 * `| undefined` on top of `?` (so a consumer's `exactOptionalPropertyTypes` still accepts
 * the forwarded `{sourceType: undefined}`), the non-TypeScript bags declare
 * `sourceType?: undefined` rather than omitting it (an empty interface would accept every
 * value the runtime refuses), and the TypeScript bags are standalone rather than
 * `extends` of their base.
 *
 * These declare the PUBLISHED functions — what each package entry re-exports by name
 * from `create_parse_api`'s result — not `api_parse.js`'s own export
 * (`create_parse_api`), which is the entries' plumbing; named apart from it, as the
 * format declarations are from `api.js`, so no resolver reads this file as that module's
 * types. Hand-written, staged into every package that parses.
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
	 * Parse goal: at `'script'`, `await` is an ordinary identifier, and
	 * `import`/`export`/`import.meta` and a top-level `for await` are syntax errors. A
	 * script is also **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a module is
	 * always strict.
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
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key, a non-boolean `locations` or a set
 *   `sourceType`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 */
export declare function parse_svelte(
	source: string,
	options?: ParseOptions
): import('./tsv_ast.js').Root;
/**
 * Parse a Svelte component into its span-only wire, as a compact JSON string.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries `locations`, an unknown key or a set `sourceType`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 */
export declare function parse_svelte_json(source: string, options?: ParseJsonOptions): string;

/**
 * Parse TypeScript (or JavaScript) into its ESTree AST — the span-only tree
 * (`start`/`end` offsets); `{locations: true}` adds `loc`.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key, a non-boolean `locations` or a
 *   `sourceType` other than `'script'` or `'module'`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 */
export declare function parse_typescript(
	source: string,
	options?: TypeScriptParseOptions
): import('./tsv_ast.js').Program;
/**
 * Parse TypeScript into its span-only wire, as a compact JSON string.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries `locations`, an unknown key or a `sourceType` other than
 *   `'script'` or `'module'`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 */
export declare function parse_typescript_json(
	source: string,
	options?: TypeScriptParseJsonOptions
): string;

/**
 * Parse CSS into its AST — the span-only tree (`start`/`end` offsets);
 * `{locations: true}` adds `loc`.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key, a non-boolean `locations` or a set
 *   `sourceType`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 */
export declare function parse_css(
	source: string,
	options?: ParseOptions
): import('./tsv_ast.js').StyleSheetFile;
/**
 * Parse CSS into its span-only wire, as a compact JSON string.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries `locations`, an unknown key or a set `sourceType`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 */
export declare function parse_css_json(source: string, options?: ParseJsonOptions): string;
