/**
 * Types for the import-records API the imports-capable tsv npm packages export
 * (`@fuzdev/tsv-wasm`, `@fuzdev/tsv`) — built by `api_imports.js`'s `create_imports_api`.
 * Declares the PUBLISHED functions, not `api_imports.js`'s own export; named apart from it
 * for the reason `facade_format.d.ts` gives. It imports only the shared error type every
 * package ships (`facade_error.d.ts`) — no `tsv_ast.d.ts` — so the imports half carries no
 * dependency on the parse half's declarations.
 */

import type { TsvParseError } from './facade_error.js';

/**
 * Options accepted by `parse_imports_typescript`: the parse goal, read as
 * `strip_typescript` reads it.
 */
export interface TypeScriptImportsOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier, and
	 * `import`/`export`/`import.meta` and a top-level `for await` are syntax errors — so a
	 * script's only records are dynamic imports and `import x = require('y')`, and its
	 * `hasModuleSyntax` is always `false`.
	 *
	 * Omitted, the source is read as a **module, retried as a script** if that parse
	 * fails — so a legacy sloppy script parses without naming a grammar, while anything
	 * the module grammar accepts is never reinterpreted. A set value is exact: `'module'`
	 * refuses a script-only source rather than retrying.
	 */
	sourceType?: 'script' | 'module' | undefined;
}

/**
 * Options accepted by `parse_imports_svelte`: none settable. `sourceType` is declared only
 * so a bag forwarded from a TypeScript call type-checks with it `undefined` — a set value is
 * refused, since a Svelte `<script>` is always a module.
 */
export interface ImportsOptions {
	sourceType?: undefined;
}

/**
 * The form of a module request: `'static'` (`import … from 'x'`, `import 'x'`), `'re_export'`
 * (`export … from 'x'`, `export * from 'x'`), `'dynamic'` (`import(…)`, `import.source(…)`,
 * `import.defer(…)`), or `'import_equals'` (TypeScript's `import x = require('x')`).
 */
export type ImportRecordKind = 'static' | 're_export' | 'dynamic' | 'import_equals';

/** A span of the source: UTF-16 code-unit offsets, as the parse wire's `start` / `end`. */
export interface ImportRecordSpan {
	start: number;
	end: number;
}

/** A request's specifier literal: its decoded value and the span of its contents. */
export interface ImportRecordSpecifier extends ImportRecordSpan {
	/**
	 * The specifier's value, decoded: a string literal's escapes, or a template literal's
	 * cooked value (a CR or CRLF read as LF) — the value the parse wire's literal carries.
	 */
	value: string;
}

/** One module request. */
export interface ImportRecord extends ImportRecordSpan {
	kind: ImportRecordKind;
	/**
	 * The specifier literal, its span the contents with the quotes (or backticks) excluded;
	 * `null` for a dynamic import whose specifier is computed — anything but a string literal
	 * or a template literal with no substitutions.
	 */
	specifier: ImportRecordSpecifier | null;
	/**
	 * Whether the request is for types alone: `'statement'` for `import type` /
	 * `export type` / `import type x = require('y')`, `'specifiers'` when every specifier is
	 * an inline `type` one (type stripping keeps `import {} from 'x'`, so the module still
	 * loads), `null` for a value request.
	 */
	typeOnly: 'statement' | 'specifiers' | null;
	/** `'source'` / `'defer'` for the source-phase and deferred imports; `null` otherwise. */
	phase: 'source' | 'defer' | null;
	/**
	 * A static import's or re-export's attributes clause, from `with` through its closing
	 * `}`, or a dynamic import's options argument; `null` when there is none.
	 */
	attributes: ImportRecordSpan | null;
}

/** A source that parsed: its module requests. */
export interface ImportsSuccess {
	ok: true;
	/**
	 * Whether the source uses syntax only a module parses: a top-level `import` or `export`
	 * declaration (type-only ones included), an `import.meta`, or a top-level `await` /
	 * `for await` / `await using` — the module-syntax core of what Node's ESM syntax
	 * detection reads. A dynamic `import(…)`, an `import x = require('y')`, an
	 * `export import x = …` and an `export =` are not module syntax.
	 */
	hasModuleSyntax: boolean;
	/**
	 * Every module request, in source order. Ambient code (`declare` declarations and
	 * bodies, overloads) holds none, nor does a type position.
	 */
	imports: ImportRecord[];
}

/** A source that did not parse. */
export interface ImportsFailure {
	ok: false;
	/**
	 * The error `parse_<lang>(source, {sourceType})` returns for the goal the error belongs
	 * to — with no `sourceType` named, the module's, unless the script retry reached further
	 * into the source without stopping at a construct only a module holds.
	 */
	error: TsvParseError;
}

/** What `parse_imports_<lang>` returns: it never throws for what it finds in the source. */
export type ImportsResult = ImportsSuccess | ImportsFailure;

/**
 * The module requests of TypeScript or JavaScript.
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key or a `sourceType` other than `'script'` or
 *   `'module'`
 * @throws Error when the source exceeds the 4 GiB size cap
 */
export declare function parse_imports_typescript(
	source: string,
	options?: TypeScriptImportsOptions
): ImportsResult;

/**
 * The module requests of a Svelte component: both `<script>` blocks, in source order, every
 * offset into the document (a leading byte-order mark removed, as the Svelte parse wire
 * removes it).
 * @throws TypeError when `source` is not a well-formed UTF-16 string, `options` is not an
 *   object, or `options` carries an unknown key or a set `sourceType`
 * @throws Error when the source exceeds the 4 GiB size cap
 */
export declare function parse_imports_svelte(
	source: string,
	options?: ImportsOptions
): ImportsResult;
