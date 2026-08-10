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

## Usage

CommonJS and ESM both work; no initialization step:

```javascript
import {format_svelte, format_typescript, parse_typescript} from '@fuzdev/tsv';

const formatted = format_svelte('<script>\nconst   x=1\n</script>');
const ast = parse_typescript('const x = 1;'); // acorn-typescript-shaped JSON AST
```

Formatting: `format_svelte` / `format_typescript` / `format_css` take a source `string` and return the formatted `string`, throwing on a parse error. Formatting itself is non-configurable; the only option is `format_typescript(source, {goal: 'script' | 'module'})` — the parse goal, where `'script'` makes `await` an ordinary identifier and turns `import`/`export`/`import.meta` into syntax errors.

Parsing: `parse_svelte` / `parse_typescript` / `parse_css` return the language's public JSON AST as an object; the `parse_*_json` siblings return the JSON string itself for consumers that forward the wire format without paying `JSON.parse`. All take an optional `{locations?, goal?}` bag: `locations: false` emits the span-only wire (~46% smaller; `loc` stays derivable from `start`/`end` plus the source), and `goal` is TypeScript-only.

Option semantics (identical to the WASM package): unknown keys throw whatever their value; a supported key set to `undefined` means its default — including the TypeScript-only `goal` on the other languages, so one bag forwards to whichever function; a non-object options argument throws, arrays included, which makes `sources.map(format_typescript)` an error — write `sources.map((s) => format_typescript(s))`.

Errors: parse errors and engine errors are thrown JS errors. A Rust panic — always a tsv bug, please report it — is also thrown rather than aborting the process; stack overflow is the one crash that still aborts.
