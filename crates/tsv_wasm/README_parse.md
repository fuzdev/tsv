# @fuzdev/tsv-parse-wasm

> parser for Svelte, TypeScript, and CSS

Rust-based parser compiled to WASM. Drop-in replacement for **Svelte's parser** + **acorn** + **acorn-typescript**.

Parsing only — for formatting, see [`@fuzdev/tsv-format-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-format-wasm), or [`@fuzdev/tsv-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-wasm) for both plus a CLI. On Node.js and Bun, [`@fuzdev/tsv`](https://www.npmjs.com/package/@fuzdev/tsv) is the native build of the same API.

Docs and benchmarks: [tsv.fuz.dev](https://tsv.fuz.dev/). Source and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv-parse-wasm
```

Requires Node.js 22+; Bun and Deno work too, and browsers via `init()` (below).

## Usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {parse_css, parse_svelte, parse_typescript} from '@fuzdev/tsv-parse-wasm';
import type {Program, Root, StyleSheetFile} from '@fuzdev/tsv-parse-wasm';

const root: Root = parse_svelte('<script>const x = 1;</script>');
const program: Program = parse_typescript('const x: number = 1;');
const stylesheet: StyleSheetFile = parse_css('a { color: red }');
```

Three parsers: `parse_svelte` (matches Svelte's modern parser), `parse_typescript` (matches acorn + acorn-typescript), `parse_css` (matches Svelte's `parseCss`). Each takes a source `string` plus an optional options object and returns a Svelte-compatible JSON AST, throwing on a parse error. The AST is **span-only** by default: every node carries its `start`/`end` offsets but no per-node `loc` (line/column) object, and Svelte nodes no `name_loc` — pass `{locations: true}` for those (below).

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

To parse across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv-parse-wasm/worker` subpath is the same API without the import-time initialization — so a worker calls `init_sync({module: wasm_module})` on the module handed to it (`workerData` or `postMessage`) instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead, or `postMessage` a `WebAssembly.Module` you compiled yourself.

Each parser also has a `parse_*_json` variant (`parse_svelte_json`, `parse_typescript_json`, `parse_css_json`) returning the span-only AST as a compact JSON string — faster when you're writing it to disk or sending it over the wire, since it skips materializing the JS object tree. It takes `{sourceType}` alone: `loc` is computed over objects, so `locations` is not a `_json` option.

### Options

Every parser accepts an optional second argument, like acorn:

- `locations` (default `false`) — add per-node `loc` (line/column), as acorn's `locations: true` does, and Svelte's `name_loc`. The parser emits only the offsets; `{locations: true}` computes the line/column objects from them and your source in JS after the parse (see below) — one extra walk over the tree, so it costs more than the default but needs no re-parse, and leaving it off loses nothing if you have the source. For a few lookups, `create_locator` is cheaper than reconstructing the whole tree.
- `sourceType` (TypeScript only, default `'module'` — the parsers have no source-type fallback, since the wire's `Program.sourceType` is a claim one settled grammar has to produce) — the parse goal: at `'script'`, `await` is an ordinary identifier, `import`/`export`/`import.meta` and a top-level `for await` are syntax errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse; a module is always strict). `parse_svelte` and `parse_css` **throw** on the key rather than ignoring it (Svelte's `<script>` is always a module, CSS has no goal), so code forwarding one options bag to whichever parser should spell the inapplicable source type as `undefined` — every supported key reads `undefined` as its default.

Unknown option keys throw, whatever their value — a typo like `{locatons: true}` (or `{locatons: undefined}`) fails loudly instead of silently handing back the tree without `loc`.

A second argument that isn't an object throws too, arrays included. That makes `sources.map(parse_typescript)` an error, since `map` passes the index as the second argument — write `sources.map((s) => parse_typescript(s))`.

A Rust panic — always a tsv bug, please report it — surfaces as a `RuntimeError: unreachable` with the real message on `console.error`; the instance survives it, so the next call works.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, and the deepest shapes — nested arrow bodies and member chains — cost several times more of it per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error or a panic it **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Real code is nowhere near this ceiling; generated and minified code can be.

### Reconstructing line/column

`{locations: true}` derives `loc` from the offsets plus your source — no re-parse — with a pure-JS helper the package also exports, for a tree you already hold:

```typescript
import {parse_typescript, reconstruct_locations} from '@fuzdev/tsv-parse-wasm';

const src = 'const x = 1;\n';
const ast = parse_typescript(src, {locations: true});
// every node carries loc: {start: {line, column}, end: {line, column}}
// the same, in two steps:
const same = reconstruct_locations(parse_typescript(src), src);
```

`reconstruct_locations(ast, source)` walks the tree and adds `loc` to every object carrying `start`/`end`, **mutating in place** and returning it (`structuredClone(ast)` first if you need the input untouched). Each `loc` is the line and column of the object's own `start`/`end` — ECMAScript's line terminators for TypeScript (acorn's `locations: true` exactly), `\n` alone for a whole Svelte document and for CSS — together with the `name_loc` on Svelte elements, attributes, and directives, and the `character` field Svelte reports on a shorthand attribute's identifier, a snippet name, a simple-identifier block pattern, and an in-tag comment (`<div /* c */ class="x">`, including inside `<svelte:options>`). The key is appended last on each object. For a Svelte document this is a superset of Svelte's own `parse` output, which carries `loc` only on the nodes acorn parsed, and it follows the definition above where Svelte's `loc` departs from its own offsets (Svelte's `<script>` `Program.loc` sits at the tag; here it matches `start`/`end`) — cataloged in the repo's [docs/conformance_svelte.md](https://github.com/fuzdev/tsv/blob/main/docs/conformance_svelte.md). Without a `language` it is read off the root (`Root`, `Program`, `StyleSheetFile`), so for a subtree — a Svelte `Fragment`, a single statement — pass `{language}`; it throws rather than guess.

For sparse or repeated lookups, `create_locator(source, {language})` holds the prebuilt line-start table and exposes `loc_of(node)` and `reconstruct(ast)`; the `language` (`'typescript'`, `'svelte'`, or `'css'`) is required, since it picks the document's line rule. A one-shot `loc_of(node, source, {language})` is also exported for the occasional single lookup (it rebuilds the table each call).

## Status

Near production-ready. A long tail of rare bugs remains and APIs may still change; reports are appreciated.

## License

[MIT](LICENSE)
