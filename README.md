# tsv

> a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS - [tsv.fuz.dev](https://tsv.fuz.dev/)

tsv is a toolchain for Svelte, TypeScript/JS, and CSS, written in Rust.
The first release has a formatter that closely follows [Prettier](https://prettier.io/) +
[prettier-plugin-svelte](https://github.com/sveltejs/prettier-plugin-svelte),
and a drop-in replacement for [Svelte](https://svelte.dev/)'s parser +
[acorn](https://github.com/acornjs/acorn) +
[acorn-typescript](https://github.com/sveltejs/acorn-typescript).

Compared to Oxc, Biome, and SWC, tsv is a set of focused tools, not a generic language platform,
so the focus is web standards and there's no support for JSX/SCSS/etc,
beyond Svelte as the only JS framework.
The extensibility story is currently limited to using its Rust crates as libraries;
bridging to JS or WASM plugins is an open question, but may not be supported.

tsv prioritizes, in order:

1. correctness (Svelte and TypeScript conformance, spec adherence for HTML/CSS/JS)
2. speed
3. binary size
4. memory usage
5. and lastly, extensibility (deprioritized compared to Oxc/Biome/SWC)

See the [benchmarks](https://tsv.fuz.dev/docs/benchmarks) for stats.
Compared to Oxc and Biome, tsv is significantly faster, smaller, and uses less memory.

This is an early release, and reports and feedback are appreciated -
see the [issues](https://github.com/fuzdev/tsv/issues)
and [discussions](https://github.com/fuzdev/tsv/discussions).

AI disclosure: this codebase is mostly LLM-generated, and the usual caveats apply.
The first release took 7 months and ~1800 manual commits.
It's a high-effort project that prioritizes quality.

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
import {format_svelte} from '@fuzdev/tsv_format_wasm';
const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

```typescript
import {parse_svelte, type Root} from '@fuzdev/tsv_parse_wasm';
const ast: Root = parse_svelte('<script>const x = 1;</script>');
```

Both `parse_svelte` and `format_svelte` import the same way from `@fuzdev/tsv_wasm`.
As with other WASM packages, it works without setup in Node.js/Bun/Deno,
but browsers must call `await init()`.
See the [website docs](https://tsv.fuz.dev/docs)
and package READMEs for the full API and CLI flags:

- [crates/tsv_wasm/README_all.md](crates/tsv_wasm/README_all.md)
- [crates/tsv_wasm/README_format.md](crates/tsv_wasm/README_format.md)
- [crates/tsv_wasm/README_parse.md](crates/tsv_wasm/README_parse.md)

Native builds will be published with v0.2, for v0.1 only WASM builds are published.

## Design

- supports Svelte, TypeScript/JS, CSS (and planned HTML/JSON)
- non-configurable: formatter settings are fixed at Prettier's defaults except
  `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and
  `trailingComma: 'none'` (no trailing comma on multiline lists, matching the
  Svelte project's own Prettier config),
  and there are no config files or CLI options for formatting style;
  tsv is opinionated like `gofmt` and Python's Black,
  see [CLAUDE.md § Configuration](CLAUDE.md#configuration)
- formatting is similar Prettier and prettier-plugin-svelte for the common case,
  but intentionally diverges in some cases and fixes numerous bugs
  (see [docs/conformance_prettier.md](docs/conformance_prettier.md))
- tsv can generate a public JSON AST that should exactly match
  Svelte 5's modern AST with acorn and acorn-typescript
  (see [docs/conformance_svelte.md](docs/conformance_svelte.md)),
  and tsv has its own internal optimal AST
- the parser can also emit optimized JSON that drops the per-node `loc` and
  Svelte `name_loc` objects, mirroring acorn's `locations: false` for improved performance
  (`parse --no-locations` or the `parse_*_no_locations` bindings) 
- `tsv format` discovery is gitignore-aware,
  honoring `.gitignore` and `.formatignore` (original to tsv),
  hierarchically supporting nested files like git and unlike `.prettierignore`,
  plus a compatible `.prettierignore`
  (but relative to repo root if available, not cwd like Prettier's default)
  (also, all 3 files use [gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format))
- Rust-only implementation that currently does not call or embed a JS runtime
  (open for discussion, needs research into the tradeoffs);
  JS reaches tsv through the WASM bindings, and native N-API bindings will be published with v0.2
- ships optimal binary artifacts: runtime speed and compiled
  code size are priorities, so if all you need is a formatter, a minimal build is available,
  and heavier future layers (incremental parsing, CST for LSP) will be feature-gated so they
  don't regress the focused artifacts
- JS and TS parse in strict mode only - sloppy-mode-only syntax like `with` is
  rejected, while strict-mode early errors (e.g. duplicate params, reserved-word
  bindings) still parse for now, with enforcement deferred to a future
  diagnostics layer. The parse goal defaults to Module, with an opt-in Script
  goal (`--goal script`); since Svelte and TypeScript are inherently strict
  modules, this affects only standalone JS scripts to force modern patterns
- pushes complexity and mess to the printer and JSON conversion,
  out of the parser and internal AST,
  keeping the model clean for the other planned tools

Each language is a self-contained Rust crate exposing the same
`parse`/`format`/`convert_ast_json_bytes` functions over its own concrete types - there's no
central `Language` trait, registry, or dynamic dispatch ("closed scope, open convention").
That means the builds tree-shake at the link level:
the parse build excludes the printers, and the format build excludes the JSON-AST conversion layer.
Languages tree-shake the same way - a build binding only TypeScript would exclude
Svelte and CSS entirely - though there are no such minimal builds published yet.
Future LSP/incremental features will be later feature-gated layers that don't bloat
these artifacts - see [docs/architecture.md](docs/architecture.md)

tsv's goal is to be an optimal toolchain for TypeScript and Svelte.
Consumers can use tsv's crates ([not yet published](https://github.com/fuzdev/tsv/issues/140) to crates.io)
to build custom tools independently.
Hard non-goals:

- no markup for frameworks besides Svelte - no JSX/TSX, Vue, Astro, etc (unlike Biome+Oxc+SWC+friends) -
  but note that you can use tsv's crates and patterns to vibe your own thing
- standard CSS and Svelte extensions only - no SCSS, CSS Modules, LESS, etc
- no style config settings, so on-disk state and caller params
  never change the output for a given input
- no strict Prettier conformance -
  see the [conformance doc](https://github.com/fuzdev/tsv/blob/main/docs/conformance_prettier.md)
  and [discussion #1](https://github.com/fuzdev/tsv/discussions/1)

tsv currently does not support JS plugins or JS runtime integration.
JS bridging and WASM plugins will be evaluated to see if the tradeoffs work for tsv's goals,
but the current lean is against, mainly for performance and simplicity reasons.

tsv is derived from:

- Svelte
- TypeScript
- Prettier and prettier-plugin-svelte
- HTML/CSS/JS

tsv currently supports:

- [x] formatter matching Prettier + prettier-plugin-svelte (with intentional divergences)
- [x] parser, drop-in for Svelte+acorn+acorn-typescript
- [ ] [vscode formatter plugin](https://github.com/fuzdev/vscode_plugin_tsv_format) - fuzdev.tsv-format

Future features (unknown order):

- ts->js conversion (easy, probably soon)
- module lexer (easy, probably soon)
- minifier
- JSON support
- HTML support (assuming Svelte mostly works, but isn't correct e.g. with `{`)
- CSS error recovery (recover past invalid CSS per the spec instead of
  failing the parse - doesn't add dialect support)
- later:
  - TypeScript 7 integration (the Go impl), unlocking:
    - svelte-check replacement
    - LSP
    - linter - type aware, initially focused on serializable data-only plugins for extensibility
  - Svelte compiler (exact mirror, maybe out of scope, see [rsvelte](https://github.com/baseballyama/rsvelte))
  - bundling is probably out of scope 
  - discussion welcome

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
deno task check          # all checks (typecheck, test, lint, fmt)

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

- `parse(source) -> Result<AST>`
- `format(ast, source) -> String`
- `convert_ast_json_bytes(ast, source) -> Vec<u8>` — the wire JSON, emitted directly from the internal AST (default-on `convert` cargo feature; turn off for parse+format-only builds)

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
