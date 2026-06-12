# @fuzdev/tsv_parse_wasm

> parser for TypeScript, Svelte, and CSS

Rust-based parser compiled to WASM. Drop-in replacement for **Svelte's parser** + **acorn** + **acorn-typescript**.

Parsing only — for formatting, see [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm), or [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) for both plus a CLI.

Source of truth, full docs, and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv_parse_wasm
```

## Usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {parse_css, parse_svelte, parse_typescript} from '@fuzdev/tsv_parse_wasm';
import type {Program, Root, StyleSheetFile} from '@fuzdev/tsv_parse_wasm';

const root: Root = parse_svelte('<script>const x = 1;</script>');
const program: Program = parse_typescript('const x: number = 1;');
const stylesheet: StyleSheetFile = parse_css('a { color: red }');
```

Three parsers: `parse_svelte` (matches Svelte's modern parser), `parse_typescript` (matches acorn + acorn-typescript), `parse_css` (matches Svelte's `parseCss`). Each takes a source `string` and returns a Svelte-compatible JSON AST, throwing on a parse error.

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

Each parser also has a `parse_*_json` variant (`parse_svelte_json`, `parse_typescript_json`, `parse_css_json`) returning the AST as a compact JSON string — faster when you're writing it to disk or sending it over the wire, since it skips materializing the JS object tree.

## Status

v0.1 — pre-release. API may change.

## License

[MIT](LICENSE)
