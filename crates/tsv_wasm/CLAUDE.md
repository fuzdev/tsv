# tsv_wasm

WebAssembly bindings for `tsv`. Three npm packages from one Rust crate via
the `format` + `parse` cargo features (default = both):
`--no-default-features --features format` → `@fuzdev/tsv_format_wasm`
(format only); `--no-default-features --features parse` →
`@fuzdev/tsv_parse_wasm` (parse only — the printers drop out at link time;
bundles `tsv_ast.d.ts` for typed returns); default build →
`@fuzdev/tsv_wasm` (everything, plus the `tsv` CLI from `npm/cli.js`).

See [../../CLAUDE.md §Publishing](../../CLAUDE.md#publishing) for the
package shape, version-of-truth rule, and the `deno task publish` /
`build:npm:*` commands. A separate types-only `@fuzdev/tsv_ast` package
is deferred.

## Parse Options & Typed Returns

Every parse export shares one uniform signature — `(source, options?)` with an
acorn-style `{locations?, goal?}` bag, parsed in Rust (`parse_parse_options` in
`src/lib.rs`, via `js_sys` `Object.keys` + `Reflect::get` — `js-sys` already
rides the `parse` feature, so nothing lands on the format-only package).
`locations` (default `true`) selects the wire: the loc-bearing drop-in
contract, or the span-only variant (see below); it is accepted everywhere and
inert where nothing reads it (CSS emits no `loc`; `parse_internal_*` emits no
wire). `goal` (`'script'` / `'module'`, default `'module'`) is TypeScript-only
— Svelte hard-wires `Module`, CSS has no goal — so the other languages reject
the key. Unknown keys always error (a typo like `{locatons: false}` silently
succeeding would hand back the full wire while the caller believes they opted
out); an explicitly-`undefined` value means that key's default — including the
TS-only `goal` on a language that rejects it, which is what lets a caller
forward one bag to whichever parser (`npm/cli.js` does). A non-object argument
errors, arrays included.

The parse exports are all `#[wasm_bindgen(skip_typescript)]`; their `.d.ts` is
the hand-written `TS_PARSE_DECLS` `typescript_custom_section` in `src/lib.rs`.
wasm-bindgen can't express an options-dependent return type, and
`{locations: false}` returns a shape `tsv_ast.d.ts` can't name (its interfaces
declare `loc` required), so that overload returns `any` and comes first (the
more specific signature). The section also declares `ParseOptions` /
`TypeScriptParseOptions`, which `scripts/patch_npm_package.ts` re-exports by
name through the npm facade. A signature change in `lang_bindings!` must update
`TS_PARSE_DECLS` in the same edit. The `export type * from "./tsv_ast"` header
rides its own custom section, so consumers `import type` AST nodes directly.

## Panic Reporting

A `#[wasm_bindgen(start)]` hook forwards panic messages to `console.error`
(**measured 468 B**), because under the shipped `panic = "abort"` + `strip` a
panic reaches the host as a bare `RuntimeError: unreachable`. The why — and why
the `console.error` binding is hand-rolled rather than a dep — is in the comment
above the hook in `src/lib.rs`.

Diagnostic only: the call still traps. What makes the **instance survive** it is
[`tsv_arena`](../tsv_arena/CLAUDE.md#abort-safety-take-and-park)'s take/park.

## JSON-String Transport

The AST crosses the JS↔WASM boundary as **one compact JSON string**:
`parse_*` builds it with the lang crate's `convert_ast_json_string` (each
language's wire-JSON writer emits it directly from the internal AST — no
intermediate `serde_json::Value` or typed public tree) and calls the
engine's native `JSON.parse` from Rust via
`js_sys::JSON::parse`, so the export signature stays the typed object.
Building the JS object graph node-by-node with `serde_wasm_bindgen` is
measurably slower.

`parse_*_json` exports return the JSON string itself — for consumers that
forward the wire format (disk, network, another tool) without paying
`JSON.parse` for an object they don't need.

## TypeScript Goal-Aware Format Export

The parse goal rides the parse exports' `goal` option (see Parse Options
above). **Format** instead keeps one flat export,
`format_typescript_with_goal(source, goal)` (goal string validated by
`goal_from_str`): parsing an options bag in Rust needs `js_sys`, which
deliberately rides only the `parse` feature — an options object here would
newly weigh down the format-only package. TypeScript-only, like the option
(Svelte `<script>` is always a module; CSS has no goal). `npm/cli.js` routes
`tsv format --goal` through it and `tsv parse --goal` through the `goal`
option; see [../../docs/cli.md §Input Handling](../../docs/cli.md).

## The Span-Only Wire (`locations: false`)

The opt-in **span-only** parse wire — the same AST minus the per-node `loc`
(Svelte also minus `name_loc`) — is the `{locations: false}` option on every
parse export, uniform in `lang_bindings!` (for CSS it's accepted and inert —
`parseCss` emits no `loc`). Goal and locations compose (goal drives the
parser, locations the writer), mirroring `tsv_cli`'s `--goal` +
`--no-locations`, which `npm/cli.js` routes through the same options. A
`{locations: false}` object call materializes in Rust via `js_sys::JSON::parse`
exactly as the loc-bearing call does, keeping benchmarks of the two
mechanism-matched; its return is `any` in the `.d.ts` (the shape omits the
`loc` that `tsv_ast.d.ts` requires). `loc` is derivable from `start`/`end` +
source (see [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API), so this is
a distinct narrower product, not a second encoding of the drop-in contract.

### Line/Column Reconstruction Helper (`npm/locations.js`)

Because `loc` is a pure function of `start`/`end` + source, a consumer holding
only the span-only wire recovers it in JS — and, for a consumer that needs full
`loc`, no-loc-wire + JS-reconstruct beats the full loc-bearing wire end-to-end
(the full wire's `loc` bytes cost real `JSON.parse` tokenization; a line-start
table + binary search is cheaper). `npm/locations.js` (pure JS, zero deps, no
WASM) is that reconstruction, shipped so callers don't reimplement the line
rules: `reconstruct_locations(ast, source, opts?)` (one-shot, adds `loc` to every
node, **mutates in place**), `create_locator(source, opts?)` (amortized — holds
the prebuilt line table, exposes `loc_of(node)` / `reconstruct(ast)`), and a bare
`loc_of(node, source, opts?)` convenience. **Exact for TypeScript**; **approximate
for Svelte** (doesn't replicate the `<script>` tag-position or destructure
`+1`-column parser quirks, and adds `loc` to template nodes Svelte's own wire
omits — but `name_loc` is restored exactly, its span derived from each node's own
`start`/`end` + type, as is the name-shaped `loc` on shorthand-attribute
identifiers, snippet names, and simple-identifier block patterns, and the
`character` field on an in-tag comment, recovered structurally: a comment sitting
between an element's attributes, i.e. inside its opening tag at brace depth 0 —
including the `<svelte:options>` head, whose wire node carries no `type` and is
pushed into the host-element pass explicitly);
**a no-op for CSS**. It rides the **parse-capable**
packages only (`@fuzdev/tsv_parse_wasm`, `@fuzdev/tsv_wasm`) — it operates on the
parse wire, so the format-only package has no use for it. `patch_npm_package.ts`
copies it + the hand-written `npm/locations.d.ts` into the package root and
re-exports the functions from index.js/browser.js/index.d.ts (directly, with no
init guard — it never touches WASM). Its correctness is gated by the package Node
tests (`scripts/test_npm.ts`) and, at corpus scale, by
`benches/js/diagnostics/no_locations_parity.ts`.

**`.d.ts` export-name constraint.** `index.d.ts` re-exports both `tsv_ast.d.ts`
(`export type *`) and `locations.d.ts` (`export *`), so a name exported by BOTH is
ambiguated away (TS2308) — silently dropping that name from the package. `tsv_ast`
owns `Position` / `SourceLocation` / `NameLocation` / `NamePosition` (+ every AST
node type), so `locations.d.ts` must not export any of those — its `Loc` inlines
the `{line, column}` point rather than naming a `Position`. Any future hand-written
`.d.ts` added to the parse packages faces the same rule; nothing in-repo type-checks
the merged package `.d.ts` (`check:ast-types` covers `tsv_ast.d.ts` alone), so a
collision only surfaces at a consumer's compile — check names against `tsv_ast`
before adding. (`ParseOptions` / `TypeScriptParseOptions` are re-exported **by
name** from the generated `tsv_wasm.d.ts`, which explicit form star-export
ambiguation can't drop — but the names were checked against both files anyway.)

## Discovery Matcher + Policy (`IgnoreStack`)

The `format` feature exports an `IgnoreStack` class wrapping
`tsv_ignore::IgnoreStack` — tsv's hierarchical, git-faithful matcher — plus the
`tsv_discover` discovery *policy* layered on it (the build-output heuristic +
safety-net pruning). It rides the format-capable packages
(`@fuzdev/tsv_format_wasm`, `@fuzdev/tsv_wasm`) and is absent from the parse-only
package; `tsv_ignore` **and** `tsv_discover` are **optional** deps pulled in by
`format`. This gives the JS CLI (`npm/cli.js`) and the VS Code extension the exact
same matcher *and* prune decision as the native CLI, so all three agree by
construction. The caller builds it up: `new IgnoreStack()`, then
`push_gitignore(anchor, content)` per discovered `.gitignore` and
`push_tsv(anchor, content)` per discovered `.formatignore` (and, inside a repo,
`.prettierignore` — each shadowed by a sibling `.formatignore`) (both
shallowest-first; `pop_gitignore()`/`pop_tsv()` to unwind a DFS — tsv layers are
hierarchical), then queries:

- `classify_dir(name, child_rel, heuristic_active) -> 'descend' | 'prune' |
  'prune_warn'` — the shared per-directory verdict (`tsv_discover::classify_dir`:
  safety nets, the build-output heuristic, the matcher). On `'prune_warn'` fetch
  the message via `heuristic_shadow_warning(dir)`.
- `should_format_file(name, child_rel) -> bool` — the per-file verdict (a
  formattable extension and not ignored).
- `is_path_pruned(rel) -> bool` — the per-file form of the directory-prune verdict
  for a consumer with **no top-down traversal** (the VS Code extension formats one
  open document at a time). It walks `rel`'s ancestor directories itself and
  reconstructs each level's `heuristic_active` from the stack's own pushed
  `.gitignore` anchors, so it takes no extra arguments; pair it with
  `is_ignored(rel, false)` for the file-level match. (`classify_dir` stays the
  primitive for `npm/cli.js`, which threads `heuristic_active` down a real walk.)
- `heuristic_shadow_warning(dir) -> string` — the one warning template (a method,
  not a free function, so it rides the class re-export; single source of truth
  with the native CLI, never re-templated in JS).
- `is_ignored(path, is_dir)` / `is_empty()` — the raw matcher primitives, still
  exposed for direct consumers.

The string-tag return for `classify_dir` (rather than a wasm-bindgen enum or a
returned struct) needs no `patch_npm_package.ts` change and allocates no JS object
on the common descend path. The earlier `is_reincluded` / `has_negation_under`
primitives are now folded inside `classify_dir`, so they're no longer exported
across the WASM boundary (they stay public on the Rust `tsv_ignore::IgnoreStack`) —
JS callers consume the verdict instead of re-deriving the prune decision.

Unlike the parse exports, the class is emitted as `export class` (not
`export function`); `scripts/patch_npm_package.ts` detects `export class` and
re-exports it through the package facade alongside the functions, and
`scripts/validate_artifacts.ts` smoke-tests it (present in format/all, absent in
parse-only). The wasm-bindgen-generated `tsv_wasm.d.ts` declares the class, so no
`tsv_ast.d.ts` entry is needed.

## TS Type Maintenance

`types/tsv_ast.d.ts` is **hand-maintained**. Any change to the wire JSON a
writer emits — a field, its key name, when it's omitted, or a discriminator
`type` string — in `crates/tsv_*/src/ast/convert/write*` must also update the
`.d.ts`. Reviewers (human or agent) flag drift at PR time.

Maintenance checklist when a writer's emitted shape changes:

1. Update the `write_*` function (the emitted field / key / skip condition).
2. Locate the matching `interface` / `type` in `types/tsv_ast.d.ts`.
3. Apply the same change. The JSON key is exactly what the writer emits
   (e.g. `w.raw(",\"typeParameters\":")` → `typeParameters`).
4. A field the writer emits only conditionally (`if let Some(..)` / `if flag`)
   is optional in TS (`T?`); one it never emits is absent from the interface.
5. If the field carries positions (`start`/`end`/`loc`/`character`), make sure
   the writer (`ast/convert/write*`) emits them through the `LocationMapper`
   (`ctx.pos(...)` / the `loc` helpers) — a raw byte offset means silently
   untranslated positions on multibyte sources.
6. Run `cargo test --workspace` and `deno task check:ast-types`.

`deno task check:ast-types` (also part of `deno task check`) invokes
`tsv parse` on a curated set of source snippets, embeds each JSON
output as a typed literal against `tsv_ast.d.ts`, and runs `deno check`.
TypeScript's excess-property checking catches both directions of drift:
missing/added fields and discriminator-string mismatches. Extend
`scripts/check_ast_types.ts`'s `samples` array when a previously
uncovered AST node regresses.

`Schema::Acorn` vs `Schema::SvelteScript` deltas the writer emits
require dual updates.

## Files

- `src/lib.rs` — WASM bindings (`lang_bindings!` macro + `parse_parse_options` + the hand-written `TS_PARSE_DECLS` declarations) + the wasm32-gated talc `#[global_allocator]` and panic hook
- `types/tsv_ast.d.ts` — Hand-maintained TS types, bundled into the parse-capable packages
- `npm/cli.js` — The `tsv` bin shipped in `@fuzdev/tsv_wasm` — mirrors `tsv_cli`'s contract (flags, exit codes, traversal); `node:util` `parseArgs`, zero deps
- `npm/locations.js` + `npm/locations.d.ts` — Pure-JS line/column reconstruction for the span-only `no-locations` wire; ships in the parse-capable packages, re-exported from index.js/browser.js by `patch_npm_package.ts` (see [Line/Column Reconstruction Helper](#linecolumn-reconstruction-helper-npmlocationsjs))
- `README_format.md` — Shipped as `README.md` in `@fuzdev/tsv_format_wasm` (copied by `patch_npm_package.ts`)
- `README_parse.md` — Shipped as `README.md` in `@fuzdev/tsv_parse_wasm` (copied by `patch_npm_package.ts`)
- `README_all.md` — Shipped as `README.md` in `@fuzdev/tsv_wasm` (copied by `patch_npm_package.ts`)
- `pkg/` — Build output (gitignored), `pkg/<variant>/<target>/`

## Build Targets

Variant-first output dirs (`pkg/<variant>/<target>/`) so builds never clobber
each other. Subsets build `--no-default-features --features format|parse`;
the `all` builds use the default features (both).

| Target | format output dir  | parse output dir  | all output dir  | Command (format / parse / all)                                      |
| ------ | ------------------ | ----------------- | --------------- | ------------------------------------------------------------------- |
| deno   | `pkg/format/deno/` | `pkg/parse/deno/` | `pkg/all/deno/` | `build:wasm:deno` / `build:wasm:parse:deno` / `build:wasm:all:deno` |
| npm    | `pkg/format/npm/`  | `pkg/parse/npm/`  | `pkg/all/npm/`  | `build:npm:format` / `build:npm:parse` / `build:npm:all`            |

The `pkg/all/deno` build feeds the benches and sidecar (it has every
export); the subset deno builds are size-tracked by `binary_sizes.ts`. The
`npm` builds are the published artifacts: a wasm-pack `web`-target build
patched by `scripts/patch_npm_package.ts` into the multi-entry package shape
(Node auto-init entry, guarded browser entry, conditional `exports`,
metadata, README — plus `cli.js` and the `tsv` bin for the `all` variant).
`deno task test:npm[:parse|:all]` builds the package and then runs Node tests
against it (the `all` variant adds CLI subprocess tests; the `:run` suffix —
e.g. `test:npm:run` — skips the rebuild when the bundle is already fresh, as in
the publish/CI pipelines), and `deno task validate:artifacts`
checks tight wasm size bounds plus a Deno runtime smoke of every built
bundle (both run in the publish pipeline). The npm package itself covers
Node/browser/bundler consumers, so there are no standalone `web`/`nodejs`
target builds.

The generated `tsv_wasm_bg.wasm.d.ts` is intentionally excluded from the
npm `files` list: it types direct `.wasm` ES-module imports, which the
package shape never uses (bytes via `readFileSync`, URL via `init()`), and
nothing in `tsv_wasm.d.ts` references it — matching blake3's packages.
