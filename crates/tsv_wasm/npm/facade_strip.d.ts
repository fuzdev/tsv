/**
 * Types for the type-strip API the strip-capable tsv npm packages export
 * (`@fuzdev/tsv-wasm`, `@fuzdev/tsv`) — built by `api_strip.js`'s `create_strip_api`.
 * Declares the PUBLISHED function, not `api_strip.js`'s own export; named apart from it
 * for the reason `facade_format.d.ts` gives. Self-contained (no `tsv_ast.d.ts` import), so
 * the strip half carries no dependency on the parse half's declarations.
 */

/**
 * Options accepted by `strip_typescript`: the parse goal, read as `format_typescript` reads it.
 */
export interface StripOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier, and
	 * `import`/`export`/`import.meta` and a top-level `for await` are syntax errors. A
	 * script is also **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a module is
	 * always strict. The output is JS at the goal it was read at.
	 *
	 * Omitted, the source is read as a **module, retried as a script** if that parse
	 * fails — so a legacy sloppy script strips without naming a grammar, while anything
	 * the module grammar accepts is never reinterpreted. A set value is exact: `'module'`
	 * refuses a script-only source rather than retrying.
	 */
	sourceType?: 'script' | 'module' | undefined;
}

/** A source that stripped: the JS. */
export interface StripSuccess {
	ok: true;
	/**
	 * The JS: every type-only range blanked to whitespace (one space per UTF-16 unit, each
	 * line terminator inside it kept), plus at most one `;`, `(` or `)` per blanked range
	 * where blanking alone would let ASI or a line break change the parse. Every surviving
	 * character keeps its line and UTF-16 column, so `code.length === source.length`. A
	 * type-free source comes back as the same string.
	 */
	code: string;
}

/** A source that did not strip, and why. */
export interface StripFailure {
	ok: false;
	error: StripError;
}

/** What `strip_typescript` returns: it never throws for what it finds in the source. */
export type StripResult = StripSuccess | StripFailure;

/** Why a strip failed — `kind` says which. */
export type StripError = StripParseError | StripUnsupportedError;

/**
 * The source does not parse: exactly the `message`, `start` and `loc` the `TsvSyntaxError`
 * from `parse_typescript(source, {sourceType})` carries for the goal the error belongs to —
 * the named `sourceType`, or with none named the module's, unless the script retry reached
 * further into the source without stopping at a construct only a module holds (a top-level
 * `import` / `export`, `import.meta`, a top-level `await`), which makes it the script's.
 */
export interface StripParseError {
	kind: 'parse';
	/** The parse error's message: what went wrong, then `line:column + 1`, the line, a caret. */
	message: string;
	/** UTF-16 offset of the error (a leading BOM counted). */
	start: number;
	/** Line (1-based) and column (0-based, UTF-16 units) of `start`, ECMAScript's line terminators. */
	loc: { line: number; column: number };
}

/**
 * TypeScript that type blanking cannot erase — a construct with runtime semantics, or an
 * assertion whose blanking would regroup its expression — which the strip refuses rather
 * than transform. The first six are what `erasableSyntaxOnly` rejects.
 */
export type StripUnsupportedKind =
	/** A non-`declare` `enum`, `const enum` included. */
	| 'enum'
	/** A non-`declare` `namespace` holding a value member. */
	| 'value_namespace'
	/** `constructor(public x)` and the other accessibility / `readonly` / `override` modifiers on a constructor parameter. */
	| 'parameter_property'
	/** `import x = require('y')` / `import x = A.B`. */
	| 'import_equals'
	/** `export = x`. */
	| 'export_equals'
	/** An angle-bracket assertion, `<T>x`. */
	| 'type_assertion'
	/** An `as` / `satisfies` whose blanking would regroup the expression around it (`a + b as T / c`, `-a as T ** 2`); parenthesize it to keep it. */
	| 'assertion_grouping';

/** The source holds a construct the strip refuses; `start` / `end` / `loc` are its span. */
export interface StripUnsupportedError {
	kind: 'unsupported';
	construct: StripUnsupportedKind;
	/** `cannot strip <the construct>`, then `line:column + 1` of `start`, the line, a caret. */
	message: string;
	/** UTF-16 offsets of the refused span, in the parse wire's coordinates. */
	start: number;
	end: number;
	/** The span's line/column, as an acorn node's `loc` (1-based lines, 0-based UTF-16 columns). */
	loc: { start: { line: number; column: number }; end: { line: number; column: number } };
}

/**
 * Strip the types from TypeScript, keeping every surviving character at its line and
 * UTF-16 column.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key or a `sourceType` other than `'script'` or
 *   `'module'`
 * @throws Error when the source exceeds the 4 GiB size cap
 */
export declare function strip_typescript(source: string, options?: StripOptions): StripResult;
