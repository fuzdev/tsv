# tsv

[<img src="static/logo.svg" alt="a pixelated orange quill drawing a precise line" align="right" width="192" height="192">](https://tsv.fuz.dev/)

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust - [tsv.fuz.dev](https://tsv.fuz.dev/)

tsv is a toolchain for TypeScript/JS, CSS, and Svelte in Rust.
The first release has a formatter that closely follows [Prettier](https://prettier.io/) +
[prettier-plugin-svelte](https://github.com/sveltejs/prettier-plugin-svelte),
and a drop-in replacement for [Svelte](https://svelte.dev/)'s parser +
[acorn](https://github.com/acornjs/acorn) +
[acorn-typescript](https://github.com/sveltejs/acorn-typescript).

Compared to Oxc, Biome, and SWC, tsv is a set of focused tools, not a generic language platform,
so the focus is web standards and there's no support for JSX/SCSS/etc,
beyond Svelte as the only JS framework.
The extensibility story is currently limited to using its Rust crates as libraries (or forking);
bridging to JS or WASM plugins is an open question (leaning against).

tsv prioritizes, in order:

1. correctness (Svelte and TypeScript conformance, spec adherence for HTML/CSS/JS)
2. speed
3. binary size and memory usage
4. extensibility (valued but deprioritized)

See the [benchmarks](https://tsv.fuz.dev/docs/benchmarks) for stats.
Compared to Oxc and Biome, tsv (v0.2, not yet published) is significantly faster,
smaller, and uses less memory to parse and format its supported languages.

This is an early release, and reports and feedback are appreciated -
see the [issues](https://github.com/fuzdev/tsv/issues)
and [discussions](https://github.com/fuzdev/tsv/discussions).

AI disclosure: this codebase is mostly LLM-generated, and the usual caveats apply.
The first release took 7 months and ~1800 manual commits.
It's a high-effort project that prioritizes quality.

## Status

pre-alpha - v0.1 is for feedback only not production use; tsv v0.2 is closer but not yet published

## About

tsv is derived from:

- HTML/CSS/JS
- TypeScript
- Svelte
- Prettier and prettier-plugin-svelte

tsv's features:

- [x] formatter following Prettier + prettier-plugin-svelte (with intentional divergences)
- [x] parser for TypeScript/JS + CSS + Svelte, drop-in for Svelte+acorn+acorn-typescript
- [ ] [vscode formatter plugin](https://github.com/fuzdev/vscode_extension_tsv_format) - `fuzdev.tsv-format`
- [ ] ts-to-js conversion (types-to-whitespace only)
- [ ] module lexer

Future features (unknown order):

- minifier
- JSON support
- HTML support (formatting as Svelte isn't correct e.g. with whitespace and `{`,
  but the lift to support it is small)
- JS parsing diagnostics (test262 negative cases)
- CSS error recovery (recover past invalid CSS per the spec)
- later:
  - TypeScript 7 integration (the Go impl), unlocking:
    - svelte-check replacement
    - LSP
    - linter - type aware, initially focused on serializable data-only plugins for extensibility
  - Svelte compiler (exact mirror, maybe out of scope, see [rsvelte](https://github.com/baseballyama/rsvelte))
  - bundling is probably out of scope 
  - [discussion](https://github.com/fuzdev/tsv/discussions) welcome

## Install

tsv ships three WASM packages to npm (native builds will arrive with the v0.2 release):

- [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) - the full tool (formatter + parser) with a `tsv` CLI
- [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm) - formatter only (smaller)
- [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm) - parser + JSON AST only (smallest)

```bash
npm i @fuzdev/tsv_wasm
npx tsv format src                # if installed locally
npx @fuzdev/tsv_wasm format src   # or without installing first
```

```typescript
import {format_svelte} from '@fuzdev/tsv_wasm';
const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

```typescript
import {parse_svelte, type Root} from '@fuzdev/tsv_wasm';
const ast: Root = parse_svelte('<script>const x = 1;</script>');
```

Both `parse_svelte` and `format_svelte` import the same way
from `@fuzdev/tsv_format_wasm` and `@fuzdev/tsv_parse_wasm`.
As with other WASM packages, it works without setup in Node.js/Bun/Deno,
but browsers must call `await init()`.
See the [website docs](https://tsv.fuz.dev/docs)
and package READMEs for the full API and CLI flags:

- [crates/tsv_wasm/README_all.md](crates/tsv_wasm/README_all.md)
- [crates/tsv_wasm/README_format.md](crates/tsv_wasm/README_format.md)
- [crates/tsv_wasm/README_parse.md](crates/tsv_wasm/README_parse.md)

Native builds will be published with v0.2, for v0.1 only WASM builds are published.

## Design

- focused on reducing complexity
  - supports Svelte, TypeScript/JS, CSS and planned HTML/JSON - but no JSX/SCSS/etc
  - formater is non-configurable: formatting style is hardcoded to
    Prettier's defaults with Svelte's official repo config
    (`printWidth: 100`, `useTabs: true`, `singleQuote: true`, and
    `trailingComma: 'none'`),
    and there are no config files or CLI options for formatting style;
    i.e. `tsv format` is opinionated like `gofmt`, `zig fmt`, and Python's Black,
    see [CLAUDE.md § Configuration](CLAUDE.md#configuration)
  - pushes complexity and mess to the printer and JSON conversion,
    out of the parser and internal AST,
    keeping the model clean for the other planned tools
- drop-in for Svelte's JS tools
  - tsv can generate a public JSON AST that should exactly match
    Svelte 5's modern AST with acorn and acorn-typescript
    (see [docs/conformance_svelte.md](docs/conformance_svelte.md)),
    and tsv has its own internal optimal AST
  - the parser can also emit optimized JSON that drops the per-node `loc` and
    Svelte `name_loc` objects, mirroring acorn's `locations: false` for improved performance
    (`parse --no-locations` or the `parse_*_no_locations` bindings)
- compatible with Prettier, with generic rethought APIs
  - formatting is similar Prettier and prettier-plugin-svelte for the common case,
    but intentionally diverges in some cases and fixes numerous bugs
    (see [docs/conformance_prettier.md](docs/conformance_prettier.md))
  - `tsv format` discovery honors `.gitignore`, `.prettierignore`, `.formatignore`
    (original to tsv)
    (all 3 use [gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format))
- Rust-only
  - implementation currently does not call or embed a JS runtime
    (open for discussion, needs research into the tradeoffs);
    JS reaches tsv through the WASM bindings, and native N-API bindings will be published with v0.2
  - no C compiler needed to build tsv
- optimal
  - ships optimal binary artifacts: runtime speed and compiled
    code size are priorities, so if all you need is a formatter or parser,
    a minimal build is available (with lang-specific artifacts likely coming),
    and heavier future layers (incremental parsing, CST for LSP) will be feature-gated so they
    don't regress the focused artifacts
- modern and Web-conformant
  - up-to-date with web specs (roughly aiming for late-stage TC39 proposals and up)
  - JS and TS parse in strict mode only - sloppy-mode-only syntax like `with` is
    rejected, while strict-mode early errors (e.g. duplicate params, reserved-word
    bindings) still parse for now, with enforcement deferred to a future
    diagnostics layer. The parse goal defaults to Module, with an opt-in Script
    goal (`--goal script`); since Svelte and TypeScript are inherently strict
    modules, this affects only standalone JS scripts to force modern patterns

Each language is a self-contained Rust crate exposing the same
`parse`/`format`/`convert_ast_json_bytes` functions over its own concrete types - there's no
central `Language` trait, registry, or dynamic dispatch ("closed scope, open convention and crates").
That means builds tree-shake, so the parse build excludes the printers,
and the formatter build excludes the JSON-AST conversion layer.
Languages tree-shake the same way - a TypeScript-only build would exclude
Svelte and CSS entirely (publishing lang-specific builds is a TODO).
Future LSP/incremental features will be later feature-gated layers that don't bloat
these artifacts - see [docs/architecture.md](docs/architecture.md)

tsv currently has no support JS plugins or JS/WASM runtime integration.
JS bridging and WASM plugins will be evaluated to see if the tradeoffs work for tsv's goals,
but the current lean is against, mainly for performance and simplicity.
Forks could maintain custom extensible APIs on some of tsv's crates today
(please share any friction you experience with these cases).

tsv's goal is to be an optimal toolchain for TypeScript and Svelte.
Consumers can use tsv's crates ([not yet published](https://github.com/fuzdev/tsv/issues/140) to crates.io)
to build custom tools independently.
Hard non-goals:

- no style config settings, so on-disk state and caller params
  never change the output for a given input
- no markup for frameworks besides Svelte - no JSX/TSX, Vue, Astro, etc (unlike Biome+Oxc+SWC+friends) -
  but note that you can use tsv's crates and patterns to vibe your own thing
- no SCSS, CSS Modules, LESS, etc - standard CSS with Svelte extensions only
- no strict Prettier conformance -
  see the [conformance doc](https://github.com/fuzdev/tsv/blob/main/docs/conformance_prettier.md)
  and [discussion #1](https://github.com/fuzdev/tsv/discussions/1)

## Docs

- [CLAUDE.md](CLAUDE.md) - development guide (commands, structure, conventions)
- [docs/architecture.md](docs/architecture.md) - the major design decisions
- [docs/directives.md](docs/directives.md) - `format-ignore` / `prettier-ignore` formatting directives
- [docs/cli.md](docs/cli.md) - commands and design
- [docs/conformance_prettier.md](docs/conformance_prettier.md) - where formatting diverges from Prettier (and why)
- [docs/conformance_svelte.md](docs/conformance_svelte.md) - where the parser diverges from Svelte (and why)
- [docs/conformance_test262.md](docs/conformance_test262.md) - ECMAScript parser conformance
- [docs/fixture_overview.md](docs/fixture_overview.md) - fixture system design
- [docs/fixture_workflow.md](docs/fixture_workflow.md) - step-by-step fixture creation
- [docs/fixture_naming.md](docs/fixture_naming.md) - fixture naming conventions and patterns

## Developing

Dev dependencies:

- [Rust](https://rust-lang.org/) - rustc, cargo
- [Deno](https://docs.deno.com/runtime/) - see [deno.json](deno.json) for the tasks
  - uses `npm:` imports from `svelte`, `typescript`, `acorn`,
    `@sveltejs/acorn-typescript`, `prettier`, `prettier-plugin-svelte`

Rust dependencies are kept fairly minimal.
See [CLAUDE.md § Rust Crates](CLAUDE.md#rust-crates-minimal-deps) for the full list.

```bash
# Build workspace (recommended - uses deno tasks)
deno task build          # dev build
deno task dev            # watch mode (requires: cargo install cargo-watch)
deno task check          # all checks (typecheck, tests, audits, lint, fmt)

# Or build directly with cargo
cargo build --workspace
cargo check --workspace  # fast syntax check (no codegen)
cargo test --workspace   # all tests including fixture validation

# Run CLI
cargo run -p tsv_cli parse --content "const x = 1;" --parser typescript
cargo run -p tsv_cli format --content "<div>test</div>" --parser svelte
```

For the full reference see [CLAUDE.md § Commands](CLAUDE.md#commands).

## Project structure

Multi-crate workspace with clean separation of concerns:

```
tsv/
├── Cargo.toml         # workspace root
├── crates/
│   ├── tsv_lang/      # foundation (Span, Location, ParseError)
│   ├── tsv_arena/     # per-thread reusable AST/doc arenas for the bindings' hot loop
│   ├── tsv_html/      # HTML classification and whitespace rules
│   ├── tsv_ignore/    # gitignore-aware discovery matcher (.gitignore/.formatignore/.prettierignore)
│   ├── tsv_discover/  # file-discovery policy (build-output heuristic + safety nets) over tsv_ignore
│   ├── tsv_ts/        # TypeScript parser/formatter (standalone)
│   ├── tsv_css/       # CSS parser/formatter (standalone)
│   ├── tsv_svelte/    # Svelte parser/formatter (uses tsv_ts + tsv_css)
│   ├── tsv_cli/       # unified CLI (binary: `tsv`)
│   ├── tsv_debug/     # dev utilities (binary: `tsv_debug`, uses Deno)
│   ├── tsv_ffi/       # C FFI bindings
│   ├── tsv_wasm/      # WebAssembly bindings
│   └── tsv_napi/      # N-API bindings (Node/Bun native path)
└── tests/             # workspace-level integration tests
```

Each language crate exports a consistent API:

- `parse(source, arena) -> Result<AST>` — the AST allocates into the caller's `bumpalo` arena (the bindings reuse a per-thread arena across calls via `tsv_arena`)
- `format(ast, source) -> String` — plus `format_in(ast, source, doc_arena)`, the same formatter writing through a reusable doc arena for the bindings' hot loop
- `convert_ast_json_bytes(ast, source) -> Vec<u8>` — the wire JSON, emitted directly from the internal AST, with `convert_ast_json_string`/`convert_ast_json` wrappers and span-only `_no_locations` variants alongside (default-on `convert` cargo feature; turn off for parse+format-only builds)

For more details see [CLAUDE.md](CLAUDE.md).

## Credits

tsv is an implementation of various software designs and specs:

Software:

- [Svelte](https://svelte.dev/)
- [Prettier](https://prettier.io/) which was forked
  from [recast](https://github.com/benjamn/recast)'s printer,
  which is based on the algorithms described
  in [A prettier printer](https://homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf)
  by Philip Wadler
- [TypeScript](https://typescriptlang.org/)

Web Standards:

- [WHATWG HTML](https://github.com/whatwg/html) - HTML Living Standard
- [WHATWG DOM](https://github.com/whatwg/dom) - DOM Living Standard
- [W3C CSS Working Group](https://github.com/w3c/csswg-drafts) - CSS specifications
- [TC39 ECMAScript](https://github.com/tc39/ecma262) - JS language specification
- [TC39 test262](https://github.com/tc39/test262) - ECMAScript conformance tests
- [W3C webref](https://github.com/w3c/webref) - Machine-readable web specs
- [web-platform-tests/wpt](https://github.com/web-platform-tests/wpt/) - Test suites for Web platform specs

Claude Code was instrumental to this project,
and tsv wouldn't exist without LLMs because of the high coding labor requirements.
Source code of projects similar to tsv was not used by agents
or consulted by the author unless listed above.
The author learned Rust in 2015 but wrote only simple programs and some abandoned toys before tsv.

## License

[MIT](LICENSE)

The code for the following projects was sometimes read by AI agents while producing tsv,
so their license information is included for completeness:

Svelte
Copyright (c) 2016-2026 [Svelte Contributors](https://github.com/sveltejs/svelte/graphs/contributors)
MIT - https://github.com/sveltejs/svelte/blob/main/LICENSE.md

Prettier
Copyright © James Long and contributors
MIT - https://github.com/prettier/prettier/blob/main/LICENSE
