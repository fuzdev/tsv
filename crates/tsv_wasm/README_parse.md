# @fuzdev/tsv_parse_wasm

> parser for Svelte, TypeScript, and CSS

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

Three parsers: `parse_svelte` (matches Svelte's modern parser), `parse_typescript` (matches acorn + acorn-typescript), `parse_css` (matches Svelte's `parseCss`). Each takes a source `string` plus an optional options object and returns a Svelte-compatible JSON AST, throwing on a parse error.

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

To parse across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv_parse_wasm/worker` subpath is the same API without the import-time initialization — so a worker calls `init_sync({module: wasm_module})` on the module handed to it (`workerData` or `postMessage`) instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead; there is nothing compiled to hand across.

Each parser also has a `parse_*_json` variant (`parse_svelte_json`, `parse_typescript_json`, `parse_css_json`) taking the same arguments but returning the AST as a compact JSON string — faster when you're writing it to disk or sending it over the wire, since it skips materializing the JS object tree.

### Options

Every parser accepts an optional second argument, like acorn:

- `locations` (default `true`) — emit per-node `loc` (line/column), the drop-in acorn/svelte wire. Pass `false` for the **span-only** wire: `start`/`end` offsets only (Svelte also drops `name_loc`), ~46% smaller and much faster to materialize, mirroring acorn's `locations: false`. Line/column stays derivable from the offsets plus your source, so nothing is lost if you have the source. Inert for CSS (its wire carries no `loc` either way).
- `goal` (TypeScript only, default `'module'`) — the parse goal: at `'script'`, `await` is an ordinary identifier and `import`/`export`/`import.meta` are syntax errors. `parse_svelte` and `parse_css` **throw** on the key rather than ignoring it (Svelte's `<script>` is always a module, CSS has no goal), so code forwarding one options bag to whichever parser should spell the inapplicable goal as `undefined` — every supported key reads `undefined` as its default.

Unknown option keys throw, whatever their value — a typo like `{locatons: false}` (or `{locatons: undefined}`) fails loudly instead of silently handing back the full wire.

A second argument that isn't an object throws too, arrays included. That makes `sources.map(parse_typescript)` an error, since `map` passes the index as the second argument — write `sources.map((s) => parse_typescript(s))`.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, which is roughly 1,600 levels of the loosest shape, nested parens (for comparison, acorn gives up around 500 and prettier around 800, measured the same way) — deeply nested *statements* cost several times more stack per level, so treat that figure as a ceiling rather than a floor. Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error that **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Real code is nowhere near this ceiling; generated and minified code can be.

### Reconstructing line/column

Need `loc` back? The package ships a pure-JS helper that derives it from the span-only wire + your source — no re-parse:

```typescript
import {parse_typescript, reconstruct_locations} from '@fuzdev/tsv_parse_wasm';

const src = 'const x = 1;\n';
const ast = reconstruct_locations(parse_typescript(src, {locations: false}), src);
// every node now carries loc: {start: {line, column}, end: {line, column}}
```

`reconstruct_locations(ast, source)` walks the tree and adds `loc` to every node, **mutating in place** and returning it (`structuredClone(ast)` first if you need the input untouched). It's **exact for TypeScript** — each node's `loc` value equals acorn's exactly (the key is appended last, so an object consumer matches but a re-serialized tree won't byte-match the wire's key order) — and **approximate for Svelte**: it doesn't replicate Svelte's `<script>` tag-position or destructure `+1`-column parser quirks. Everything else reconstructs exactly, including the `name_loc` on elements, attributes, and directives, the name-shaped `loc` Svelte reports on a shorthand attribute's identifier, a snippet name, and a simple-identifier block pattern, and the `character` field on an in-tag comment (`<div /* c */ class="x">`, including inside `<svelte:options>`). CSS is a no-op (there's no `loc` to rebuild).

For sparse or repeated lookups, `create_locator(source, opts?)` holds the prebuilt line-start table and exposes `loc_of(node)` and `reconstruct(ast)`; pass `{language: 'svelte'}` for a `.svelte` document (LF-only line rule). A one-shot `loc_of(node, source)` is also exported for the occasional single lookup (it rebuilds the table each call).

## Status

0.x — pre-release. API may change.

## License

[MIT](LICENSE)
