# @fuzdev/tsv

> native formatter and parser for Svelte, TypeScript, and CSS (N-API)

Rust-based formatter + parser as a prebuilt native addon for Node.js and Bun. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (documented divergences), and a drop-in replacement for the canonical parsers' JSON AST (acorn + acorn-typescript, Svelte's modern parser, `parseCss`).

The API mirrors [`@fuzdev/tsv-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-wasm) export for export — same names, same options, same errors — minus only what a WASM engine needs and this one doesn't (`init`, `init_sync`, `wasm_module`, `reinstantiate`, the `./worker` subpath, and the WASM `IgnoreStack`'s `free()` / `[Symbol.dispose]()`, which a GC-managed native class doesn't need), so the two are drop-in swaps: this package is the fast native path; the WASM one runs everywhere (browsers included) and is the fallback for platforms without a prebuilt binding.

Docs and benchmarks: [tsv.fuz.dev](https://tsv.fuz.dev/). Source and conformance notes: [github.com/fuzdev/tsv](https://github.com/fuzdev/tsv).

## Install

```bash
npm i @fuzdev/tsv
```

Requires Node.js 22+ or Bun.

The right platform binary installs automatically (per-platform `optionalDependencies`). Prebuilt platforms:

- `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl` (Alpine)
- `darwin-arm64`, `darwin-x64`
- `win32-x64`

On any other platform the import throws with a pointer at `@fuzdev/tsv-wasm`.

The same native `tsv` CLI binary is also attached to each [GitHub Release](https://github.com/fuzdev/tsv/releases) (one per platform above, with a `SHA256SUMS`; every asset carries a build provenance attestation: `gh attestation verify <file> -R fuzdev/tsv`) for use without npm.

## CLI

```bash
npx @fuzdev/tsv format src        # format .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css in place, recursively
npx @fuzdev/tsv format --check .  # CI: exit 1 if anything would change
npx @fuzdev/tsv format --list .   # list the in-scope files, format nothing
npx @fuzdev/tsv parse file.svelte # JSON AST to stdout (--pretty to indent)
```

Installed (`npm i -D @fuzdev/tsv`), the bin is `tsv` — and here it is the **real native CLI**: the platform package ships tsv's production Rust binary beside the addon, and the bin execs it directly (the esbuild/biome shape), so you get the native CLI's exact contract — multi-file parallelism (`--jobs`), parallel discovery, native error paths. (Both binaries ship because neither can play the other's role: an addon can't be exec'd as a process, and an executable can't be loaded as an in-process module.)

Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`), `.svelte`, and `.css` with gitignore-aware discovery:

- **Inside a git repo** it honors `.gitignore`, then `.formatignore` — both hierarchical, like git — plus `.prettierignore` as a drop-in fallback, read in any directory that has no `.formatignore` of its own (a sibling `.formatignore` shadows it, with a warning); all scoped to the repo so results are reproducible.
- **Outside a repo** it honors only `.formatignore`.
- With no `.gitignore` in scope (in or out of a repo), discovery falls back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped by a walk.
- A path you name — file or directory — is bounded by the ignore files alone: one they exclude is skipped (with a warning, unless it is a file your `.formatignore` or `.prettierignore` excludes), while the built-in skips never apply to it; a named file's extension must still be one tsv formats.

`format --list` prints the discovered in-scope files without formatting — a read-only view of what `format` would touch. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. For TypeScript, `--source-type script|module` (`parse` defaults to `module`; `format` takes an unset flag as *none named* — module, retried as a script, except a `.mjs`/`.mts` path, a module by its own name, which takes no retry — and accepts it on `--content`/`--stdin` only) selects the parse goal — at `script`, `await` is an ordinary identifier, top-level `import`/`export`, `for await` and `import.meta` are errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse). `parse` emits the span-only wire (no per-node `loc`; Svelte also no `name_loc`); `parse --locations` adds them. Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

Files format in parallel; `--jobs N` overrides the worker count. (If the platform package's CLI binary is missing or unrunnable, the bin degrades to a JS mirror of the same contract over the addon — which parallelizes too, on worker threads; on a platform with no prebuilt package at all, use [`@fuzdev/tsv-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-wasm) instead.) Both packages claim the `tsv` bin name, so install one or the other in a project.

## Usage

The package is ESM, and there is no initialization step (a CommonJS host loads it with `await import('@fuzdev/tsv')`):

```javascript
import {format_svelte, format_typescript, parse_typescript} from '@fuzdev/tsv';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const ast = parse_typescript('const x = 1;'); // acorn-typescript-shaped JSON AST, span-only
```

### Formatting

`format_svelte` / `format_typescript` / `format_css` take a source `string` and return the formatted `string`, throwing on a parse error. Formatting itself is non-configurable; the only option is TypeScript's parse goal, `format_typescript(source, {sourceType: 'script' | 'module'})`:

- `'module'` — the source is a module, always strict.
- `'script'` — `await` is an ordinary identifier, top-level `import`/`export`, `for await` and `import.meta` are syntax errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse).
- omitted — not the same as `'module'`: the source is formatted as a module and, only if that parse fails, retried as a script. So a legacy sloppy script formats with no options at all, while anything the module grammar accepts is never reinterpreted.

A set value is exact. With none named and both grammars rejecting the source, the error thrown is the one that best explains the failure (the rule is in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#multi-file-formatting)). `parse_typescript` has no such fallback; there an omitted `sourceType` means `'module'`.

### Parsing

`parse_svelte` / `parse_typescript` / `parse_css` return the language's public JSON AST as an object — span-only by default, `start`/`end` offsets on every node and no per-node `loc`; the `parse_*_json` siblings return that wire as a JSON string for consumers that forward it without paying `JSON.parse`. The object parsers take an optional `{locations?, sourceType?}` bag, the `_json` ones `{sourceType?}`, and `sourceType` is TypeScript-only. TypeScript types for the AST are bundled in `tsv_ast.d.ts` and re-exported from the package (`import type {...} from '@fuzdev/tsv'`).

`locations: true` adds `loc` (and Svelte's `name_loc`), computed in JS after the parse by `reconstruct_locations`. It is exported too, with `create_locator(source, {language})` for sparse lookups (`language` required), whose `position_at(offset)` / `loc_of(node)` throw a `RangeError` for an offset or span the source doesn't hold; the whole-tree forms (`reconstruct_locations`, the locator's `reconstruct(ast)`) check nothing per node. Both are also the `@fuzdev/tsv/locations` entry point, pure JS that loads no native addon.

### Options

Options behave as in `@fuzdev/tsv-wasm`: a supported key set to `undefined` reads as its default, and a `sourceType`-only bag forwards to any function (`sourceType: undefined` included on Svelte and CSS; a `locations` key, whatever its value, throws anywhere but an object parser). Argument errors are `TypeError`s:

- an unknown key, whatever its value;
- a wrong-typed or invalid value, or a non-string source;
- a non-object second argument, arrays included — write `sources.map((s) => format_typescript(s))`, not `sources.map(format_typescript)`, which passes the index.

### File scoping

`IgnoreStack` is tsv's own hierarchical, git-faithful matcher plus its discovery policy (`classify_dir`, `should_format_file`, `is_path_pruned`, `excluded_argument_warning`, `unsupported_extension_error`, and the warning templates), exported so tooling can reproduce exactly which files `tsv format` would touch — the same class `@fuzdev/tsv-wasm` exports; [`@fuzdev/tsv-format-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-format-wasm) documents it with examples.

### Errors

A source that doesn't parse throws a `SyntaxError` — from a parser and a formatter alike, the same one `@fuzdev/tsv-wasm` throws — with two own properties, `start` and `loc` (typed `TsvSyntaxError`, exported), in the same coordinates as the AST's own positions:

- `start` is the error's UTF-16 offset and `loc` its `{line, column}` (1-based line, 0-based UTF-16 column), equal to `create_locator(source, {language}).position_at(start)`.
- TypeScript counts ECMAScript line terminators (LF, CR, CRLF, U+2028, U+2029) and a leading BOM; Svelte (`<script>`, `<style>` and template expressions included) and CSS count LF alone and leave a leading BOM out.
- The message's second line starts with `loc` as `line:column + 1`.
- A formatter reports positions in your own source, CRLF line endings included — the error a parser reports, except that with no `sourceType`, `format_typescript` may report its script retry's error instead.
- Read `start` and `loc`, not `line` / `column`, which some runtimes put on every `Error`.

```javascript
import {format_typescript} from '@fuzdev/tsv';

try { format_typescript('let a;\nconst = ;'); } catch (e) {
	e instanceof SyntaxError; // true — the message ends with the line and a caret
	[e.start, e.loc]; // [13, {line: 2, column: 6}]
}
```

A Rust panic — always a tsv bug, please report it — is also thrown rather than aborting the process; stack overflow is the one crash that still aborts, as a bare `SIGSEGV`. Its depth is your thread's, not the addon's: a main thread has the process stack limit (commonly 8 MiB) and a `worker_threads` worker has Node's 4 MiB default, so a worker reaches about half as deep; the deepest shapes — nested arrow bodies and member chains — cost several times more stack per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Raise the worker's stack with `new Worker(path, {resourceLimits: {stackSizeMb: 16}})` if you format generated or minified input. The bundled `tsv` CLI is unaffected; it sizes its own. Reconstructing `loc` — `{locations: true}`, or `reconstruct_locations` — runs in JS without recursion, so it reaches as deep as the parse does.

## Status

Near production-ready. A long tail of rare bugs remains and APIs may still change; reports are appreciated.

## License

[MIT](LICENSE)
