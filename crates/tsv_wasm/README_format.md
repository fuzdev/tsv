# @fuzdev/tsv_format_wasm

> formatter for Svelte, TypeScript, and CSS

Rust-based formatter compiled to WASM. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely, with documented divergences.

Formatting only — for parser / AST extraction, see [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm), or [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) for both plus a CLI.

Source of truth, full docs, and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv_format_wasm
```

## Usage

Three formatting functions: `format_svelte`, `format_typescript`, `format_css`. Each takes a source `string` and returns the formatted `string`, throwing on a parse error.

### Node.js, Bun, Deno

Zero config — WASM is initialized synchronously at import time:

```javascript
import {format_css, format_svelte, format_typescript} from '@fuzdev/tsv_format_wasm';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

### Browsers and bundlers

Call `await init()` once before formatting. Bundlers that understand `new URL('./file.wasm', import.meta.url)` (Vite, Webpack, Rollup) resolve the WASM asset automatically:

```javascript
import {format_svelte, init} from '@fuzdev/tsv_format_wasm';

await init();
const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

`init_sync({ module })` is also exported for Workers and custom loading.

tsv is non-configurable: settings are fixed at Prettier's defaults (`print_width: 100`, `tab_width: 2`, `use_tabs: true`) — no options, like `gofmt` and Black.

## Status

v0.1 — pre-release. API may change.

## License

[MIT](LICENSE)
