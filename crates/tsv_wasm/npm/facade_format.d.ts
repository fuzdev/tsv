/**
 * Types for the format API every format-capable tsv npm package exports, and the
 * conventions every options bag of the facade follows (the parse half's declarations are
 * `facade_parse.d.ts`).
 *
 * These declare the PUBLISHED functions — what each package entry re-exports by name
 * from `create_format_api`'s result (`api.js`) — not `api.js`'s own module exports, which
 * are the entries' plumbing and reachable through no package `exports` path. That is why
 * this file is not named `api.d.ts`: a declaration file beside a `.js` of the same
 * basename is read as that module's types, so under a resolver that ignores `exports`
 * (`moduleResolution: node10`) a deep import of `api.js` would type-check as these
 * functions and be `undefined` at runtime. Hand-written; staged into every package that
 * formats.
 *
 * Every option key spells `| undefined` on top of `?`. That is not redundant: under a
 * consumer's `exactOptionalPropertyTypes` a bare `?` accepts an ABSENT key but rejects
 * one explicitly set to `undefined` — and setting it to `undefined` is the documented
 * forwarding idiom (`npm/cli.js` builds `{sourceType: <maybe undefined>}` and hands it
 * to whichever export).
 *
 * The non-TypeScript bags declare `sourceType?: undefined` rather than omitting the key
 * or being `{}`: an empty interface opts out of excess-property and weak-type checking,
 * so it would accept every non-nullish value the runtime refuses; and an omitted key
 * would reject the forwarded `{sourceType: undefined}` the runtime accepts. The
 * TypeScript bags are standalone rather than `extends` of their base, since a settable
 * `sourceType` is incompatible with the `undefined`-only one.
 */

/**
 * Options accepted by `format_svelte` / `format_css`. Formatting itself is
 * non-configurable and the parse goal is TypeScript's alone, so these carry no
 * settable key. Every unknown key throws, `locations` included: format emits no wire.
 */
export interface FormatOptions {
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no goal,
	 * so a set `sourceType` throws. Declared (as `undefined`) rather than omitted so one
	 * bag still forwards to whichever formatter.
	 */
	sourceType?: undefined;
}

/** The TypeScript formatter's bag: the same key, settable. */
export interface TypeScriptFormatOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier, and
	 * `import`/`export`/`import.meta` and a top-level `for await` are syntax errors. A
	 * script is also **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a module is
	 * always strict.
	 *
	 * Omitted, the source is formatted as a **module, retried as a script** if that
	 * parse fails — so a legacy sloppy script formats without naming a grammar, while
	 * anything the module grammar accepts is never reinterpreted (the printer does not
	 * read the goal, so no output changes). A set value is exact: `'module'` refuses a
	 * script-only source rather than retrying. `parse_typescript` has no such fallback:
	 * omitting the key there means `'module'`.
	 */
	sourceType?: 'script' | 'module' | undefined;
}

/**
 * Format a Svelte component.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key or a set `sourceType`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 * @throws Error when the source exceeds the 4 GiB size cap, or parses but cannot be
 *   formatted faithfully — a lone CR inside an in-tag `//` comment, which the format's CR
 *   fold would end early
 */
export declare function format_svelte(source: string, options?: FormatOptions): string;
/**
 * Format TypeScript (or JavaScript).
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key or a `sourceType` other than `'script'` or
 *   `'module'`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 * @throws Error when the source exceeds the 4 GiB size cap
 */
export declare function format_typescript(
	source: string,
	options?: TypeScriptFormatOptions
): string;
/**
 * Format CSS.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key or a set `sourceType`
 * @throws SyntaxError when the source does not parse — a `TsvSyntaxError`, its position on
 *   `start` and `loc`
 * @throws Error when the source exceeds the 4 GiB size cap
 */
export declare function format_css(source: string, options?: FormatOptions): string;
