# @fuzdev/tsv_wasm

> a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS

Rust-based formatter + parser compiled to WASM — the full tool in one package, with a `tsv` CLI. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (with documented divergences), plus a drop-in replacement parser for **Svelte's parser** + **acorn** + **acorn-typescript**.

Only need one half? The subset packages ship smaller WASM blobs: [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm) (format only) and [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm) (parse only).

Source of truth, full docs, and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## CLI

```bash
npx @fuzdev/tsv_wasm format src        # format .ts/.svelte/.css in place, recursively
npx @fuzdev/tsv_wasm format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv_wasm parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed (`npm i -D @fuzdev/tsv_wasm`), the bin is `tsv`. Directories recurse over `.ts`/`.svelte`/`.css`, skipping hidden directories and `node_modules`/`dist`/`build`/`target`. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

This CLI runs the single-threaded WASM build — plenty fast for most trees. A future native `tsv` binary will be the high-performance path.

## Library usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {format_svelte, parse_svelte} from '@fuzdev/tsv_wasm';
import type {Root} from '@fuzdev/tsv_wasm';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const root: Root = parse_svelte('<script>const x = 1;</script>');
```

Three formatters (`format_svelte`, `format_typescript`, `format_css`) take a source `string` and return the formatted `string`. Three parsers (`parse_svelte`, `parse_typescript`, `parse_css`) return a Svelte-compatible JSON AST; the `parse_*_json` variants return the AST as a compact JSON string instead (faster when writing to disk or the wire). All throw on a parse error.

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

tsv is non-configurable: formatter settings are fixed at Prettier's defaults (`print_width: 100`, `tab_width: 2`, `use_tabs: true`) — no options, like `gofmt` and Black.

## Status

v0.1 — pre-release. API may change.

## License

[MIT](LICENSE)
