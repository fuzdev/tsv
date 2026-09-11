# @fuzdev/tsv_format_wasm

> formatter for Svelte, TypeScript, and CSS

Rust-based formatter compiled to WASM. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely, with documented divergences.

tsv is non-configurable: settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

Formatting only — for parser / AST extraction, see [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm), or [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) for both plus a CLI. On Node.js and Bun, [`@fuzdev/tsv`](https://www.npmjs.com/package/@fuzdev/tsv) is the native build of the same API.

Docs and benchmarks: [tsv.fuz.dev](https://tsv.fuz.dev/). Source and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv_format_wasm
```

Requires Node.js 22+; Bun and Deno work too, and browsers via `init()` (below).

## Usage

Three formatting functions: `format_svelte`, `format_typescript`, `format_css`. Each takes a source `string` and returns the formatted `string`, throwing on a parse error.

Each also takes an optional trailing options object. Formatting itself is non-configurable, so the only option is `format_typescript(source, {sourceType: 'script' | 'module'})` — the parse goal, where `'script'` makes `await` an ordinary identifier, turns top-level `import`/`export`, `for await` and `import.meta` into syntax errors, and leaves the code sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse; a module is always strict). `format_svelte`/`format_css` **throw** on the key rather than ignoring it (Svelte's `<script>` is always a module, CSS has no goal), so code forwarding one options bag to whichever formatter should spell the inapplicable source type as `undefined` — a supported key set to `undefined` reads as its default. Omitting `sourceType` is not the same as passing `'module'`: the source is formatted as a module and, only if that parse fails, retried as a script — so a legacy sloppy script formats with no bag at all, while anything the module grammar accepts is never reinterpreted (formatting reads no goal, so no output changes). A set value is exact, and when both grammars reject the source the error thrown is the module's if the script retry died on a top-level `import`/`export`, an `import.meta`, a top-level `for await` or a top-level `await`'s operand (the source is a module, wherever that sits), else the one that reached further into it (the module's on a tie) — a sloppy script's own typo, not the `with` the retry would have accepted. Unknown option keys throw, whatever their value.

A second argument that isn't an object throws too, arrays included. That makes `sources.map(format_typescript)` an error, since `map` passes the index as the second argument — write `sources.map((s) => format_typescript(s))`.

A Rust panic — always a tsv bug, please report it — surfaces as a `RuntimeError: unreachable` with the real message on `console.error`; the instance survives it, so the next call works.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, and the deepest shapes — nested arrow bodies and member chains — cost several times more of it per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error or a panic it **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Objects created before the swap (an `IgnoreStack`) are invalidated — rebuild them after: every method on a stale one throws, and `free()` on it is a safe no-op. Real code is nowhere near this ceiling; generated and minified code can be.

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

### Worker pools

To format across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv_format_wasm/worker` subpath is the same API without the import-time initialization, so a worker starts from that module instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead, or `postMessage` a `WebAssembly.Module` you compiled yourself.

```typescript
// main thread
import {Worker} from 'node:worker_threads';
import {wasm_module} from '@fuzdev/tsv_format_wasm';
new Worker(worker_url, {workerData: {wasm_module}});

// worker
import {workerData} from 'node:worker_threads';
import {format_typescript, init_sync} from '@fuzdev/tsv_format_wasm/worker';
init_sync({module: workerData.wasm_module});
```

### File scoping (`IgnoreStack`)

For tooling that needs tsv's exact file scoping, this package also exports an `IgnoreStack` class — the same hierarchical, git-faithful matcher the `tsv` CLI uses (per-directory `.gitignore` layers plus one tsv layer per directory, fed by `.formatignore` or, failing that, `.prettierignore`) to decide which files it formats. Build it from a repo's ignore files (one layer per directory, anchored at that directory), then query per path. Walking directories is the caller's job, but the class also carries tsv's discovery policy — `classify_dir`, `should_format_file`, `is_path_pruned`, `excluded_argument_warning`, `unsupported_extension_error`, `heuristic_shadow_warning`, `prettierignore_outside_repo_warning`, and `prettierignore_shadowed_warning` — so a walker reproduces the CLI's decisions, and its warnings, exactly.

```javascript
import {IgnoreStack} from '@fuzdev/tsv_format_wasm';

const stack = new IgnoreStack();
stack.push_gitignore('', 'build/\n*.log\n'); // a .gitignore (anchor '' = root)
stack.push_formatignore('', '!keep.log\n'); // a .formatignore, evaluated after the gitignores
stack.is_ignored('build/out.js', false); // → true
stack.is_ignored('keep.log', false); // → false (the tsv layer re-includes it)
```

## Status

Pre-alpha — not for production use. The published packages are for feedback; expect bugs and API changes.

## License

[MIT](LICENSE)
