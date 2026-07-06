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

## Typed Parse Returns

The parse build wires `parse_*` WASM exports to interfaces in the bundled
`tsv_ast.d.ts` via `#[wasm_bindgen(typescript_type = "import('./tsv_ast').Program")]`
extern types plus a `typescript_custom_section` `export type * from "./tsv_ast"`
header. wasm-pack emits typed `parse_typescript(...): import('./tsv_ast').Program`
etc. with no post-build patcher.

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

## TypeScript Goal-Aware Exports

`parse_typescript_json_with_goal(source, goal)` and
`format_typescript_with_goal(source, goal)` mirror the `tsv_ts` arms of
`lang_bindings!` but take an explicit goal string (`"script"` / `"module"`,
validated by `goal_from_str`) and call `tsv_ts::parse_with_goal`. They are the
**one deliberate exception** to the uniform per-language binding shape: the parse
goal is TypeScript-only (Svelte `<script>` is always a module; CSS has no goal),
so they sit **outside** the macro rather than threading a meaningless `goal`
through svelte/css. The goalless `parse_typescript_json` / `format_typescript`
remain the `Module` default. Only these two TS variants exist — no typed-object
`parse_typescript_with_goal` (returns are plain `String`, so no `tsv_ast.d.ts`
wiring); add one only if a typed consumer needs it. `npm/cli.js` routes
`tsv parse|format --goal` through them (TS only); see [../../docs/cli.md §Input Handling](../../docs/cli.md).

## No-Locations Exports

The opt-in **span-only** parse wire — the same AST minus the per-node `loc`
(Svelte also minus `name_loc`) — is exposed as `parse_{typescript,svelte}_no_locations`
(object, materialized in Rust via `js_sys::JSON::parse`) and their
`parse_{typescript,svelte}_json_no_locations` string siblings, plus
`parse_typescript_json_with_goal_no_locations` (the goal-and-no-loc combination —
goal drives the parser, no-loc the writer, so they compose, mirroring `tsv_cli`'s
`--goal` + `--no-locations`; `npm/cli.js` routes both flags through it). Like the
goal-aware exports, these are **hand-written outside** `lang_bindings!` and **TS +
Svelte only** — CSS's `parseCss` emits no `loc`, so a CSS variant would duplicate
`parse_css`.
The object form returns an untyped `JsValue` (`any`), **not** a `tsv_ast.d.ts`
interface: those interfaces declare `loc` as required, and the shape here
deliberately omits it, so there is no typed-object export and no `.d.ts` wiring.
The object form exists (rather than string-only) so a benchmark of this path
materializes in Rust exactly as `parse_*` does, keeping the comparison
mechanism-matched. `loc` is derivable from `start`/`end` + source (see
[../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API), so this is a distinct
narrower product, not a second encoding of the drop-in contract. `npm/cli.js`
routes a `--no-locations` flag through these (and the goal combination above).

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
for Svelte** (doesn't recover `name_loc`, doesn't replicate the `<script>`
tag-position or destructure `+1`-column parser quirks, and adds `loc` to template
nodes Svelte's own wire omits); **a no-op for CSS**. It rides the **parse-capable**
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
before adding.

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
`push_tsv(anchor, content)` per discovered `.formatignore` (both shallowest-first;
`pop_gitignore()`/`pop_tsv()` to unwind a DFS — tsv layers are hierarchical),
then queries:

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

- `src/lib.rs` — WASM bindings (`lang_bindings!` macro + typed extern types) + the wasm32-gated talc `#[global_allocator]`
- `types/tsv_ast.d.ts` — Hand-maintained TS types, bundled into the parse-capable packages
- `npm/cli.js` — The `tsv` bin shipped in `@fuzdev/tsv_wasm` — mirrors `tsv_cli`'s contract (flags, exit codes, traversal); `node:util` `parseArgs`, zero deps
- `npm/locations.js` + `npm/locations.d.ts` — Pure-JS line/column reconstruction for the span-only `no-locations` wire; ships in the parse-capable packages, re-exported from index.js/browser.js by `patch_npm_package.ts` (see [No-Locations Reconstruction Helper](#linecolumn-reconstruction-helper-npmlocationsjs))
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
