/**
 * The errors every parse, format and strip export of a tsv npm package returns: their
 * shared shape `TsvError`, and the parse failure every export returns for a source that does
 * not parse. Types only — the error itself is built by the
 * facade (`api.js`) — and its own file so every package ships it, the format-only and
 * parse-only ones alike, beside whichever family's declarations they carry (each imports it
 * from here).
 */

/**
 * What every error a tsv export RETURNS carries, whatever its `kind` — the `error` of a
 * `{ok: false}` result. A plain object (no stack, structured-clone-able, so a worker can post
 * it) whose span is shaped like a wire node's, in the same coordinates as the parse wire's own
 * `start` / `end`. Each kind extends it: `TsvParseError`, `FormatRefusedError`,
 * `StripUnsupportedError`.
 *
 * Every argument refusal — a source that is not a well-formed UTF-16 string, a bad options
 * bag — is a thrown `TypeError` instead, and so is no result at all; nor is an engine
 * failure with no point, which throws too: a plain `Error` (a source over the 4 GiB size
 * cap, an internal failure), a `WebAssembly.RuntimeError` (a WASM trap) or a `RangeError`
 * (stack exhaustion).
 */
export interface TsvError {
	/** Which failure: narrowed by each error type that extends this one. */
	kind: 'parse' | 'refused' | 'unsupported';
	/**
	 * What went wrong, on one line (`Expected ';', found 'y'`). A line terminator it quotes from
	 * the source (LF, CR, U+2028, U+2029) is spelled `<U+000A>` and so on, so it never breaks
	 * the line.
	 */
	message: string;
	/**
	 * Where, as the native CLI prints it under `message`: a line `<line>:<column> <excerpt>`
	 * whose header is `loc.start.line:loc.start.column + 1`, then a caret under the error's
	 * character — so `${message}\n${frame}` is the CLI's text. The excerpt is the physical
	 * line around the error, bounded by ANY line terminator (LF, CR, U+2028, U+2029), so a raw
	 * CR never reaches a terminal. For a Svelte or CSS document, whose line rule is LF alone,
	 * that is narrower than the document line the header counts in: the excerpt can start
	 * after a lone CR or U+2028 that the header's line count does not break on, so the
	 * header's column counts from the start of the document line, not from the start of the
	 * excerpt.
	 */
	frame: string;
	/**
	 * The UTF-16 code-unit offset a wire node at the error would carry: into the caller's
	 * source for TypeScript, and into the source with a leading byte-order mark removed for
	 * Svelte and CSS (their wires elide it, as Svelte's `parse` and `parseCss` do). On a
	 * format too the offset is into the caller's own source — the formatter's line-ending
	 * normalization never shows through.
	 */
	start: number;
	/** Where the error's span ends, in `start`'s coordinates — each kind says what it covers. */
	end: number;
	/**
	 * `start` and `end` as a wire node's `loc`: `line` 1-based, `column` 0-based in UTF-16
	 * code units, under the document's line rule — ECMAScript's line terminators (LF, CR,
	 * CRLF, U+2028, U+2029) for TypeScript, LF alone for a Svelte document (its `<script>`,
	 * `<style>` and template expressions included) and for CSS. Always deep-equal to
	 * `create_locator(source, {language}).loc_of(error)` where the package exports it.
	 */
	loc: { start: { line: number; column: number }; end: { line: number; column: number } };
}

/**
 * The source does not parse — returned by every parse, format and strip export. A format
 * reports the error at the same `start` and `loc` a parse of the same source does — though
 * its `message` can differ where it quotes source text, which a format reads with line
 * endings folded to LF — with one exception: with no `sourceType` named, `format_typescript`
 * and `strip_typescript` retry a failed module parse as a script and can report that
 * attempt's error where a module parse reports its own.
 */
export interface TsvParseError extends TsvError {
	kind: 'parse';
	/** Equal to `start`: a parse error is a point, not a span. */
	end: number;
}
