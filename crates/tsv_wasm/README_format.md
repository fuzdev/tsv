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

tsv is non-configurable: settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

### File scoping (`IgnoreStack`)

For tooling that needs tsv's exact file scoping, this package also exports an `IgnoreStack` class — the same hierarchical, git-faithful matcher (per-directory `.gitignore`, `.formatignore`, and `.prettierignore` layers) the `tsv` CLI uses to decide which files it formats. Build it from a repo's ignore files (one layer per directory, anchored at that directory), then query per path; locating the files and walking directories is the caller's job.

```javascript
import {IgnoreStack} from '@fuzdev/tsv_format_wasm';

const stack = new IgnoreStack();
stack.push_gitignore('', 'build/\n*.log\n'); // a .gitignore (anchor '' = root)
stack.push_tsv('', '!keep.log\n'); // a .formatignore, evaluated after the gitignores
stack.is_ignored('build/out.js', false); // → true
stack.is_ignored('keep.log', false); // → false (the tsv layer re-includes it)
```

## Status

v0.1 — pre-release. API may change.

## License

[MIT](LICENSE)
