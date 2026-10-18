# @fuzdev/tsv

> native formatter and parser for Svelte, TypeScript, and CSS, and a TypeScript type stripper (N-API)

Rust-based formatter + parser + type stripper as a prebuilt native addon for Node.js and Bun. A near-Prettier formatter that tracks **Prettier** + **prettier-plugin-svelte** closely (documented divergences), and a parser whose AST is compatible with Svelte's parser + acorn + acorn-typescript (and `parseCss`).

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
npx @fuzdev/tsv strip file.ts     # TypeScript to JS on stdout, every position kept
```

Installed (`npm i -D @fuzdev/tsv`), the bin is `tsv` — and here it is the **real native CLI**: the platform package ships tsv's production Rust binary beside the addon, and the bin execs it directly (the esbuild/biome shape), so you get the native CLI's exact contract — multi-file parallelism (`--jobs`), parallel discovery, native error paths. (Both binaries ship because neither can play the other's role: an addon can't be exec'd as a process, and an executable can't be loaded as an in-process module.)

Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`), `.svelte`, and `.css` with gitignore-aware discovery:

- **Inside a git repo** it honors `.gitignore`, then `.formatignore` — both hierarchical, like git — plus `.prettierignore` as a drop-in fallback, read in any directory that has no `.formatignore` of its own (a sibling `.formatignore` shadows it, with a warning); all scoped to the repo so results are reproducible.
- **Outside a repo** it honors only `.formatignore`.
- With no `.gitignore` in scope (in or out of a repo), discovery falls back to skipping hidden directories and `dist`/`build`/`target`. `node_modules` and VCS directories are always skipped by a walk.
- A path you name — file or directory — is bounded by the ignore files alone: one they exclude is skipped (with a warning, unless it is a file your `.formatignore` or `.prettierignore` excludes), while the built-in skips never apply to it; a named file's extension must still be one tsv formats.

`format --list` prints the discovered in-scope files without formatting — a read-only view of what `format` would touch. `--content <source>` / `--stdin` (with `--parser svelte|typescript|css`) format or parse strings to stdout. For TypeScript, `--source-type script|module` (`parse` defaults to `module`; `format` takes an unset flag as *none named* — module, retried as a script, except a `.mjs`/`.mts` path, a module by its own name, which takes no retry — and accepts it on `--content`/`--stdin` only) selects the parse goal — at `script`, `await` is an ordinary identifier, top-level `import`/`export`, `for await` and `import.meta` are errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse). `parse` emits the span-only wire (no per-node `loc`; Svelte also no `name_loc`); `parse --locations` adds them. Exit codes — `format`: 0 clean, 1 would-change (`--check`), 2 errors; `parse`: 0 ok, 1 error.

`strip` takes one TypeScript input — a `.ts`/`.mts`/`.cts` path (a `.svelte.ts` rune module included; a JavaScript path is refused rather than read as TypeScript), `--content`, or `--stdin` — and prints it as JS with every type blanked to whitespace ([Stripping types](#stripping-types)); `--check` prints nothing and only sets the exit code. The parse goal is `format`'s: a module, retried as a script if that fails (a `.mts` path takes no retry), or the one `--source-type script|module` names for `--content`/`--stdin`. Exit codes — `strip`: 0 stripped, 1 refused (TypeScript that type blanking cannot erase, such as an `enum`), 2 a parse error, an unreadable file or a usage error.

Files format in parallel; `--jobs N` overrides the worker count. (If the platform package's CLI binary is missing or unrunnable, the bin degrades to a JS mirror of the same contract over the addon — which parallelizes too, on worker threads; on a platform with no prebuilt package at all, use [`@fuzdev/tsv-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-wasm) instead.) Both packages claim the `tsv` bin name, so install one or the other in a project.

## Usage

The package is ESM, and there is no initialization step (a CommonJS host loads it with `await import('@fuzdev/tsv')`):

```javascript
import {format_svelte, format_typescript, parse_typescript} from '@fuzdev/tsv';

const formatted = format_svelte('<script>\nconst   x=1\n</script>'); // {ok: true, code}
const parsed = parse_typescript('const x = 1;'); // {ok: true, ast}: acorn-typescript-shaped, span-only
if (!parsed.ok) throw new Error(parsed.error.message);
```

### Formatting

`format_svelte` / `format_typescript` / `format_css` take a source `string` and return `{ok: true, code}` with the formatted `string`, or `{ok: false, error}` for a source they cannot format ([Errors](#errors)). Formatting itself is non-configurable; the only option is TypeScript's parse goal, `format_typescript(source, {sourceType: 'script' | 'module'})`:

- `'module'` — the source is a module, always strict.
- `'script'` — `await` is an ordinary identifier, top-level `import`/`export`, `for await` and `import.meta` are syntax errors, and the code is sloppy unless its own `"use strict"` prologue makes it strict (so `with` and legacy octal literals/escapes parse).
- omitted — not the same as `'module'`: the source is formatted as a module and, only if that parse fails, retried as a script. So a legacy sloppy script formats with no options at all, while anything the module grammar accepts is never reinterpreted.

A set value is exact. With none named and both grammars rejecting the source, the error returned is the one that best explains the failure (the rule is in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#multi-file-formatting)). `parse_typescript` has no such fallback; there an omitted `sourceType` means `'module'`.

### Parsing

`parse_svelte` / `parse_typescript` / `parse_css` return `{ok: true, ast}` with the language's public JSON AST as an object — span-only by default, `start`/`end` offsets on every node and no per-node `loc`; the `parse_*_json` siblings return that wire as a JSON string, `{ok: true, json}`, for consumers that forward it without paying `JSON.parse`. A source that doesn't parse returns `{ok: false, error}` ([Errors](#errors)). The object parsers take an optional `{locations?, sourceType?}` bag, the `_json` ones `{sourceType?}`, and `sourceType` is TypeScript-only. TypeScript types for the AST are bundled in `tsv_ast.d.ts` and re-exported from the package (`import type {...} from '@fuzdev/tsv'`).

`locations: true` adds `loc` (and Svelte's `name_loc`), computed in JS after the parse by `reconstruct_locations`. It is exported too, with `create_locator(source, {language})` for sparse lookups (`language` required), whose `position_at(offset)` / `loc_of(node)` throw a `RangeError` for an offset or span the source doesn't hold (a `TypeError` for an offset that isn't a number, or a node whose `start`/`end` getter throws); the whole-tree forms (`reconstruct_locations`, the locator's `reconstruct(ast)`) check nothing per node, and take an acyclic tree — the parse's own, or a clone of it. Both are also the `@fuzdev/tsv/locations` entry point, pure JS that loads no native addon.

### Stripping types

`strip_typescript(source)` turns TypeScript into JS by blanking every type to whitespace — no transform, no sourcemap: every surviving character keeps its line and UTF-16 column, line terminators are kept verbatim, and a type-free source comes back unchanged. Where blanking alone would let the next line continue the statement, one `;`, `(` or `)` takes a blanked character's place.

It returns a result rather than throwing for what it finds in the source: `{ok: true, code}`, or `{ok: false, error}` with `error.kind` `'parse'` (the error a `parse_typescript` of the source returns at the goal the error belongs to) or `'unsupported'` (TypeScript that type blanking cannot erase — an `enum`, a value `namespace`, a parameter property, `import =`, `export =`, `<T>x`, or an `as` / `satisfies` whose removal would regroup its expression — named by `error.construct`, with its span as `start`/`end` and `loc`, a one-line `message`, and a `frame` with the line and a caret).

```javascript
import {strip_typescript} from '@fuzdev/tsv';

const result = strip_typescript('let id: string = "a";\nenum Color { Red }\n');
if (!result.ok && result.error.kind === 'unsupported') {
	result.error.construct; // 'enum'
	result.error.loc.start; // {line: 2, column: 0}
}
```

The one option is `sourceType`, read as `format_typescript` reads it: omitted, the source is a module retried as a script if that fails, so a legacy sloppy script strips as the sloppy script it is; `'script'` or `'module'` is exact.

### Options

Options behave as in `@fuzdev/tsv-wasm`: a supported key set to `undefined` reads as its default, and a `sourceType`-only bag forwards to any function (`sourceType: undefined` included on Svelte and CSS; a `locations` key, whatever its value, throws anywhere but an object parser). Argument errors are thrown `TypeError`s:

- an unknown key, whatever its value;
- a wrong-typed or invalid value, or a source that is not a string or not well-formed UTF-16 (it holds a lone surrogate);
- a non-object second argument, arrays included — write `sources.map((s) => format_typescript(s))`, not `sources.map(format_typescript)`, which passes the index.

### File scoping

`IgnoreStack` is tsv's own hierarchical, git-faithful matcher plus its discovery policy (`classify_dir`, `should_format_file`, `is_path_pruned`, `path_shadow_warning`, `excluded_argument_warning`, `unsupported_extension_error`, and the warning templates), exported so tooling can reproduce exactly which files `tsv format` would touch — the same class `@fuzdev/tsv-wasm` exports; [`@fuzdev/tsv-format-wasm`](https://www.npmjs.com/package/@fuzdev/tsv-format-wasm) documents it with examples. The two answer every call its declared types admit alike; unlike the facade's functions, the class checks its arguments in each engine's own binding layer, so a wrong-typed argument is refused or coerced differently by each.

### Errors

A source that doesn't parse returns `{ok: false, error}` — from a parser, a formatter and the strip alike, the same result `@fuzdev/tsv-wasm` returns — with `error` a plain object (no stack; `postMessage`-able): `{kind: 'parse', message, frame, start, end, loc}`, typed `TsvParseError` (exported), in the same coordinates as the AST's own positions:

- `message` says what went wrong, on one line; `frame` is the excerpt under it — a `line:column` header and the source line, then a caret line — so `` `${message}\n${frame}` `` is the text the CLI prints.
- `start` and `end` are the error's UTF-16 offsets — a parse error is a point, so `end === start` — and `loc` is their `{start: {line, column}, end: {line, column}}` (1-based line, 0-based UTF-16 column), shaped like a node's: `create_locator(source, {language}).loc_of(result.error)` equals it.
- TypeScript counts ECMAScript line terminators (LF, CR, CRLF, U+2028, U+2029) and a leading BOM; Svelte (`<script>`, `<style>` and template expressions included) and CSS count LF alone and leave a leading BOM out.
- The frame's header is `loc.start` as `line:column + 1`.
- A formatter reports positions in your own source, CRLF line endings included — the error a parser reports (its `message` can differ where it quotes source text, which a formatter reads with line endings folded to LF), except that with no `sourceType`, `format_typescript` may report its script retry's error instead.

```javascript
import {format_typescript} from '@fuzdev/tsv';

const result = format_typescript('let a;\nconst = ;');
if (!result.ok) {
	result.error.kind; // 'parse'
	result.error.message; // "Expected identifier or destructuring pattern, found '='"
	result.error.frame; // '2:7 const = ;\n          ^ here'
	[result.error.start, result.error.loc.start]; // [13, {line: 2, column: 6}]
}
```

`format_svelte` refuses one source that does parse: a lone CR (not part of a CRLF) with comment text after it inside a `//` comment between a tag's attributes, which Svelte ends at LF alone — the formatter's CR-to-LF fold would end that comment early and turn the rest of its line into markup. The refusal is `{kind: 'refused', message, frame, start, end, loc}` (typed `FormatRefusedError`), its span the CR.

What still throws is what no result can report: an argument error (a `TypeError`, above), a source over the 4 GiB size cap, and the engine failures below.

A Rust panic — always a tsv bug, please report it — is also thrown rather than aborting the process; stack overflow is the one crash that still aborts, as a bare `SIGSEGV`. Its depth is your thread's, not the addon's: a main thread has the process stack limit (commonly 8 MiB) and a `worker_threads` worker has Node's 4 MiB default, so a worker reaches about half as deep; the deepest shapes — nested arrow bodies and member chains — cost several times more stack per level than nested parens (the per-shape stack costs and each surface's ceiling are in the repo's [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md#recursion-depth)). Raise the worker's stack with `new Worker(path, {resourceLimits: {stackSizeMb: 16}})` if you format generated or minified input. The bundled `tsv` CLI is unaffected; it sizes its own. Reconstructing `loc` — `{locations: true}`, or `reconstruct_locations` — runs in JS without recursion, so it reaches as deep as the parse does.

## Status

The formatters are near production-ready. A long tail of rare bugs remains and APIs may still change; reports are appreciated.

## License

[0BSD](LICENSE)
