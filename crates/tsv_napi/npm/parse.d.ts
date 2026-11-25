/**
 * Hand-written types for `@fuzdev/tsv/parse` — `@fuzdev/tsv-wasm/parse`'s surface: the parse
 * family (re-exported from the staged `facade_parse.d.ts`), the AST types it returns (the
 * generated `tsv_ast` re-export, which lives on this entry alone), the `locations.js` helper's
 * functions and types, and the error types a failed result carries.
 *
 * Relative specifiers carry the **`.js`** extension, as in `index.d.ts`.
 */
export type * from './tsv_ast.js';
// `export *`, not `export type *` — the helper's functions AND its types flow through.
export * from './locations.js';

// Re-exported by NAME, so the `tsv_ast` star export above can never ambiguate one away
// (TS2308).
export type {
	ParseOptions,
	TypeScriptParseOptions,
	ParseJsonOptions,
	TypeScriptParseJsonOptions,
	ParseSuccess,
	ParseJsonSuccess,
	ParseFailure,
	ParseResult,
	ParseJsonResult
} from './facade_parse.js';
export type { TsvError, TsvParseError } from './facade_error.js';
export {
	parse_svelte,
	parse_svelte_json,
	parse_typescript,
	parse_typescript_json,
	parse_css,
	parse_css_json
} from './facade_parse.js';
