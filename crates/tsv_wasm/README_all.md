# @fuzdev/tsv_wasm

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

Rust-based formatter + parser compiled to WASM — the full tool in one package, with a `tsv` CLI. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (with documented divergences), plus a drop-in replacement parser for **Svelte's parser** + **acorn** + **acorn-typescript**.

Only need one half? The subset packages ship smaller WASM blobs: [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm) (format only) and [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm) (parse only).

Source of truth, full docs, and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## CLI

```bash
npx @fuzdev/tsv_wasm format src        # format .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css in place, recursively
npx @fuzdev/tsv_wasm format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv_wasm format --list .   # list the in-scope files, format nothing
npx @fuzdev/tsv_wasm parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed (`npm i -D @fuzdev/tsv_wasm`), the bin is `tsv`. Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`), `.svelte`, and `.css` with gitignore-aware discovery. **Inside a git repo** it honors `.gitignore`, `.formatignore`, and `.prettierignore` (all hierarchical, like git), scoped to the repo so results are reproducible. **Outside a repo** it honors only `.formatignore`. With no `.gitignore` in scope (in or out of a repo), discovery falls back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped; an explicitly named file skips the ignore files, but its extension must still be one tsv formats.

`format --list` prints the discovered in-scope files without formatting — a read-only view of what `format` would touch. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. For TypeScript, `--goal script|module` (default `module`; for `format`, `--content`/`--stdin` only) selects the parse goal — at `script`, `await` is an ordinary identifier and `import`/`export`/`import.meta` are errors. `parse --no-locations` emits the span-only wire (no per-node `loc`; Svelte also no `name_loc`; no-op for CSS). Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

Large trees format across worker threads — `--jobs N` sets the count, and the default is sized to your machine. Small ones stay on a single thread, where spinning a pool up would cost more than it saves. This CLI runs the WASM build; on Node.js and Bun, [`@fuzdev/tsv`](https://www.npmjs.com/package/@fuzdev/tsv) ships tsv's real native CLI binary and its `tsv` bin execs it — still the fast path.

## Library usage

In Node.js, Bun, and Deno, WASM is initialized synchronously at import time — zero config. In browsers and bundlers, call `await init()` once first (Vite, Webpack, and Rollup resolve the WASM asset automatically; `init_sync({ module })` is also exported for Workers and custom loading).

```typescript
import {format_svelte, parse_svelte} from '@fuzdev/tsv_wasm';
import type {Root} from '@fuzdev/tsv_wasm';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const root: Root = parse_svelte('<script>const x = 1;</script>');
```

Three formatters (`format_svelte`, `format_typescript`, `format_css`) take a source `string` and return the formatted `string`. Three parsers (`parse_svelte`, `parse_typescript`, `parse_css`) return a Svelte-compatible JSON AST; the `parse_*_json` variants return the AST as a compact JSON string instead (faster when writing to disk or the wire). Every export shares one signature — `(source, options?)` with an acorn-style options object. `{goal: 'script' | 'module'}` sets the parse goal (TypeScript only — `parse_svelte`/`parse_css` and `format_svelte`/`format_css` throw on the key, so forward it as `undefined` when it doesn't apply). The parsers additionally take `{locations: false}`, which emits a **span-only** wire — `start`/`end` offsets, no per-node `loc` (Svelte also no `name_loc`) — ~46% smaller and faster to materialize, with line/column derivable from offsets + source; formatting emits no wire, so the formatters reject that key (and, being non-configurable, take no option beyond the goal). A **goal-only** bag is therefore the one that forwards to any export, parser or formatter; a bag carrying `locations` is a parse bag and throws on a formatter. Unknown option keys throw, whatever their value; a supported key set to `undefined` is read as absent. A second argument that isn't an object throws too, arrays included — that makes `sources.map(format_typescript)` an error, since `map` passes the index as the second argument, so write `sources.map((s) => format_typescript(s))`. All throw on a parse error.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, which is roughly 1,600 levels of nesting (for comparison, acorn gives up around 500 and prettier around 800). Past it the call traps with `memory access out of bounds`, and unlike a parse error that **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Objects created before the swap (an `IgnoreStack`) are invalidated — `free()` them first and rebuild after. The `tsv` bin does this automatically, so a too-deep file is one per-file error and the rest of the run formats normally. Real code is nowhere near this ceiling; generated and minified code can be.

To turn a span-only wire back into a loc-bearing one, `reconstruct_locations(ast, source)` adds `loc` to every node — and the Svelte `name_loc` — (mutating in place; `structuredClone` first to keep the input) — **exact for TypeScript** (each node's `loc` value equals acorn's; the key is appended last, so an object consumer matches but a re-serialized tree won't byte-match the wire's key order), **approximate for Svelte** (it skips only Svelte's `<script>`/destructure position quirks; `name_loc` and the in-tag comment `character` field are exact), a no-op for CSS. For sparse lookups, `create_locator(source, opts?)` reuses one line table across `loc_of(node)` / `reconstruct(ast)` calls (pass `{language: 'svelte'}` for `.svelte`); a bare `loc_of(node, source)` is also exported.

AST types are bundled in `tsv_ast.d.ts` and re-exported from the package — `import type` any node directly.

To run tsv across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv_wasm/worker` subpath is the same API without the import-time initialization — so a worker calls `init_sync({module: wasm_module})` on the module handed to it (`workerData` or `postMessage`) instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. The `tsv` bin above uses exactly this. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead; there is nothing compiled to hand across.

For tooling that needs tsv's exact file scoping, the package also exports the `IgnoreStack` class — the hierarchical `.gitignore`/`.formatignore`/`.prettierignore` matcher plus tsv's discovery policy (`classify_dir`, `should_format_file`, `is_path_pruned`, `unsupported_extension_error`, `heuristic_shadow_warning`); [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm) documents it with examples.

tsv is non-configurable: formatter settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

## Status

0.x — pre-release. API may change.

## License

[MIT](LICENSE)
