import {
	format_css as _format_css,
	format_svelte as _format_svelte,
	format_typescript as _format_typescript,
	parse_css as _parse_css,
	parse_internal_css as _parse_internal_css,
	parse_internal_svelte as _parse_internal_svelte,
	parse_internal_typescript as _parse_internal_typescript,
	parse_svelte as _parse_svelte,
	parse_typescript as _parse_typescript,
} from './pkg/deno/tsv_wasm.js';

/** Format Svelte source code. Returns formatted output matching Prettier's style. */
export const format_svelte: (source: string) => string = _format_svelte;

/** Format TypeScript source code. Returns formatted output matching Prettier's style. */
export const format_typescript: (source: string) => string = _format_typescript;

/** Format CSS source code. Returns formatted output matching Prettier's style. */
export const format_css: (source: string) => string = _format_css;

/** Parse Svelte source code and return the AST as a JavaScript object. */
export const parse_svelte: (source: string) => any = _parse_svelte;

/** Parse TypeScript source code and return the AST as a JavaScript object. */
export const parse_typescript: (source: string) => any = _parse_typescript;

/** Parse CSS source code and return the AST as a JavaScript object. */
export const parse_css: (source: string) => any = _parse_css;

/** Parse Svelte to internal AST only (for benchmarking). */
export const parse_internal_svelte: (source: string) => void = _parse_internal_svelte;

/** Parse TypeScript to internal AST only (for benchmarking). */
export const parse_internal_typescript: (source: string) => void = _parse_internal_typescript;

/** Parse CSS to internal AST only (for benchmarking). */
export const parse_internal_css: (source: string) => void = _parse_internal_css;
