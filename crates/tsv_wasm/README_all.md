# @fuzdev/tsv-wasm

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

Rust-based formatter + parser compiled to WASM — the full tool in one package, with a `tsv` CLI. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (with documented divergences), plus a drop-in replacement parser for **Svelte's parser** + **acorn** + **acorn-typescript**.

tsv is non-configurable: formatter settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

Only need one half? The subset packages ship smaller WASM blobs: [`@fuzdev/tsv-format-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-format-wasm) (format only) and [`@fuzdev/tsv-parse-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-parse-wasm) (parse only). On Node.js and Bun, [`@fuzdev/tsv`](https://www.npmjs.com/package/@fuzdev/tsv) is the native build of the same API and CLI.

Docs and benchmarks: [tsv.fuz.dev](https://tsv.fuz.dev/). Source and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i -D @fuzdev/tsv-wasm
```

Requires Node.js 22+; Bun and Deno work too, and browsers via `init()` (below).

TypeScript declarations are bundled. They name the DOM lib's `fetch` and `WebAssembly` types (for `init()` and the compiled module), so a project type-checking them needs `"dom"` in its `lib`, or `skipLibCheck`.

## CLI

```bash
npx @fuzdev/tsv-wasm format src        # format .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css in place, recursively
npx @fuzdev/tsv-wasm format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv-wasm format --list .   # list the in-scope files, format nothing
npx @fuzdev/tsv-wasm parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed, the bin is `tsv`.

Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`), `.svelte`, and `.css` with gitignore-aware discovery:

- **Inside a git repo** it honors `.gitignore`, then `.formatignore` — both hierarchical, like git — plus `.prettierignore` as a drop-in fallback, read in any directory that has no `.formatignore` of its own (a sibling `.formatignore` shadows it, with a warning); all scoped to the repo so results are reproducible.
- **Outside a repo** it honors only `.formatignore`.
- With no `.gitignore` in scope (in or out of a repo), discovery falls back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped by a walk.
- A path you name — file or directory — is bounded by the ignore files alone: one they exclude is skipped (with a warning, unless it is a file your `.formatignore` or `.prettierignore` excludes), while the built-in skips never apply to it; a named file's extension must still be one tsv formats.

`format --list` prints the discovered in-scope files without formatting — a read-only view of what `format` would touch. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. For TypeScript, `--source-type script|module` (`parse` defaults to `module`; `format` takes an unset flag as *none named* — module, retried as a script — and accepts it on `--content`/`--stdin` only) selects the parse goal — at `script`, `await` is an ordinary identifier, `import`/`export`/`import.meta` and a top-level `for await` are errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse). `parse` emits the span-only wire (no per-node `loc`; Svelte also no `name_loc`); `parse --locations` adds them. Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

Large trees format across worker threads — `--jobs N` sets the count, and the default is sized to your machine. Small ones stay on a single thread, where spinning a pool up would cost more than it saves. This CLI runs the WASM build; on Node.js and Bun, [`@fuzdev/tsv`](https://www.npmjs.com/package/@fuzdev/tsv) ships tsv's real native CLI binary and its `tsv` bin execs it — the faster option there. Both packages claim the `tsv` bin name, so install one or the other in a project.

## Library usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {format_svelte, parse_svelte} from '@fuzdev/tsv-wasm';
import type {Root} from '@fuzdev/tsv-wasm';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const root: Root = parse_svelte('<script>const x = 1;</script>');
```

### Formatting and parsing

Three formatters (`format_svelte`, `format_typescript`, `format_css`) take a source `string` and return the formatted `string`. Three parsers return the AST their canonical parser returns — `parse_svelte` Svelte's modern `parse` AST, `parse_typescript` ESTree as acorn + acorn-typescript emit it, `parse_css` Svelte's `parseCss` AST — span-only by default, `start`/`end` offsets on every node and no per-node `loc`; the `parse_*_json` variants return that AST as a compact JSON string instead (faster when writing to disk or the wire). All throw on a parse error. AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

### Options

Every export takes `(source, options?)` with an acorn-style options object.

`{sourceType: 'script' | 'module'}` sets TypeScript's parse goal — at `'script'` the code is sloppy unless its own `"use strict"` prologue makes it strict. `parse_svelte`/`parse_css` and `format_svelte`/`format_css` throw on a set value. An **omitted** `sourceType` is `'module'` to a parser, while a formatter parses as a module and retries as a script only if that fails, so a legacy sloppy script formats with no options; a set value is exact on both. When both grammars reject a format's source, the error is the one that best explains the failure (the rule is in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#multi-file-formatting)).

The object parsers also take `{locations: true}`, adding per-node `loc` (line/column, as acorn's `locations: true` does) and Svelte's `name_loc`. Every other export throws on a `locations` key, whatever its value, so only a `sourceType`-only bag forwards to any export.

A supported key set to `undefined` reads as its default — `sourceType: undefined` included on Svelte and CSS. Argument errors are `TypeError`s:

- an unknown key, whatever its value;
- a wrong-typed or invalid value, or a non-string source;
- a non-object second argument, arrays included — write `sources.map((s) => format_typescript(s))`, not `sources.map(format_typescript)`, which passes the index.

### Reconstructing line/column

`{locations: true}` runs `reconstruct_locations`, a pure-JS helper the package also exports for a tree you already hold:

```typescript
import {parse_typescript, reconstruct_locations} from '@fuzdev/tsv-wasm';

const src = 'const x = 1;\n';
const ast = parse_typescript(src, {locations: true});
// the same, in two steps:
const same = reconstruct_locations(parse_typescript(src), src);
```

It adds `loc` to every object with `start`/`end` and `name_loc` to Svelte elements, attributes, and directives, **in place** (keys appended last); `structuredClone` the tree first to keep the input.

- Lines follow the document's rule: ECMAScript's terminators for TypeScript (as acorn counts them), LF alone for a whole Svelte document and for CSS. Offsets are UTF-16 units; a leading BOM counts in TypeScript offsets, not in Svelte or CSS ones.
- For Svelte it is a superset of Svelte's own output (which has `loc` only on acorn-parsed nodes), following the offsets where Svelte's `loc` departs from them ([docs/conformance_svelte.md](https://github.com/fuzdev/tsv/blob/main/docs/conformance_svelte.md)).
- The language is read off the root (`Root`, `StyleSheetFile`, or a whole-source `Program`); pass `{language}` for a subtree, or it throws rather than guess.

For sparse lookups, `create_locator(source, {language})` builds the line table once for `position_at(offset)`, `loc_of(node)` (`null` for a node without numeric `start`/`end`) and `reconstruct(ast)`. Its `language` (`'typescript'`, `'svelte'`, or `'css'`) is required.

```typescript
import {create_locator, parse_typescript} from '@fuzdev/tsv-wasm';

const src = 'let a;\nlet b;';
const locator = create_locator(src, {language: 'typescript'});
const [, b] = parse_typescript(src).body;
locator.loc_of(b); // {start: {line: 2, column: 0}, end: {line: 2, column: 6}}
locator.position_at(7); // {line: 2, column: 0}
```

A locator's single lookups throw a `RangeError` for an offset or span the text doesn't hold; the whole-tree forms check nothing per node. A missing or unknown `language`, an uninferable root, or a non-string source throws a `TypeError`. So does an options argument that isn't an object, or a key other than `language` — a typo like `{langauge: 'css'}` throws rather than falling back to inference.

`reconstruct_locations` and `create_locator` are also the `@fuzdev/tsv-wasm/locations` entry point, pure JS that loads no WASM.

### Errors and depth limits

A source that doesn't parse throws a `SyntaxError` — from a parser and a formatter alike — with two own properties, `start` and `loc` (typed `TsvSyntaxError`, exported), in the AST's coordinates ([above](#reconstructing-linecolumn)):

- `start` is the error's UTF-16 offset and `loc` its `{line, column}` (1-based line, 0-based UTF-16 column), equal to `create_locator(source, {language}).position_at(start)`.
- The message's second line starts with `loc` as `line:column + 1`.
- A formatter reports positions in your own source, CRLF line endings included — the error a parser reports, except that with no `sourceType`, `format_typescript` may report its script retry's error instead.
- Read `start` and `loc`, not `line` / `column`, which some runtimes put on every `Error`.

```javascript
import {format_typescript} from '@fuzdev/tsv-wasm';

try { format_typescript('let a;\nconst = ;'); } catch (e) {
	e instanceof SyntaxError; // true — the message ends with the line and a caret
	[e.start, e.loc]; // [13, {line: 2, column: 6}]
}
```

A Rust panic — always a tsv bug, please report it — surfaces as a `RuntimeError: unreachable` with the real message on `console.error`; the instance survives it, so the next call works.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, and the deepest shapes — nested arrow bodies and member chains — cost several times more of it per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error or a panic it **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Objects created before the swap (an `IgnoreStack`) are invalidated — rebuild them after: every method on a stale one throws, and `free()` on it is a safe no-op. The `tsv` bin does this automatically, so a too-deep file is one per-file error and the rest of the run formats normally. Real code is nowhere near this ceiling; generated and minified code can be.

### Worker pools

To run tsv across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv-wasm/worker` subpath is the same API without the import-time initialization — so a worker calls `init_sync({module: wasm_module})` on the module handed to it (`workerData` or `postMessage`) instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. The `tsv` bin above uses exactly this. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead, or `postMessage` a `WebAssembly.Module` you compiled yourself. [`@fuzdev/tsv-format-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-format-wasm) shows the two-line pattern.

### File scoping (`IgnoreStack`)

For tooling that needs tsv's exact file scoping, the package also exports the `IgnoreStack` class — the hierarchical `.gitignore` matcher with tsv's `.formatignore`/`.prettierignore` layer, plus tsv's discovery policy (`classify_dir`, `should_format_file`, `is_path_pruned`, `path_shadow_warning`, `excluded_argument_warning`, `unsupported_extension_error`, `shadow_warning`, `prettierignore_outside_repo_warning`, `prettierignore_shadowed_warning`, `gitignore_symlink_warning`); [`@fuzdev/tsv-format-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-format-wasm) documents it with examples.

## Status

Near production-ready. A long tail of rare bugs remains and APIs may still change; reports are appreciated.

## License

[MIT](LICENSE)
