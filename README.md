# tsv

> a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS - [tsv.fuz.dev](https://tsv.fuz.dev/)

tsv is a toolchain for Svelte, TypeScript, and CSS, written in Rust.
The first release has a near-[Prettier](https://prettier.io/) formatter,
similar to [prettier-plugin-svelte](https://github.com/sveltejs/prettier-plugin-svelte),
and a drop-in replacement for [Svelte](https://svelte.dev/)'s parser +
[acorn](https://github.com/acornjs/acorn) +
[acorn-typescript](https://github.com/sveltejs/acorn-typescript).

For benchmarks including performance and binary size, visit [tsv.fuz.dev](https://tsv.fuz.dev/).

This is an early release, and reports and feedback are appreciated -
see the [issues](https://github.com/fuzdev/tsv/issues)
and [discussions](https://github.com/fuzdev/tsv/discussions).

AI disclosure: this codebase was generated with machine agents.
The first release took 7 months and ~1800 manual commits.
Significant effort went into design and quality
(I think refactoring and quality were well over half of the token spend),
but the usual LLM caveats apply.

## Install

tsv ships three WASM packages to npm:

- [`@fuzdev/tsv_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_wasm) - the full tool (formatter + parser) with a `tsv` CLI
- [`@fuzdev/tsv_format_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_format_wasm) - formatter only (smaller)
- [`@fuzdev/tsv_parse_wasm`](https://www.npmjs.com/package/@fuzdev/tsv_parse_wasm) - parser + JSON AST only (smallest)

```bash
npm i @fuzdev/tsv_wasm            # or pick a subset: @fuzdev/tsv_format_wasm / @fuzdev/tsv_parse_wasm
npx tsv format src                # if installed locally
npx @fuzdev/tsv_wasm format src   # try the formatter without installing
```

```typescript
import {format_svelte} from '@fuzdev/tsv_format_wasm';
const formatted = format_svelte('<script>\nconst   x=1\n</script>');
```

```typescript
import {parse_svelte, type Root} from '@fuzdev/tsv_parse_wasm';
const ast: Root = parse_svelte('<script>const x = 1;</script>');
```

Both halves import the same way from `@fuzdev/tsv_wasm`.
Works without setup in Node.js/Bun/Deno (sync auto-init);
browsers and bundlers must call `await init()` once first.
See the package READMEs for the full API and CLI flags:

- [crates/tsv_wasm/README_all.md](crates/tsv_wasm/README_all.md)
- [crates/tsv_wasm/README_format.md](crates/tsv_wasm/README_format.md)
- [crates/tsv_wasm/README_parse.md](crates/tsv_wasm/README_parse.md)

There are no prebuilt native binaries yet - the npm packages are all WASM.
For native speed today, build the C FFI library from source
(`deno task build:ffi`, producing `target/release/libtsv_ffi.so`/`.dylib`/`.dll`) and bind it
from anything that speaks C FFI. Deno's FFI is used in the benchmarks.

## Design

- supports Svelte, TypeScript, CSS, JS, and HTML
- formatting tracks Prettier and prettier-plugin-svelte for the common case, but intentionally
  diverges in some cases - see [docs/conformance_prettier.md](docs/conformance_prettier.md)
- tsv can generate a public JSON AST that should exactly match
  Svelte 5's modern AST with acorn and acorn-typescript
  (see [docs/conformance_svelte.md](docs/conformance_svelte.md)),
  but keeps its own internal AST optimized for manipulation over serialization
- non-configurable: formatter settings are fixed at Prettier's defaults except
  `printWidth: 100`, `useTabs: true`, `singleQuote: true`, and
  `bracketSpacing: false` (tight object braces stay distinct from function/block `{`),
  and there are no config files or CLI options for formatting style;
  tsv is opinionated like `gofmt` and Python's Black,
  see [CLAUDE.md § Configuration](CLAUDE.md#configuration)
- `tsv format` discovery is gitignore-aware, honoring `.gitignore`, `.formatignore`,
  and a repo-root `.prettierignore`
  ([gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format))
- Rust-only implementation that never embeds or calls a JS runtime, for performance;
  JS reaches tsv through the WASM bindings, and native N-API bindings are
  undecided (open to requests)
- "optimal artifacts" is an invariant, not a preference: runtime speed and compiled
  code size are first-class goals for every shipped artifact, and heavier future
  layers (incremental parsing, CST for LSP) will be feature-gated so they
  don't regress the artifacts that exist today
- JS and TS always parse as modules in strict mode - sloppy-mode-only syntax
  (`with`, legacy octal literals, etc) is rejected; Svelte and TypeScript are
  inherently strict, so this only matters for standalone JS scripts
- pushes complexity and mess to the printer, out of the parser and AST,
  keeping the model clean for the other planned tools

Each language is a self-contained Rust crate exposing the same
`parse`/`format`/`convert_ast` functions over its own concrete types - there's no
central `Language` trait, registry, or enum dispatch ("closed scope, open convention").
That means no dynamic dispatch, and WASM builds tree-shake at the link level:
the parse build excludes the printers, and the format build excludes the JSON-AST conversion layer.
Languages tree-shake the same way - a build binding only TypeScript would exclude
Svelte and CSS entirely - though the published packages include all three.
Future LSP/incremental features will be later feature-gated layers that don't bloat
these artifacts - see [docs/architecture.md](docs/architecture.md)

tsv's goal is to be an optimal toolchain for Svelte and TypeScript,
and avoiding bloat is a key characteristic.
The scope trade is depth over breadth: a closed language set with an expanding
tool set, instead of more frameworks. Hard non-goals:

- other frameworks' markup - no Vue, Astro, JSX/TSX, etc (unlike Biome and friends);
  Svelte is the only component language (it's in the name `tsv`)
- CSS preprocessor and vendor dialects - no SCSS, LESS, CSS Modules, or IE hacks;
  tsv parses standard and Svelte CSS only
- JS plugins - follows from never embedding a JS runtime; linter extensibility,
  if any, will be WASM plugins and/or pattern-based rules
- no style config settings, so on-disk state and caller params
  never change the output for a given input

Deferred rather than refused:

- internal AST stabilization - not any time soon; the public JSON AST is the
  stable surface, tracking Svelte's
- N-API native bindings - npm is WASM-only for now
- full Prettier conformance? see [discussion 1](https://github.com/fuzdev/tsv/discussions/1)

tsv is derived from:

- Svelte
- TypeScript
- Prettier and prettier-plugin-svelte
- HTML/CSS/JS

tsv currently supports:

- [x] formatter matching Prettier + prettier-plugin-svelte (with intentional divergences)
- [x] parser, drop-in for Svelte+acorn+acorn-typescript

Future features (unknown order):

- CSS error recovery (recover past invalid CSS per the spec instead of
  failing the parse - doesn't add dialect support)
- linter (type aware, all Rust, maybe WASM plugins and/or pattern-based rules for extensibility)
- type stripper (easy, probably soon)
- module lexer (easy, probably soon)
- minifier
- LSP
- later
  - Svelte compiler (exact mirror)
  - bundler
  - typechecker isn't off the table
  - more? see the issues and discussions, suggestions welcome

## Docs

- **[CLAUDE.md](CLAUDE.md)** - development guide (commands, structure, conventions)
- **[docs/architecture.md](docs/architecture.md)** - the major design decisions
- **[docs/directives.md](docs/directives.md)** - `format-ignore` / `prettier-ignore` formatting directives
- **[docs/conformance_prettier.md](docs/conformance_prettier.md)** - where formatting diverges from Prettier (and why)
- **[docs/conformance_svelte.md](docs/conformance_svelte.md)** - where the parser diverges from Svelte (and why)
- **[docs/conformance_test262.md](docs/conformance_test262.md)** - ECMAScript parser conformance
- **[docs/fixture_overview.md](docs/fixture_overview.md)** - fixture system design
- **[docs/fixture_workflow.md](docs/fixture_workflow.md)** - step-by-step fixture creation
- **[docs/fixture_naming.md](docs/fixture_naming.md)** - fixture naming conventions and patterns

## Developing

Dev dependencies:

- [Rust](https://rust-lang.org/) - rustc, cargo
- [Deno](https://docs.deno.com/runtime/) - see ./deno.json for the tasks
  - currently uses `npm:` imports from `svelte`, `typescript`, `acorn`,
    `@sveltejs/acorn-typescript`, `prettier`, `prettier-plugin-svelte`

Rust dependencies are kept fairly minimal.
See [CLAUDE.md § Rust Crates](CLAUDE.md#rust-crates-minimal-deps) for the full list.

```bash
# Build workspace (recommended - uses deno tasks)
deno task build                    # dev build
deno task dev                      # watch mode (requires: cargo install cargo-watch)

# Or build directly with cargo
cargo build --workspace
cargo check --workspace            # fast syntax check (no codegen)

# Generate test fixtures (requires Deno for Svelte parser + prettier)
deno task fixtures:update:parsed

# Run tests and checks (requires Deno)
deno task check                     # all checks (typecheck, test, lint, fmt)
cargo test --workspace              # all tests including fixture validation

# Run CLI
cargo run -p tsv_cli parse --content "const x = 1;" --parser typescript
cargo run -p tsv_cli format --content "<div>test</div>" --parser svelte
```

## Project structure

Multi-crate workspace with clean separation of concerns:

```
tsv/
├── Cargo.toml           # workspace root
├── crates/
│   ├── tsv_lang/       # foundation (Span, Location, ParseError)
│   ├── tsv_html/       # HTML classification and whitespace rules
│   ├── tsv_ignore/     # gitignore-aware discovery matcher (.gitignore/.formatignore/.prettierignore)
│   ├── tsv_discover/   # file-discovery policy (build-output heuristic + safety nets) over tsv_ignore
│   ├── tsv_ts/         # TypeScript parser/formatter (standalone)
│   ├── tsv_css/        # CSS parser/formatter (standalone)
│   ├── tsv_svelte/     # Svelte parser/formatter (uses tsv_ts + tsv_css)
│   ├── tsv_cli/        # unified CLI (binary: `tsv`)
│   ├── tsv_debug/      # dev utilities (binary: `tsv_debug`, uses Deno)
│   ├── tsv_ffi/        # C FFI bindings
│   └── tsv_wasm/       # WebAssembly bindings
└── tests/              # workspace-level integration tests
```

Each language crate exports a consistent API:

- `parse(source) -> Result<AST>`
- `format(ast, source) -> String`
- `convert_ast(ast, source) -> PublicAST` (default-on `convert` cargo feature; turn off for parse+format-only builds)

See [CLAUDE.md](CLAUDE.md) for detailed structure and full command reference.

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

Claude Code was instrumental to this project, and tsv wouldn't exist without LLMs.
Source code of projects similar to tsv was not used by agents unless listed above.

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
