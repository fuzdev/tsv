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

Each also takes an optional trailing options object. Formatting itself is non-configurable, so the only option is `format_typescript(source, {goal: 'script' | 'module'})` — the parse goal, where `'script'` makes `await` an ordinary identifier and turns `import`/`export`/`import.meta` into syntax errors. `format_svelte`/`format_css` **throw** on the key rather than ignoring it (Svelte's `<script>` is always a module, CSS has no goal), so code forwarding one options bag to whichever formatter should spell the inapplicable goal as `undefined` — a supported key set to `undefined` reads as its default. Unknown option keys throw, whatever their value.

A second argument that isn't an object throws too, arrays included. That makes `sources.map(format_typescript)` an error, since `map` passes the index as the second argument — write `sources.map((s) => format_typescript(s))`.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, and nested *statements* cost several times more of it per level than nested parens (the measured ceiling per nesting shape is in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error that **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Objects created before the swap (an `IgnoreStack`) are invalidated — `free()` them first and rebuild after. Real code is nowhere near this ceiling; generated and minified code can be.

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

To format across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv_format_wasm/worker` subpath is the same API without the import-time initialization, so a worker starts from that module instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead; there is nothing compiled to hand across.

```typescript
// main thread
import {wasm_module} from '@fuzdev/tsv_format_wasm';
new Worker(worker_url, {workerData: {wasm_module}});

// worker
import {format_typescript, init_sync} from '@fuzdev/tsv_format_wasm/worker';
init_sync({module: workerData.wasm_module});
```

In a browser Web Worker the same entry takes `await init()` instead, or a `WebAssembly.Module` sent through `postMessage`.

tsv is non-configurable: settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

### File scoping (`IgnoreStack`)

For tooling that needs tsv's exact file scoping, this package also exports an `IgnoreStack` class — the same hierarchical, git-faithful matcher (per-directory `.gitignore`, `.formatignore`, and `.prettierignore` layers) the `tsv` CLI uses to decide which files it formats. Build it from a repo's ignore files (one layer per directory, anchored at that directory), then query per path. Walking directories is the caller's job, but the class also carries tsv's discovery policy — `classify_dir`, `should_format_file`, `is_path_pruned`, `unsupported_extension_error`, `heuristic_shadow_warning`, `prettierignore_outside_repo_warning`, and `prettierignore_shadowed_warning` — so a walker reproduces the CLI's decisions, and its warnings, exactly.

```javascript
import {IgnoreStack} from '@fuzdev/tsv_format_wasm';

const stack = new IgnoreStack();
stack.push_gitignore('', 'build/\n*.log\n'); // a .gitignore (anchor '' = root)
stack.push_tsv('', '!keep.log\n'); // a .formatignore, evaluated after the gitignores
stack.is_ignored('build/out.js', false); // → true
stack.is_ignored('keep.log', false); // → false (the tsv layer re-includes it)
```

## Status

0.x — pre-release. API may change.

## License

[MIT](LICENSE)
