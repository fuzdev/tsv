# @fuzdev/tsv-format-wasm

> formatter for Svelte, TypeScript, and CSS

Rust-based formatter compiled to WASM. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely, with documented divergences.

tsv is non-configurable: settings are fixed at Prettier's defaults except `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and `trailingComma: 'none'` — no options, like `gofmt` and Black.

Formatting only — for parser / AST extraction, see [`@fuzdev/tsv-parse-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-parse-wasm), or [`@fuzdev/tsv-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-wasm) for both plus a CLI. On Node.js and Bun, [`@fuzdev/tsv`](https://www.npmjs.com/package/@fuzdev/tsv) is the native build of the same API.

Docs and benchmarks: [tsv.fuz.dev](https://tsv.fuz.dev/). Source and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv-format-wasm
```

Requires Node.js 22+; Bun and Deno work too, and browsers via `init()` (below).

TypeScript declarations are bundled. They name the DOM lib's `fetch` and `WebAssembly` types (for `init()` and the compiled module), so a project type-checking them needs `"dom"` in its `lib`, or `skipLibCheck`.

## Usage

Three formatting functions: `format_svelte`, `format_typescript`, `format_css`. Each takes a source `string` and returns the formatted `string`, throwing on a parse error.

### Node.js, Bun, Deno

Zero config — WASM is initialized synchronously at import time:

```javascript
import {format_css, format_svelte, format_typescript} from '@fuzdev/tsv-format-wasm';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

### Browsers and bundlers

Call `await init()` once before formatting. Bundlers that understand `new URL('./file.wasm', import.meta.url)` (Vite, Webpack, Rollup) resolve the WASM asset automatically:

```javascript
import {format_svelte, init} from '@fuzdev/tsv-format-wasm';

await init();
const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

`init_sync({ module })` is also exported for Workers and custom loading.

### Options

Each function also takes an optional trailing options object. Formatting itself is non-configurable, so the one option is TypeScript's parse goal, `format_typescript(source, {sourceType: 'script' | 'module'})`:

- `'module'` — the source is a module, always strict.
- `'script'` — `await` is an ordinary identifier, top-level `import`/`export`, `for await` and `import.meta` are syntax errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse).
- omitted — not the same as `'module'`: the source is formatted as a module and, only if that parse fails, retried as a script. So a legacy sloppy script formats with no options at all, while anything the module grammar accepts is never reinterpreted (formatting reads no goal, so no output changes).

A set value is exact. With none named and both grammars rejecting the source, the error thrown is the one that best explains the failure (the rule is in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#multi-file-formatting)).

`format_svelte`/`format_css` throw on a set `sourceType`. A supported key set to `undefined` reads as its default, so one options object forwards to every formatter. Argument errors are `TypeError`s:

- an unknown key, whatever its value;
- a wrong-typed or invalid value, or a source that is not a string or not well-formed UTF-16 (it holds a lone surrogate);
- a non-object second argument, arrays included — write `sources.map((s) => format_typescript(s))`, not `sources.map(format_typescript)`, which passes the index.

### Errors and depth limits

A source that doesn't parse throws a `SyntaxError` with two own properties, `start` and `loc` (typed `TsvSyntaxError`, exported):

- `start` is the error's UTF-16 offset and `loc` its `{line, column}` (1-based line, 0-based UTF-16 column).
- TypeScript counts ECMAScript line terminators (LF, CR, CRLF, U+2028, U+2029) and a leading BOM; Svelte (`<script>`, `<style>` and template expressions included) and CSS count LF alone and leave a leading BOM out.
- The message's second line starts with `loc` as `line:column + 1`.
- The position is into your own source, CRLF line endings included — the one [`@fuzdev/tsv-parse-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-parse-wasm) reports, except that with no `sourceType`, `format_typescript` may report its script retry's error instead.
- Read `start` and `loc`, not `line` / `column`, which some runtimes put on every `Error`.

```javascript
import {format_typescript} from '@fuzdev/tsv-format-wasm';

try { format_typescript('let a;\nconst = ;'); } catch (e) {
	e instanceof SyntaxError; // true — the message ends with the line and a caret
	[e.start, e.loc]; // [13, {line: 2, column: 6}]
}
```

A Rust panic — always a tsv bug, please report it — surfaces as a `RuntimeError: unreachable` with the real message on `console.error`; the instance survives it, so the next call works.

Deeply nested input has a ceiling: the WASM stack is 1 MiB, and the deepest shapes — nested arrow bodies and member chains — cost several times more of it per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Past the ceiling the call traps with `memory access out of bounds`, and unlike a parse error or a panic it **poisons the instance** — every later call throws the same thing. `reinstantiate()` is the recovery: it synchronously swaps in a fresh instance from the already-compiled module (no recompile — same environment constraints as `init_sync`), and every import keeps working against it. Objects created before the swap (an `IgnoreStack`) are invalidated — rebuild them after: every method on a stale one throws, and `free()` on it is a safe no-op. Real code is nowhere near this ceiling; generated and minified code can be.

### Worker pools

To format across threads, compile once and share: the main entry exports `wasm_module`, the compiled `WebAssembly.Module` behind its exports, and the `@fuzdev/tsv-format-wasm/worker` subpath is the same API without the import-time initialization, so a worker starts from that module instead of reading and compiling the WASM again. Compiled code is shared across isolates, so no worker pays for a second compile. `wasm_module` is the Node/Bun entry's alone — that entry is the one that compiles at import — so in a browser Worker call `await init()` instead, or `postMessage` a `WebAssembly.Module` you compiled yourself.

<!-- typecheck: node -->
```typescript
// main thread
import {Worker} from 'node:worker_threads';
import {wasm_module} from '@fuzdev/tsv-format-wasm';
new Worker(new URL('./worker.js', import.meta.url), {workerData: {wasm_module}});

// worker.js
import {workerData} from 'node:worker_threads';
import {format_typescript, init_sync} from '@fuzdev/tsv-format-wasm/worker';
init_sync({module: workerData.wasm_module});
```

### File scoping (`IgnoreStack`)

For tooling that needs tsv's exact file scoping, this package also exports an `IgnoreStack` class — the same hierarchical, git-faithful matcher the `tsv` CLI uses (per-directory `.gitignore` layers plus one tsv layer per directory, fed by `.formatignore` or, failing that, `.prettierignore`) to decide which files it formats. Build it from a repo's ignore files (one layer per directory, anchored at that directory), then query per path. Walking directories is the caller's job, but the class also carries tsv's discovery policy — `classify_dir`, `should_format_file`, `is_path_pruned`, `path_shadow_warning`, `excluded_argument_warning`, `unsupported_extension_error`, `shadow_warning`, `prettierignore_outside_repo_warning`, `prettierignore_shadowed_warning`, and `gitignore_symlink_warning` — so a walker reproduces the CLI's decisions, and its warnings, exactly.

```javascript
import {IgnoreStack} from '@fuzdev/tsv-format-wasm';

const stack = new IgnoreStack();
stack.push_gitignore('', 'build/\n*.log\n'); // a .gitignore (anchor '' = root)
stack.push_formatignore('', '!keep.log\n'); // a .formatignore, evaluated after the gitignores
stack.is_ignored('build/out.js', false); // → true
stack.is_ignored('keep.log', false); // → false (the tsv layer re-includes it)
```

## Status

Near production-ready. A long tail of rare bugs remains and APIs may still change; reports are appreciated.

## License

[MIT](LICENSE)
