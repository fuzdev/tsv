/**
 * The error every parse and format export of a tsv npm package throws for a source that
 * does not parse. Types only — the error itself is built by the facade (`api.js`) — and
 * its own file so every package ships it, the format-only and parse-only ones alike,
 * beside whichever family's declarations they carry.
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
 * The message is `<what went wrong>`, then a line `<line>:<column> <excerpt>` whose
 * header is `loc.line:loc.column + 1`, then a caret under the error's character. The
 * excerpt is the physical line around the error, bounded by ANY line terminator (LF, CR,
 * U+2028, U+2029), so a raw CR never reaches a terminal. For a Svelte or CSS document,
 * whose line rule is LF alone, that is narrower than the document line the header counts
 * in: the excerpt can start after a lone CR or U+2028 that the header's line count does
 * not break on, so the header's column counts from the start of the document line, not
 * from the start of the excerpt.
 *
 * Every argument refusal — a source that is not a well-formed UTF-16 string, a bad
 * options bag — is a `TypeError` instead, and nothing else an engine throws is a
 * `SyntaxError` either: a
 * plain `Error` (a source over the 4 GiB size cap, a format's refusal of a source that
 * parses — a lone CR inside a Svelte in-tag `//` comment, which the format's CR fold would
 * end early — an internal failure), a
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
