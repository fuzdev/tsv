# @fuzdev/tsv_wasm

> a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS

Rust-based formatter + parser compiled to WASM — the full tool in one package, with a `tsv` CLI. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (with documented divergences), plus a drop-in replacement parser for **Svelte's parser** + **acorn** + **acorn-typescript**.

Only need one half? The subset packages ship smaller WASM blobs: [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm) (format only) and [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm) (parse only).

Source of truth, full docs, and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## CLI

```bash
npx @fuzdev/tsv_wasm format src        # format .ts/.svelte/.css in place, recursively
npx @fuzdev/tsv_wasm format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv_wasm format --list .   # list the in-scope files, format nothing
npx @fuzdev/tsv_wasm parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed (`npm i -D @fuzdev/tsv_wasm`), the bin is `tsv`. Directories recurse over `.ts`/`.svelte`/`.css` with gitignore-aware discovery. **Inside a git repo** it honors `.gitignore`, `.formatignore` (both hierarchical, like git), and a repo-root `.prettierignore`, scoped to the repo so results are reproducible. **Outside a repo** it honors only `.formatignore`, falling back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped; an explicitly named file is always formatted.

`format --list` prints the discovered in-scope files without formatting — a read-only view of what `format` would touch. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. For TypeScript, `--goal script|module` (default `module`; `--content`/`--stdin` only) selects the parse goal — at `script`, `await` is an ordinary identifier and `import`/`export`/`import.meta` are errors. `parse --no-locations` emits the span-only wire (no per-node `loc`; Svelte also no `name_loc`; no-op for CSS). Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

This CLI runs the single-threaded WASM build — plenty fast for most trees. A future native `tsv` binary will be the high-performance path.

## Library usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {format_svelte, parse_svelte} from '@fuzdev/tsv_wasm';
import type {Root} from '@fuzdev/tsv_wasm';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const root: Root = parse_svelte('<script>const x = 1;</script>');
```

Three formatters (`format_svelte`, `format_typescript`, `format_css`) take a source `string` and return the formatted `string`. Three parsers (`parse_svelte`, `parse_typescript`, `parse_css`) return a Svelte-compatible JSON AST; the `parse_*_json` variants return the AST as a compact JSON string instead (faster when writing to disk or the wire). For TypeScript and Svelte, `parse_{typescript,svelte}_no_locations` (and `_json_no_locations`) emit a **span-only** wire — `start`/`end` offsets, no per-node `loc` (Svelte also no `name_loc`) — ~46% smaller and faster to materialize, with line/column derivable from offsets + source. All throw on a parse error.

To turn a span-only wire back into a loc-bearing one, `reconstruct_locations(ast, source)` adds `loc` to every node (mutating in place; `structuredClone` first to keep the input) — **exact for TypeScript** (each node's `loc` value equals acorn's; the key is appended last, so an object consumer matches but a re-serialized tree won't byte-match the wire's key order), **approximate for Svelte** (no `name_loc`, and it skips Svelte's `<script>`/destructure position quirks), a no-op for CSS. For sparse lookups, `create_locator(source, opts?)` reuses one line table across `loc_of(node)` / `reconstruct(ast)` calls (pass `{language: 'svelte'}` for `.svelte`); a bare `loc_of(node, source)` is also exported.

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

tsv is non-configurable: formatter settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

## Status

v0.1 — pre-release. API may change.

## License

[MIT](LICENSE)
