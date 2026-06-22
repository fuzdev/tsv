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
`parse_*` builds it with the lang crate's `convert_ast_json_string` (the
typed direct-serialization path — no intermediate `serde_json::Value` for
TS) and calls the engine's native `JSON.parse` from Rust via
`js_sys::JSON::parse`, so the export signature stays the typed object.
Building the JS object graph node-by-node with `serde_wasm_bindgen` is
measurably slower.

`parse_*_json` exports return the JSON string itself — for consumers that
forward the wire format (disk, network, another tool) without paying
`JSON.parse` for an object they don't need.

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

`types/tsv_ast.d.ts` is **hand-maintained**. Any change to `pub` fields,
enums, or `#[serde(rename/skip/...)]` attributes in
`crates/tsv_*/src/ast/public*` must also update the `.d.ts`. Reviewers
(human or agent) flag drift at PR time.

Maintenance checklist when modifying a public AST struct or enum:

1. Update the Rust struct/enum.
2. Locate the matching `interface` / `type` in `types/tsv_ast.d.ts`.
3. Apply the same change. Field names follow `#[serde(rename = "...")]`
   when present, otherwise the Rust field name verbatim.
4. Check `#[serde(skip_serializing*)]` rules — fields with
   `skip_serializing_if = "Option::is_none"` become `T?`;
   `skip_serializing` fields are omitted from the TS interface.
5. **tsv_ts only**: if the field carries positions (`start`/`end`/`loc`/
   `character`) or contains nodes that do, add it to the typed
   offset-translation walk (`tsv_ts/src/ast/convert/translate_typed.rs`) —
   a missed field means silently untranslated offsets on multibyte sources.
6. Run `cargo test --workspace` and `deno task check:ast-types`.

`deno task check:ast-types` (also part of `deno task check`) invokes
`tsv parse` on a curated set of source snippets, embeds each JSON
output as a typed literal against `tsv_ast.d.ts`, and runs `deno check`.
TypeScript's excess-property checking catches both directions of drift:
missing/added fields and discriminator-string mismatches. Extend
`scripts/check_ast_types.ts`'s `samples` array when a previously
uncovered AST node regresses.

`Schema::Acorn` vs `Schema::SvelteScript` deltas in the convert layer
require dual updates.

## Files

| File                 | Purpose                                                                                                                                       |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/lib.rs`         | WASM bindings (`lang_bindings!` macro + typed extern types)                                                                                   |
| `types/tsv_ast.d.ts` | Hand-maintained TS types, bundled into the parse-capable packages                                                                             |
| `npm/cli.js`         | The `tsv` bin shipped in `@fuzdev/tsv_wasm` — mirrors `tsv_cli`'s contract (flags, exit codes, traversal); `node:util` `parseArgs`, zero deps |
| `README_format.md`   | Shipped as `README.md` in `@fuzdev/tsv_format_wasm` (copied by `patch_npm_package.ts`)                                                        |
| `README_parse.md`    | Shipped as `README.md` in `@fuzdev/tsv_parse_wasm` (copied by `patch_npm_package.ts`)                                                         |
| `README_all.md`      | Shipped as `README.md` in `@fuzdev/tsv_wasm` (copied by `patch_npm_package.ts`)                                                               |
| `pkg/`               | Build output (gitignored), `pkg/<variant>/<target>/`                                                                                          |

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
