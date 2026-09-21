/**
 * Size-bundle entry: the canonical FULL toolchain — parse + format, the scope of
 * `tsv-wasm`. One bundle rather than a sum of the other two, because the formatter
 * already carries the Svelte parser and the bundler shares it.
 *
 * Never executed by the harness — see `canonical_format.ts`.
 */

export * from './canonical_format.ts';
export * from './canonical_parse.ts';
