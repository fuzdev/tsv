# @fuzdev/tsv

> native formatter and parser for Svelte, TypeScript, and CSS (N-API)

Rust-based formatter + parser as a prebuilt native addon for Node.js and Bun. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (documented divergences), and a drop-in replacement for the canonical parsers' JSON AST (acorn + acorn-typescript, Svelte's modern parser, `parseCss`).

The API mirrors [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) export for export — same names, same options, same errors — so the two are drop-in swaps: this package is the fast native path; the WASM one runs everywhere (browsers included) and is the fallback for platforms without a prebuilt binding.

Source of truth, full docs, and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv
```

The right platform binary installs automatically (per-platform `optionalDependencies`). Prebuilt platforms:

- `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl` (Alpine)
- `darwin-arm64`
- `win32-x64`

On any other platform the import throws with a pointer at `@fuzdev/tsv_wasm`.

## CLI

```bash
npx @fuzdev/tsv format src        # format .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css in place, recursively
npx @fuzdev/tsv format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv format --list .   # list the in-scope files, format nothing
npx @fuzdev/tsv parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed (`npm i -D @fuzdev/tsv`), the bin is `tsv` — and here it is the **real native CLI**: the platform package ships tsv's production Rust binary beside the addon, and the bin execs it directly (the esbuild/biome shape), so you get the native CLI's exact contract — multi-file parallelism (`--jobs`), parallel discovery, native error paths. (Both binaries ship because neither can play the other's role: an addon can't be exec'd as a process, and an executable can't be loaded as an in-process module.) Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`), `.svelte`, and `.css` with gitignore-aware discovery. **Inside a git repo** it honors `.gitignore`, `.formatignore`, and `.prettierignore` (all hierarchical, like git), scoped to the repo so results are reproducible. **Outside a repo** it honors only `.formatignore`. With no `.gitignore` in scope (in or out of a repo), discovery falls back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped; an explicitly named file skips the ignore files, but its extension must still be one tsv formats.

`format --list` prints the discovered in-scope files without formatting — a read-only view of what `format` would touch. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. For TypeScript, `--goal script|module` (default `module`; for `format`, `--content`/`--stdin` only) selects the parse goal — at `script`, `await` is an ordinary identifier and `import`/`export`/`import.meta` are errors. `parse --no-locations` emits the span-only wire (no per-node `loc`; Svelte also no `name_loc`; no-op for CSS). Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

Files format in parallel; `--jobs N` overrides the worker count. (If the platform package's CLI binary is missing or unrunnable, the bin degrades to a JS mirror of the same contract over the addon — which parallelizes too, on worker threads; on a platform with no prebuilt package at all, use [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) instead.)

## Usage

The package is ESM, and there is no initialization step (a CommonJS host loads it with `await import('@fuzdev/tsv')`):

```javascript
import {format_svelte, format_typescript, parse_typescript} from '@fuzdev/tsv';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const ast = parse_typescript('const x = 1;'); // acorn-typescript-shaped JSON AST
```

Formatting: `format_svelte` / `format_typescript` / `format_css` take a source `string` and return the formatted `string`, throwing on a parse error. Formatting itself is non-configurable; the only option is `format_typescript(source, {goal: 'script' | 'module'})` — the parse goal, where `'script'` makes `await` an ordinary identifier and turns `import`/`export`/`import.meta` into syntax errors.

Parsing: `parse_svelte` / `parse_typescript` / `parse_css` return the language's public JSON AST as an object; the `parse_*_json` siblings return the JSON string itself for consumers that forward the wire format without paying `JSON.parse`. All take an optional `{locations?, goal?}` bag: `locations: false` emits the span-only wire (much smaller; `loc` stays derivable from `start`/`end` plus the source, via the bundled `reconstruct_locations` / `create_locator` / `loc_of` — which throw rather than guess on the two Svelte shapes the span-only wire can't disambiguate, a lone `\r`/U+2028/U+2029 or a block binding split from its `: T` by a newline), and `goal` is TypeScript-only. TypeScript types for the AST are bundled in `tsv_ast.d.ts` and re-exported from the package (`import type {...} from '@fuzdev/tsv'`).

Option semantics (identical to the WASM package): unknown keys throw whatever their value; a supported key set to `undefined` means its default — including the TypeScript-only `goal` on the other languages, so one bag forwards to whichever function; a non-object options argument throws, arrays included, which makes `sources.map(format_typescript)` an error — write `sources.map((s) => format_typescript(s))`.

File scoping: `IgnoreStack` is tsv's own hierarchical, git-faithful matcher plus its discovery policy, exported so tooling can reproduce exactly which files `tsv format` would touch — the same class `@fuzdev/tsv_wasm` exports.

Errors: parse errors and engine errors are thrown JS errors. A Rust panic — always a tsv bug, please report it — is also thrown rather than aborting the process; stack overflow is the one crash that still aborts, as a bare `SIGSEGV`. Its depth is your thread's, not the addon's: a main thread has the process stack limit (commonly 8 MiB) and a `worker_threads` worker has Node's 4 MiB default, so a worker reaches about half as deep; the deepest shapes — nested arrow bodies and member chains — cost several times more stack per level than nested parens (the measured ceiling per nesting shape is in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Raise the worker's stack with `new Worker(path, {resourceLimits: {stackSizeMb: 16}})` if you format generated or minified input. The bundled `tsv` CLI is unaffected; it sizes its own.

## Status

0.x — pre-release. API may change.

## License

[MIT](LICENSE)
