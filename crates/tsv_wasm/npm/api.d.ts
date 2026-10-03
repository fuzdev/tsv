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
 * What every parse and format export throws when the source does not parse: a
 * `SyntaxError` whose own enumerable properties are `start` and then `loc`, the error's
 * position in the same coordinates as the parse wire's own `start` / `end`.
 *
 * - `start` — the UTF-16 code-unit offset a wire node at the error would carry: into the
 *   caller's source for TypeScript, and into the source with a leading byte-order mark
 *   removed for Svelte and CSS (their wires elide it, as Svelte's `parse` and `parseCss`
 *   do). On a format too the offset is into the caller's own source — the formatter's
 *   line-ending normalization never shows through — so a format and a parse report the
 *   same `start` for the same error. Not always the same error: with no `sourceType`
 *   named, `format_typescript` retries a failed module parse as a script and can report
 *   that attempt's error where a module parse reports its own.
 * - `loc` — `{line, column}` at `start`: `line` 1-based, `column` 0-based in UTF-16 code
 *   units, under the document's line rule — ECMAScript's line terminators (LF, CR, CRLF,
 *   U+2028, U+2029) for TypeScript, LF alone for a Svelte document (its `<script>`,
 *   `<style>` and template expressions included) and for CSS. Always equal to
 *   `create_locator(source, {language}).position_at(start)` where the package exports it.
 *
 * The message is `<what went wrong>`, then a line `<line>:<column> <source line>` whose
 * header is `loc.line:loc.column + 1`, then a caret under the error's character.
 *
 * Every argument refusal — a source that is not a string, a bad options bag — is a
 * `TypeError` instead, and nothing else an engine throws is a `SyntaxError` either: a
 * plain `Error` (a source over the 4 GiB size cap, an internal failure), a
 * `WebAssembly.RuntimeError` (a WASM trap) or a `RangeError` (stack exhaustion). So
 * `instanceof SyntaxError` asks exactly "did the source fail to parse".
 *
 * Read `start` and `loc`, not `line` / `column`: some runtimes put numeric `line` and
 * `column` of their own on every `Error`.
 */
export interface TsvSyntaxError extends SyntaxError {
	/** UTF-16 offset of the error, in the parse wire's coordinates. */
	start: number;
	/** Line (1-based) and column (0-based, UTF-16 units) of `start`. */
	loc: { line: number; column: number };
}

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

/**
 * Format a Svelte component.
 * @throws {TsvSyntaxError} when the source does not parse
 * @throws {TypeError} when an argument is refused
 */
export declare function format_svelte(source: string, options?: FormatOptions): string;
/**
 * Format TypeScript (or JavaScript).
 * @throws {TsvSyntaxError} when the source does not parse
 * @throws {TypeError} when an argument is refused
 */
export declare function format_typescript(
	source: string,
	options?: TypeScriptFormatOptions
): string;
/**
 * Format CSS.
 * @throws {TsvSyntaxError} when the source does not parse
 * @throws {TypeError} when an argument is refused
 */
export declare function format_css(source: string, options?: FormatOptions): string;
