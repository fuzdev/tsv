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

TypeScript declarations are bundled. They name the DOM lib's `fetch` and `WebAssembly` types (for `init()` and the compiled module), so a project type-checking them needs `"dom"` in its `lib`, or `skipLibCheck`.

## Usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {parse_css, parse_svelte, parse_typescript} from '@fuzdev/tsv-parse-wasm';
import type {Program, Root, StyleSheetFile} from '@fuzdev/tsv-parse-wasm';

const root: Root = parse_svelte('<script>const x = 1;</script>');
const program: Program = parse_typescript('const x: number = 1;');
const stylesheet: StyleSheetFile = parse_css('a { color: red }');
```

Three parsers, each returning the AST its canonical parser returns: `parse_svelte` Svelte's modern `parse` AST, `parse_typescript` ESTree as acorn + acorn-typescript emit it, and `parse_css` Svelte's `parseCss` AST. Each takes a source `string` and an optional options object. The AST is **span-only** by default: `start`/`end` offsets on every node, no per-node `loc` (line/column) and no Svelte `name_loc` — pass `{locations: true}` for those.

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

To parse across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module`, and the `@fuzdev/tsv-parse-wasm/worker` subpath is the same API without import-time initialization, so a worker calls `init_sync({module: wasm_module})` on the module handed to it (`workerData` or `postMessage`) instead of compiling again. `wasm_module` is exported by the Node/Bun entry alone; in a browser Worker call `await init()`, or `postMessage` a `WebAssembly.Module` you compiled yourself.

Each parser also has a `parse_*_json` variant (`parse_svelte_json`, `parse_typescript_json`, `parse_css_json`) returning the span-only AST as a compact JSON string, skipping the JS object tree — for writing it to disk or the wire. It takes `{sourceType}` alone; a `locations` key there throws.

### Options

Every parser takes an optional acorn-style options object:

- `locations` (default `false`) — add per-node `loc` (line/column), as acorn's `locations: true` does, and Svelte's `name_loc`. Computed in JS after the parse ([below](#reconstructing-linecolumn)).
- `sourceType` (TypeScript only; omitted means `'module'`) — the parse goal. At `'script'`, `await` is an ordinary identifier, `import`/`export`/`import.meta` and a top-level `for await` are syntax errors, and the code is sloppy unless a `"use strict"` prologue makes it strict; a module is always strict. `parse_svelte` and `parse_css` throw on a set `sourceType`.

A supported key set to `undefined` reads as its default — `sourceType: undefined` included on Svelte and CSS — so one options bag forwards to every object parser. Argument errors are `TypeError`s:

- an unknown key, whatever its value;
- a wrong-typed or invalid value, or a non-string source;
- a non-object second argument, arrays included — write `sources.map((s) => parse_typescript(s))`, not `sources.map(parse_typescript)`, which passes the index.

### Reconstructing line/column

`{locations: true}` runs `reconstruct_locations`, a pure-JS helper the package also exports for a tree you already hold:

```typescript
import {parse_typescript, reconstruct_locations} from '@fuzdev/tsv-parse-wasm';

const src = 'const x = 1;\n';
const ast = parse_typescript(src, {locations: true});
// every node carries loc: {start: {line, column}, end: {line, column}}
// the same, in two steps:
const same = reconstruct_locations(parse_typescript(src), src);
```

`reconstruct_locations(ast, source)` adds `loc` to every object with `start`/`end` and `name_loc` to Svelte elements, attributes, and directives, **in place** (keys appended last), and returns the tree; `structuredClone` it first to keep the input.

- Lines follow the document's rule: ECMAScript's terminators for TypeScript (as acorn counts them), LF alone for a whole Svelte document and for CSS. Offsets are UTF-16 units; a leading BOM counts in TypeScript offsets, not in Svelte or CSS ones.
- On the nodes where Svelte's own `loc` carries a `character` offset (listed on the bundled `Position` type), so does this one.
- For Svelte it is a superset of Svelte's own output (which has `loc` only on acorn-parsed nodes), following the offsets where Svelte's `loc` departs from them ([docs/conformance_svelte.md](https://github.com/fuzdev/tsv/blob/main/docs/conformance_svelte.md)).
- The language is read off the root (`Root`, `StyleSheetFile`, or a whole-source `Program`); pass `{language}` for a subtree, or it throws rather than guess.

For sparse lookups, `create_locator(source, {language})` builds the line table once for `position_at(offset)`, `loc_of(node)` (`null` for a node without numeric `start`/`end`) and `reconstruct(ast)`. Its `language` (`'typescript'`, `'svelte'`, or `'css'`) is required.

```typescript
import {create_locator, parse_typescript} from '@fuzdev/tsv-parse-wasm';

const src = 'let a;\nlet b;';
const locator = create_locator(src, {language: 'typescript'});
const [, b] = parse_typescript(src).body;
locator.loc_of(b); // {start: {line: 2, column: 0}, end: {line: 2, column: 6}}
locator.position_at(7); // {line: 2, column: 0}
```

A locator's single lookups throw a `RangeError` for an offset or span the text doesn't hold; the whole-tree forms check nothing per node. A missing or unknown `language`, an uninferable root, or a non-string source throws a `TypeError`. So does an options argument that isn't an object, or a key other than `language` — a typo like `{langauge: 'css'}` throws rather than falling back to inference.

`reconstruct_locations` and `create_locator` are also the `@fuzdev/tsv-parse-wasm/locations` entry point, pure JS that loads no WASM; the `SourceLocation` and `Position` types they return come from the package root.

### Errors and depth limits

A source that doesn't parse throws a `SyntaxError` with two own properties, `start` and `loc` (typed `TsvSyntaxError`, exported), in the AST's coordinates ([above](#reconstructing-linecolumn)):

- `start` is the error's UTF-16 offset and `loc` its `{line, column}` (1-based line, 0-based UTF-16 column), equal to `create_locator(source, {language}).position_at(start)`.
- The message's second line starts with `loc` as `line:column + 1`.
- Read `start` and `loc`, not `line` / `column`, which some runtimes put on every `Error`.

```javascript
import {parse_typescript} from '@fuzdev/tsv-parse-wasm';

try { parse_typescript('let a;\nconst = ;'); } catch (e) {
	e instanceof SyntaxError; // true — the message ends with the line and a caret
	[e.start, e.loc]; // [13, {line: 2, column: 6}]
}
```

A Rust panic — always a tsv bug, please report it — surfaces as a `RuntimeError: unreachable` with the real message on `console.error`; the instance survives it, so the next call works.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, and the deepest shapes — nested arrow bodies and member chains — cost several times more of it per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error or a panic it **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Real code is nowhere near this ceiling; generated and minified code can be.

## Status

Near production-ready. A long tail of rare bugs remains and APIs may still change; reports are appreciated.

## License

[MIT](LICENSE)
