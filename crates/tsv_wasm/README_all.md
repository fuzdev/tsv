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

## CLI

```bash
npx @fuzdev/tsv-wasm format src        # format .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css in place, recursively
npx @fuzdev/tsv-wasm format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv-wasm format --list .   # list the in-scope files, format nothing
npx @fuzdev/tsv-wasm parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed, the bin is `tsv`. Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`), `.svelte`, and `.css` with gitignore-aware discovery. **Inside a git repo** it honors `.gitignore`, then `.formatignore` — both hierarchical, like git — plus `.prettierignore` as a drop-in fallback, read in any directory that has no `.formatignore` of its own (a sibling `.formatignore` shadows it, with a warning); all scoped to the repo so results are reproducible. **Outside a repo** it honors only `.formatignore`. With no `.gitignore` in scope (in or out of a repo), discovery falls back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped by a walk. A path you name — file or directory — is bounded by the ignore files alone: one they exclude is skipped (with a warning, unless it is a file your `.formatignore` or `.prettierignore` excludes), while the built-in skips never apply to it; a named file's extension must still be one tsv formats.

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

Three formatters (`format_svelte`, `format_typescript`, `format_css`) take a source `string` and return the formatted `string`. Three parsers (`parse_svelte`, `parse_typescript`, `parse_css`) return a Svelte-compatible JSON AST — span-only by default, `start`/`end` offsets on every node and no per-node `loc`; the `parse_*_json` variants return that AST as a compact JSON string instead (faster when writing to disk or the wire). All throw on a parse error. AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

### Options

Every export shares one signature — `(source, options?)` with an acorn-style options object.

`{sourceType: 'script' | 'module'}` sets the parse goal — at `'script'` the code is also sloppy unless its own `"use strict"` prologue makes it strict. TypeScript only: `parse_svelte`/`parse_css` and `format_svelte`/`format_css` throw on the key, so forward it as `undefined` when it doesn't apply. The two families read an **omitted** `sourceType` differently: a parse takes it as `'module'`, while a format takes it as *none named* and parses as a module, retried as a script only if that fails — so a legacy sloppy script formats with no bag at all, and a set value is exact on both.

The object parsers additionally take `{locations: true}`, which adds per-node `loc` (line/column, as acorn's `locations: true` does; Svelte also `name_loc`), computed in JS from the offsets plus the source. The `parse_*_json` variants return the wire itself and the formatters emit none, so both reject that key — a `_json` variant with a message pointing at its object parser — and the formatters, being non-configurable, take no option beyond the source type. A **`sourceType`-only** bag is therefore the one that forwards to any export, parser or formatter; a bag carrying `locations` is an object-parse bag and throws anywhere else.

Unknown option keys throw, whatever their value; a supported key set to `undefined` is read as absent. A second argument that isn't an object throws too, arrays included — that makes `sources.map(format_typescript)` an error, since `map` passes the index as the second argument, so write `sources.map((s) => format_typescript(s))`. Every argument error — a source that isn't a string, a bad options argument, an unknown key, a wrong-typed or invalid value — is a `TypeError`; a source that doesn't parse throws a `SyntaxError` ([below](#errors-and-depth-limits)).

### Reconstructing line/column

`{locations: true}` runs `reconstruct_locations(ast, source)` (one extra walk over the tree after the parse, no re-parse), also exported for a tree you already hold: it adds `loc` to every node — and the Svelte `name_loc` — mutating in place (`structuredClone` first to keep the input), with the key appended last on each object. Without a `language` it is read off the root (`Root`, `StyleSheetFile`, or a `Program` spanning the whole source), so for a subtree — a Svelte `Fragment`, a `<script>`'s `Program`, a single statement — pass `{language}`; it throws rather than guess. Each `loc` follows the document's line rule: ECMAScript's terminators for TypeScript (acorn's count exactly), `\n` alone for a Svelte document and for CSS. For a Svelte document this is a superset of Svelte's own `parse` output, which carries `loc` only on the nodes acorn parsed, and it follows the definition above where Svelte's `loc` departs from its own offsets (Svelte's `<script>` `Program.loc` sits at the tag; here it matches `start`/`end`) — cataloged in the repo's [docs/conformance_svelte.md](https://github.com/fuzdev/tsv/blob/main/docs/conformance_svelte.md). For sparse lookups, `create_locator(source, {language})` reuses one line table across `position_at(offset)` (one offset's `{line, column}`), `loc_of(node)` (one node's `loc`, or `null` without numeric `start`/`end`) and `reconstruct(ast)` calls; a bare `loc_of(node, source, {language})` is also exported. Both take the language (`'typescript'`, `'svelte'`, or `'css'`) as a required option, since it picks the line rule. Offsets are the AST's own: UTF-16 units, into the source with a leading BOM dropped for Svelte and CSS and as given for TypeScript. The single lookups throw a `RangeError` for an offset or span the text doesn't hold; the whole-tree forms check nothing per node. A missing or unknown `language`, an uninferable root, or a non-string source throws a `TypeError`. So does an options argument that isn't an object, or a key other than `language` — a typo like `{langauge: 'css'}` throws rather than falling back to inference.

The helper is also its own entry point, `@fuzdev/tsv-wasm/locations` — the same three functions as pure JS that loads no WASM — for code that holds a tree and only needs line/column.

### Errors and depth limits

A source that doesn't parse throws a `SyntaxError` — from a parser and a formatter alike — with two own properties, `start` and `loc` (typed `TsvSyntaxError`, exported). `start` is the UTF-16 offset of the error and `loc` its `{line, column}` (1-based line, 0-based UTF-16 column), in the same coordinates as the AST's own positions: TypeScript counts ECMAScript line terminators (LF, CR, CRLF, U+2028, U+2029) and a leading BOM; Svelte (`<script>`, `<style>` and template expressions included) and CSS count LF alone and leave a leading BOM out of the offsets. So `loc` is `create_locator(source, {language}).position_at(start)`, and the message's second line starts with `loc` as `line:column + 1`. A formatter's position is into your own source even where it has CRLF line endings, so for the same error a formatter reports what a parser does — but with no `sourceType` named, `format_typescript` retries a failed module parse as a script and can report that attempt's error where `parse_typescript` (a module unless told otherwise) reports its own. Read `start` and `loc` rather than `line` / `column`, which some runtimes put on every `Error`:

```javascript
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
