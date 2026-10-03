/**
 * Types for the format API every tsv npm package exports, and the options bags the
 * whole facade shares the conventions of (`api.js`; the parse half is `api_parse.d.ts`).
 *
 * These declare the PUBLISHED functions — what each package entry re-exports by name
 * from `create_format_api`'s result — not `api.js`'s own module exports, which are
 * the entries' plumbing and reachable through no package `exports` path. Hand-written, staged into
 * every package beside `api.js`.
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
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors. A script is also **sloppy**
	 * unless its own `"use strict"` directive prologue makes it strict, so `with` and
	 * the legacy octal literals/escapes parse there; a module is always strict.
	 *
	 * Omitted, the source is formatted as a **module, retried as a script** if that
	 * parse fails — so a legacy sloppy script formats without naming a grammar, while
	 * anything the module grammar accepts is never reinterpreted (the printer does not
	 * read the goal, so no output changes). A set value is exact: `'module'` refuses a
	 * script-only source rather than retrying. `parse_typescript` has no such fallback —
	 * its wire's `Program.sourceType` is a claim, and omitting the key there means
	 * `'module'`.
	 */
	sourceType?: 'script' | 'module' | undefined;
}

/** Format a Svelte component. */
export declare function format_svelte(source: string, options?: FormatOptions): string;
/** Format TypeScript (or JavaScript). */
export declare function format_typescript(
	source: string,
	options?: TypeScriptFormatOptions
): string;
/** Format CSS. */
export declare function format_css(source: string, options?: FormatOptions): string;
