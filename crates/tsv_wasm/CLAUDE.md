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
acorn-style `{locations?, sourceType?}` bag, read in Rust (`read_options` in
`src/lib.rs`, via `js_sys` `Object.keys` + `Reflect::get`; the same reader
serves the format exports, so the two families can't drift — see
[Format Options](#format-options)). ⚠️ A **third copy** of these semantics
lives in [../tsv_napi/npm/index.js](../tsv_napi/npm/index.js) — the ESM
`@fuzdev/tsv` loader mirrors this reader key for key, error string for
error string, as its parity contract (asserted by `scripts/test_napi_npm.ts`'s
exact-string tests). A semantics or message change here must update that
mirror and its tests in the same edit.
`locations` (default `true`) selects the wire: the loc-bearing drop-in
contract, or the span-only variant (see below); it is accepted everywhere and
inert where nothing reads it (CSS emits no `loc`; `parse_internal_*` emits no
wire). `sourceType` (`'script'` / `'module'`) is TypeScript-only
— Svelte hard-wires `Module`, CSS has no goal — so the other languages reject
the key. Unset, it means `'module'` **for a parse** (the wire's `Program.sourceType`
is a claim one settled grammar has to produce); the format exports read the same
absence differently — see [Format Options](#format-options). Unknown keys always error, whatever their value (a typo like
`{locatons: false}` — or `{locatons: undefined}` — silently succeeding would
hand back the full wire while the caller believes they opted out); a supported
key explicitly set to `undefined` means that key's default — including the
TS-only `sourceType` on a language that rejects it, which is what lets a caller
forward one bag to whichever parser (`npm/cli.js` does). A non-object argument
errors, arrays included.

The parse exports are all `#[wasm_bindgen(skip_typescript)]`; their `.d.ts` is
the hand-written `TS_PARSE_DECLS` `typescript_custom_section` in `src/lib.rs`.
wasm-bindgen can't express an options-dependent return type, and
`{locations: false}` returns a shape `tsv_ast.d.ts` can't name (its interfaces
declare `loc` required), so that overload returns `any` and comes first (the
more specific signature). The section also declares `ParseOptions` /
`TypeScriptParseOptions`, which `scripts/patch_npm_package.ts` re-exports by
name through the npm facade; their exact shape — and why neither is empty and
neither `extends` the other — is
[The Four Option Interfaces](#the-four-option-interfaces). A signature change in
`lang_bindings!` must update `TS_PARSE_DECLS` in the same edit. The `export type * from "./tsv_ast"` header
rides its own custom section, so consumers `import type` AST nodes directly.

## Panic Reporting

A `#[wasm_bindgen(start)]` hook forwards panic messages to `console.error`
(**measured at under +0.1% raw and gzipped** on `@fuzdev/tsv_format_wasm`, ~1.3 KB
raw), because under the shipped `panic = "abort"` + `strip` a
panic reaches the host as a bare `RuntimeError: unreachable`. The why — and why
the `console.error` binding is hand-rolled rather than a dep — is in the comment
above the hook in `src/lib.rs`.

Diagnostic only: the call still traps. What makes the **instance survive** it is
[`tsv_arena`](../tsv_arena/CLAUDE.md#abort-safety-take-and-park)'s take/park.

⚠️ **A stack overflow is the one trap the instance does NOT survive** — a different
trap with a different cause, and take/park has no reach over it. The shadow stack is
1 MiB of linear memory (wasm-ld's default `-z stack-size`, placed first so an overflow
walks off address 0 into `memory access out of bounds` rather than quietly over the
data segments), and `__stack_pointer` is a plain mutable global that a trap does not
restore. So the pointer stays where the deep call left it and **every later call on
that instance throws the same error, in every language and on every entry point** —
verified across `format_typescript` / `format_css` / `format_svelte` /
`parse_typescript_json` after one deep `format_typescript`. Only a fresh instance
recovers. The depth is ~2,510 nested parens at ~0.41 KiB of shadow stack per level, the
lowest of any tsv surface — though above acorn's 497 and prettier's 805 — and unlike the
native builds it does not move with the host, since the shadow stack is inside the
module.

**A fresh instance is what the npm packages' `reinstantiate()` export provides.**
wasm-bindgen's `initSync` short-circuits once initialized, so the hook is patched
into the generated glue by `scripts/patch_npm_package.ts` — the only module that can
reach the glue's module-level `wasm` binding. It swaps in a fresh instance
**synchronously** from the retained compiled `WebAssembly.Module` (no recompile),
and because every glue export reads `wasm` at call time, every already-imported
binding follows the swap with no rebind. It rides all three variants and every
entry (the lazy entries re-export it; it self-guards until initialized). `npm/cli.js`
calls it in `format_one` on any `WebAssembly.RuntimeError`, and on the `RangeError` V8
raises when a deep call exhausts its native stack first (which strands the instance too)
— both roles, the
sequential path and each pool worker — so one outsized file costs one honest
per-file error instead of poisoning the rest of the run (before the hook, the
sequential path reported every later file as `memory access out of bounds`, and the
parallel path was only saved by whichever files the healthy workers drained first).
Consumers that loop over files on one instance should do the same. Wasm-backed
objects from before the swap (`IgnoreStack`) are invalidated — rebuild them after.
The patcher stamps every handle with the instance generation it was minted under
(on the object for `free()`, in the `FinalizationRegistry` held value for the GC
callback); both free paths no-op on a stale stamp, so a handle from a discarded
instance leaks its old bytes instead of freeing a stale pointer into the fresh
instance's allocator, and every exported **method** routes its receiver through the
same compare (`__tsv_live`) and throws on a stale one, since its pointer would
otherwise read the fresh memory's unrelated bytes as the object — while handles
minted after the swap free and work normally. (A
module-level "has any reinstantiation happened" check is the wrong shape: a one-way
fuse that leaks every handle after the first recovery, fresh ones included.) Gated by
`scripts/test_npm.ts` (the poison-then-recover API contract + both CLI paths) and
smoked per variant by `scripts/validate_artifacts.ts`.

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

## Format Options

The format exports take the **same bag** — `format_<lang>(source, options?)`,
read by the same `read_options` — so one package never teaches two calling
conventions: a caller holding a `{sourceType}` bag hands it to a parser or a
formatter without branching (`npm/cli.js` does exactly that on both paths).
Format's bag carries **one** key, the TypeScript-only `sourceType` (Svelte
`<script>` is always a module; CSS has no goal), because formatting itself is
non-configurable. Everything else is the parse semantics verbatim: unknown keys error
whatever their value, a supported key set to `undefined` means its default
(including the TS-only `sourceType` on a language that rejects it), and a non-object
argument errors, arrays included.

**An unset `sourceType` is the one key whose default differs between the two
families.** `read_options` decodes it to `None` rather than `Module`, and the format
exports hand that to `parse_ast_for_format!` → `tsv_ts::parse_with_goal_or_fallback`:
the module grammar, retried as a script only if that parse *fails*, reporting the
module's error when both do if the script retry died on a top-level `import`/`export`, an `import.meta`, a top-level `for await` or a top-level `await`'s operand, else the further-reaching one (the module's on a tie). That is what lets `format_typescript(source)` with no bag
— an editor's whole call, and `npm/cli.js`'s path mode — format a legacy sloppy
script (`with`, a leading-zero literal or escape, `await` as a name). A **set** value
is exact, so `{sourceType: 'module'}` still refuses one; nothing the module grammar
accepts is ever reinterpreted, and the printer reads no goal, so no output moves. The
parse exports unwrap the same `None` to `Module` (see
[../../docs/cli.md §Multi-File Formatting](../../docs/cli.md#multi-file-formatting)).

**`locations` is rejected here, not accepted-and-inert.** It selects a *wire*,
and format emits none — an inert spelling would let a caller believe they had
asked a formatter for the narrower product. The forwarding argument that makes
`sourceType` lenient doesn't reach it: nothing hands a *parse* bag to a format
export (`npm/cli.js` builds each bag at its own call site), so the key is
simply unknown. On `format_svelte`/`format_css`, where no key is settable, the
unknown-key error says so — `unknown format option 'locations' (this export
takes no options)`. `sourceType` is the exception on those two: it matches its own
arm first and reports `format option 'sourceType' is only supported for TypeScript`,
which is the more useful message and the reason the key stays leniently
`undefined`-tolerant there.

Reading a bag needs `js_sys`, so `js-sys` rides the `format` feature too —
**measured at under +0.2% raw and gzipped** on `@fuzdev/tsv_format_wasm`, two
orders of magnitude inside `scripts/validate_artifacts.ts`'s bounds. The dep is
worth stating a bound on when it moves, not worth avoiding: a hand-rolled
getter or `inline_js` validator would undercut it only by paying with a second
options reader, and the two families' semantics have to be identical key for
key.

Like the parse exports, the format exports are `#[wasm_bindgen(skip_typescript)]`
with hand-written declarations — the `TS_FORMAT_DECLS` custom section in
`src/lib.rs`. That block is **required**, not a nicety: a `JsValue` parameter
generates as a *required* `options: any`, which would untype the bag and break
every existing arity-1 `format_<lang>(source)` call (the VS Code extension's
whole usage) at compile time. It declares `FormatOptions` /
`TypeScriptFormatOptions`, which `scripts/patch_npm_package.ts` re-exports by
name through the npm facade (checked against `tsv_ast.d.ts` and
`locations.d.ts` for the TS2308 collision rule below).

## The Four Option Interfaces

The non-TypeScript bags — `FormatOptions` (`format_svelte`/`format_css`) and
`ParseOptions` (`parse_svelte`/`parse_css`) — both declare **`sourceType?: undefined`**.
Neither may be `{}`, and neither may omit the key. Two independent reasons —
the first bites `FormatOptions` alone, the second both:

- **An empty interface guards nothing.** `{}` opts out of *both*
  excess-property checking and weak-type detection, so it accepts every
  non-nullish value: `format_svelte(src, {locatons: false})`,
  `format_svelte(src, 'script')`, `format_css(src, ['script'])` would all
  compile and all throw. That is the one shape where the types are *looser*
  than the runtime rather than stricter. One declared key restores both checks.
  (`ParseOptions` was never empty — it has `locations`.)
- **Omitting the key breaks forwarding.** `npm/cli.js` builds
  `{sourceType: <maybe undefined>}` and hands it to whichever export rather than
  branching the call, and the runtime reads a `sourceType` set to `undefined` as
  its default even on a language that rejects a *set* value. A bag with a
  `sourceType: undefined` key must therefore type-check on those exports —
  which an omitted key rejects (excess property) and `sourceType?: never` rejects
  under `exactOptionalPropertyTypes`. `undefined` is the spelling that works.

The TypeScript bags — `TypeScriptFormatOptions` / `TypeScriptParseOptions` —
are standalone interfaces, **not** `extends` of their base: a settable `sourceType`
is incompatible with the undefined-only one. The assignability that `extends`
would buy (handing a `TypeScriptParseOptions`-typed variable to `parse_svelte`)
is unsound anyway, since it throws the moment `sourceType` is actually set; the
only bag that forwards at runtime is one whose `sourceType` is `undefined`, which is
exactly what these shapes accept. The cost is one duplicated `locations?` line
in `TypeScriptParseOptions`.

**Every key spells `| undefined` on top of `?`** — `locations?: boolean |
undefined`, `sourceType?: 'script' | 'module' | undefined`. Same reason as above,
applied to the settable keys: without it,
`format_typescript(src, {sourceType: undefined})` — precisely what `npm/cli.js`
builds when no `--source-type` was passed — fails to type-check under
`exactOptionalPropertyTypes` while working at runtime.

The residual looseness is the usual TypeScript one: excess-property checking
fires on fresh object literals, so a **non-literal** bag with an unknown key
still forwards past the compiler and lands on the runtime's unknown-key error.
That is the intended division of labor, not a gap.

`npm/cli.js` routes `tsv format --source-type` and `tsv parse --source-type` through the same
option; see [../../docs/cli.md §Input Handling](../../docs/cli.md).

## The Span-Only Wire (`locations: false`)

The opt-in **span-only** parse wire — the same AST minus the per-node `loc`
(Svelte also minus `name_loc`) — is the `{locations: false}` option on every
parse export, uniform in `lang_bindings!` (for CSS it's accepted and inert —
`parseCss` emits no `loc`). Goal and locations compose (goal drives the
parser, locations the writer), mirroring `tsv_cli`'s `--source-type` +
`--no-locations`, which `npm/cli.js` routes through the same options. A
`{locations: false}` object call materializes in Rust via `js_sys::JSON::parse`
exactly as the loc-bearing call does, keeping benchmarks of the two
mechanism-matched; its return is `any` in the `.d.ts` (the shape omits the
`loc` that `tsv_ast.d.ts` requires). `loc` is derivable from `start`/`end` +
source (see [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API), so this is
a distinct narrower product, not a second encoding of the drop-in contract.

### Line/Column Reconstruction Helper (`npm/locations.js`)

Because `loc` is a pure function of `start`/`end` + source — with one exception,
below — a consumer holding only the span-only wire recovers it in JS — and, for a consumer that needs full
`loc`, no-loc-wire + JS-reconstruct beats the full loc-bearing wire end-to-end
(the full wire's `loc` bytes cost real `JSON.parse` tokenization; a line-start
table + binary search is cheaper). `npm/locations.js` (pure JS, zero deps, no
WASM) is that reconstruction, shipped so callers don't reimplement the line
rules — in every package that parses, native `@fuzdev/tsv` included: `reconstruct_locations(ast, source, opts?)` (one-shot, adds `loc` to every
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
**a no-op for CSS**. The exception is not an approximation but a **refusal**: a Svelte
source holding a lone CR, U+2028 or U+2029 carries two line counts — acorn's on the nodes
it parsed, `locate-character`'s on the rest — and which one a node takes is not a function
of its offsets, so every entry point throws rather than returning quietly-wrong lines
(parse those with `loc`; see [docs/architecture.md §`loc` lines](../../docs/architecture.md#loc-lines-two-classes-one-per-acorn-parse)). The `reconstruct` forms carry a second refusal on the same principle — a block binding whose `: T` sits behind a newline, whose annotation acorn reads under a seed the offsets cannot supply — checked against the tree rather than the source, since only a parse says where a block binding is.
It rides every package that parses —
`@fuzdev/tsv_parse_wasm`, `@fuzdev/tsv_wasm`, and the native `@fuzdev/tsv` loader
(`build_napi_packages.ts` stages it there) — it operates on the
parse wire, so only the format-only package has no use for it. `patch_npm_package.ts`
copies it + the hand-written `npm/locations.d.ts` into the package root and
re-exports the functions from index.js/browser.js/index.d.ts (directly, with no
init guard — it never touches WASM). Its correctness is gated by the package Node
tests (`scripts/test_npm.ts`); at corpus scale,
`benches/js/diagnostics/no_locations_parity.ts` proves the reconstruction *rules*
(deliberately re-derived, so a transcription slip in the shipped helper shows) while
`benches/js/diagnostics/reconstruct_vs_materialize.ts` is the diagnostic that runs
the shipped helper itself.

⚠️ **The re-derivation is an independent IMPLEMENTATION, not an independent oracle.** Both
sides of that comparison come from tsv — the loc-bearing wire, and a JS transcription of
`tsv_lang::LocationTracker`'s rules — so what it grades is the transcription, never the model
both sides share. A `loc` model that is wrong makes both halves wrong the same way and the
check stays green; the same mirror then reports red against a *corrected* model, which reads
as a regression and is not one. The canonical parser's own `loc` is available in the same
harness, and is the reference that would make this an oracle.

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

That blind spot has two classes, and the second is gated. A relative
specifier inside a shipped `.d.ts` must carry the **`.js`** extension
(`'./tsv_ast.js'`, which TypeScript resolves to `./tsv_ast.d.ts`): extensionless
is TS2834/TS2835 under `moduleResolution: node16`/`nodenext`, raised from inside
the package at every consumer without `skipLibCheck`. Both package suites assert
it over every declared `.d.ts` — a regex, not a typechecker, but it pins the
class without putting `tsc` in the package tests. **The TS2308 collision rule
above is still ungated**, so it remains a review-time check. (`ParseOptions` / `TypeScriptParseOptions` and `FormatOptions` /
`TypeScriptFormatOptions` are re-exported **by name** from the generated
`tsv_wasm.d.ts`, which explicit form star-export ambiguation can't drop — but
the names were checked against both files anyway.)

## The `./worker` Entry

Every published variant exports two halves of one worker-pool contract, and
neither is obtainable without them: the node entry's **`wasm_module`** (the
compiled `WebAssembly.Module` behind its exports — the `.wasm` file is not an
exports entry, so a consumer cannot reach it) and the **`./worker` subpath**,
which is `browser.js` re-exported under the name a worker reaches for. A worker
`init_sync({module})`s the module it was handed instead of reading and compiling
the WASM again. V8 keeps a process-wide native-module cache, so a worker that
recompiles the same bytes is not paying full codegen either — but it is paying
module resolution, a redundant 2.4 MB read, and per-isolate setup, which the
handoff skips.

Two reasons the subpath aliases `browser.js` rather than adding a second copy of
the wrappers. It is already exactly the entry a worker wants — lazy `init` /
`init_sync` plus not-initialized guards — and `export *` keeps it the **same
module instance**, so `init_sync` there initializes the singleton every other
import of it observes. What it fixes is reachability: Node and Bun resolve the
bare specifier through the `node` condition, which always lands on the auto-init
`index.js`, leaving the lazy entry unreachable off the browser path.

`npm/cli.js` is the first consumer (see [Files](#files)), but the pair is public
API, and reaches its own copy by relative path — so the SUBPATH is exercised only
by the package suites: `scripts/test_npm.ts` drives a real worker through it by
bare specifier, out of a temp `node_modules` (the only place the `exports` map is
walked the way a consumer walks it, conditions included), and
`scripts/validate_artifacts.ts` pins the same-module-instance claim under Deno by
initializing through `./worker` and then calling `browser.js`.

The declarations are **one file per entry**, and that is what makes
`wasm_module` non-optional where it exists. `index.d.ts` (the Node/Bun entry)
declares it `WebAssembly.Module`; `browser.d.ts` — serving both `browser.js`
and the `./worker` subpath that re-exports it — does not declare it at all,
because neither compiles anything until initialized and neither *has* the
export. A single shared declaration would have to spell it
`WebAssembly.Module | undefined`, which reads as merely inconvenient but is
worse than that: it names an export the browser entry does not have, so a
bundler build fails on something the compiler waved through. The `exports` map
therefore nests `types` inside each condition rather than hoisting it.

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
`push_formatignore(anchor, content)` per discovered `.formatignore` and, inside a repo,
`push_prettierignore(anchor, content)` per `.prettierignore` a directory reads in its
place (all shallowest-first; `pop_gitignore()`/`pop_tsv()` to unwind a DFS — tsv
layers are hierarchical, and the two tsv pushes match identically, differing only in
the file a warning names), then queries:

- `classify_dir(name, child_rel, heuristic_active) -> 'descend' | 'prune' |
  'prune_warn'` — the shared per-directory verdict (`tsv_discover::classify_dir`:
  safety nets, the build-output heuristic, the matcher). On `'prune_warn'` fetch
  the message via `shadow_warning(dir, loose_root?)`.
- `should_format_file(name, child_rel) -> bool` — the per-file verdict (a
  formattable extension and not ignored).
- `is_path_pruned(rel) -> bool` — the per-file form of the directory-prune verdict
  for a consumer with **no top-down traversal** (the VS Code extension formats one
  open document at a time). It walks `rel`'s ancestor directories itself and
  reconstructs each level's `heuristic_active` from the stack's own pushed
  `.gitignore` anchors, so it takes no extra arguments; pair it with
  `is_ignored(rel, false)` for the file-level match. (`classify_dir` stays the
  primitive for `npm/cli.js`, which threads `heuristic_active` down a real walk.)
- `path_shadow_warning(rel, loose_root?) -> string | undefined` — the same
  per-file replay's warning: `shadow_warning`'s text for the first ancestor
  `is_path_pruned` stops at, when it was pruned — by the heuristic or by a rule — under
  a tsv-layer re-include (the `'prune_warn'` a walk would have reached); `undefined`
  for any other prune, or none.
- `excluded_argument_warning(display, rel, is_dir, loose_root?) -> string | undefined`
  — the warning for a path an argument named (a file, or a directory root) that an
  ignore file puts out of scope, naming the file whose rule did it; `undefined` when no
  rule excludes the path, and for a named file a `.formatignore`/`.prettierignore` rule
  excludes (skipped quietly). The scope decision is `is_ignored(rel, is_dir)`, which
  `npm/cli.js` gates every named path on alone (the safety nets and the heuristic grade
  no named path). `loose_root` is the format root's display path outside a repo.
- `shadow_warning(dir, loose_root?) -> string | undefined` (naming the file
  holding the re-include it read from the stack), `prettierignore_outside_repo_warning`,
  `prettierignore_shadowed_warning`, and `gitignore_symlink_warning(path)` — the four
  warning templates, beside `unresolvable_root_error(root)`'s traversal error (methods,
  not free functions, so they ride the class re-export; single source of truth
  with the native CLI, never re-templated in JS).
- `unsupported_extension_error(path) -> string | undefined` — the argument error
  for an explicitly named **file** tsv doesn't format, `undefined` when the
  extension is formattable. Also a method for the class-re-export reason, and its
  receiver is likewise unused (an argument check runs before any matcher exists).
  It carries the rendered extension list, so `npm/cli.js` never hand-mirrors
  `FORMATTABLE_EXTENSIONS`.
- `is_ignored(path, is_dir)` — the raw matcher primitive, still exposed for direct
  consumers.

**Every method has a twin on the N-API `IgnoreStack`** (`crates/tsv_napi/src/lib.rs`,
declared in `crates/tsv_napi/npm/index.d.ts`): `npm/cli.js` ships in both packages and
calls the same methods, and `scripts/test_napi_npm.ts` fails on a method one class has and
the other lacks — a suite `deno task check` does not run, so add the twin in the same edit.

The string-tag return for `classify_dir` (rather than a wasm-bindgen enum or a
returned struct) needs no `patch_npm_package.ts` change and allocates no JS object
on the common descend path. The `is_reincluded` / `negation_under`
primitives are not exported across the WASM boundary — they're folded inside
`classify_dir` (and stay public on the Rust `tsv_ignore::IgnoreStack`), so
JS callers consume the verdict instead of re-deriving the prune decision.

Unlike the parse exports, the class is emitted as `export class` (not
`export function`); `scripts/patch_npm_package.ts` detects `export class` and
carries it through the package facade alongside the functions — verbatim from the
auto-init `index.js`, and as a **guarded subclass** in the two lazy entries
(`browser.js` and the `./worker` subpath it backs), because a constructor cannot
take the per-call init guard the functions do and `new IgnoreStack()` before
`init()` would otherwise report an opaque `TypeError` from the glue where every
other export names the mistake. The subclass adds the guard and nothing else:
the prototype methods, `free()`, `[Symbol.dispose]`, and the generation stamping
the constructor does are all still the base class's, and so is `instanceof` —
in the direction that is asked. What subclassing costs is the OTHER direction:
the name is no longer one constructor across the entries, so an instance from
the auto-init `index.js` is not `instanceof` the lazy entries' `IgnoreStack`.
Two ways to meet that, and only one is guarded. A class RETURNED from Rust would
be built on the base prototype by the glue's own `__wrap` and so fail the check
against the very entry that handed it out — silently, in browsers only — which
`scripts/patch_npm_package.ts` fails the build on (a second
`FinalizationRegistry.register` site is the tell); no class is returned today.
The other is a consumer importing `.` and `./worker` into ONE thread and
comparing across them, which the subpath exists precisely not to be — a worker
imports it, and a bundler resolves both names to the same module. Both package
suites smoke the class (`scripts/validate_artifacts.ts` per variant under Deno,
`scripts/test_npm.ts` under Node — present in format/all, absent in parse-only).
The wasm-bindgen-generated `tsv_wasm.d.ts` declares the class, so no
`tsv_ast.d.ts` entry is needed (the subclass is assignable to it).

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

`deno task check:ast-types` (also part of `deno task check`) runs three arms
against `tsv_ast.d.ts`, described in full at
[docs/audits.md §Wire-Type Drift Check](../../docs/audits.md#wire-type-drift-check-checkast-types):
**(A)** `tsv parse` on a curated set of source snippets, each JSON output
embedded as a typed literal and `deno check`ed — TypeScript's
excess-property checking catches both directions of drift, missing/added
fields and discriminator-string mismatches; **(B)** every `type`
discriminant the fixture corpus produces must be declared here or be
deliberately opaque (the CSS node vocabulary plus the `customElement`
config-data `type:` key); **(C)** a computed minimal
cover of the corpus's committed `expected*.json` — every field SLOT
(`ParentType.key -> ChildType`), in as few files as possible — typed the
same way. Arm C grades this file against the CANONICAL parser's output
rather than against tsv's own, since `expected.json` is what
`fixtures_update_parsed` regenerates from Svelte / acorn-typescript /
`parseCss`.

Extend the `samples` array when a shape wants stating explicitly; arms B
and C need no maintenance — they follow the fixture tree. A `.d.ts` field
typed `unknown` is invisible to arm C, so widening one (as `Script.content`
was, from `unknown` to `Program`) is what puts a region behind the gate.

⚠️ **A node SVELTE builds is not the acorn node of the same `type`, and the tell is
`loc`.** Where Svelte constructs a node instead of handing the text to acorn, the
result carries the acorn discriminator but a different field set — so it needs its
own interface rather than reuse, and a position holding either takes a union. The
four in the wire today, each with its own type:

| position | shape | why |
| --- | --- | --- |
| shorthand `bind:x` / `class:x` `expression` | `SvelteShorthandIdentifier` | the directive name IS the expression; nothing parsed it, so **no `loc`** |
| `<svelte:element this="div">` `tag` | `SvelteFusedLiteral` | no `loc`, and `raw` is Svelte's re-quoted form, not the author's bytes |
| `{@const}` `declaration` | `SvelteConstDeclaration` / `…Declarator` | Svelte builds the wrapper: no `loc` on either, though the `id` inside has one (from Svelte's own reader, so its `loc` carries `character`) |
| `{const}` / `{let}` `declaration` | the ordinary acorn `VariableDeclaration` | parsed, so `loc` throughout — and unlike `{@const}` it may carry MORE THAN ONE declarator |

The inverse mistake is just as easy: typing one of these as its acorn namesake
compiles until a fixture reaches it. The wire-type gate's fixture-cover arm is what
holds the distinction, and it found all four.

⚠️ **Comment attachment is a Svelte-only wire fact with a modelling rule.**
`leadingComments` / `trailingComments` (`AttachedComment[]`) are appended by
Svelte's acorn pass, never by `parse_typescript` / `parse_css`, and the
attachment DFS reaches essentially every acorn node. They are therefore
declared once as `AcornCommentAttachment` and applied two ways, which
between them cover every acorn node: **intersected** into the node unions
(`Expression`, `Statement`, `TSType`, `TSTypeElement`), and **extended**
by the acorn interfaces that positions reference directly by name
(`Identifier`, `Property`, `BlockStatement`, `SwitchCase`, …). A new acorn
node reachable only by name needs the `extends`; the gate is what says so.

`Schema::Acorn` vs `Schema::SvelteScript` deltas the writer emits
require dual updates.

## Files

- `src/lib.rs` — WASM bindings (`lang_bindings!` macro, over the `parse_ast!` / `goal_allowed!` goal axis shared with the two native bindings via [`tsv_arena`](../tsv_arena/), + `read_options` + the hand-written `TS_PARSE_DECLS` / `TS_FORMAT_DECLS` declarations) + the wasm32-gated talc `#[global_allocator]` and panic hook
- `types/tsv_ast.d.ts` — Hand-maintained TS types, bundled into the parse-capable packages
- `npm/cli.js` — The `tsv` bin shipped in `@fuzdev/tsv_wasm` — mirrors `tsv_cli`'s contract (flags, exit codes, traversal); argv parsed by a transcription of argh's grammar, zero deps. Path mode fans onto `node:worker_threads` behind `--jobs`, spawning **itself** as the worker (`isMainThread` splits the two roles) and claiming work off one `Atomics` cursor. `WORKER_FILE_THRESHOLD` gates the **default** only — a JS pool costs tens of milliseconds to bring up against the native pool's ~50 µs thread spawn — while an explicit `--jobs N` bypasses the threshold at any file count and is held to the native CLI's `4 × logical` ceiling (`clamp_worker_count`, restated by hand, over the same logical count — the affinity mask capped by the cgroup CPU quota, which `cgroup_cpu_quota` transcribes from Rust std because Node's and Bun's `availableParallelism()` leave it out), giving the threshold something to be calibrated against. Both the threshold and `default_jobs` are **per engine**, keyed off the same `wasm_module` export that decides how a worker binds: the crossover and the knee are properties of the engine, not the driver (see [../../docs/cli.md](../../docs/cli.md) §Binary Structure). On WASM the pool peaks at *half* the physical cores because V8's wasm tier-up is itself multithreaded and has claimed the rest before the first worker exists; over the N-API addon there is no compiler thread to compete with and it peaks at the core count. A WASM trap is contained to its file on both roles: `format_one` calls `reinstantiate` on any `WebAssembly.RuntimeError`, and on the `RangeError` V8 raises when a deep call exhausts the engine's native stack before its shadow stack (feature-detected — the native engine exports no hook and its overflow is process-fatal), so a too-deep file costs one per-file error instead of poisoning the rest of the run (see [§Panic Reporting](#panic-reporting)). Every pool worker reserves the native CLI's `STACK_SIZE` (`WORKER_STACK_SIZE_MB`, gated against `cli/stack.rs` by `scripts/test_npm.ts`), and the sequential route re-runs a file whose main-thread format hit that `RangeError` in a one-worker pool (`retry_overflowed_files`), so a deep file's verdict no longer depends on which route the file count picked — on Node, whose workers honor the reservation (Bun and Deno ignore it; see [../../docs/cli.md §Recursion Depth](../../docs/cli.md#recursion-depth)). Which engine a worker binds is decided by whether the main thread's `./index.js` exported a `wasm_module`: here it did, so the worker takes it through the [`./worker` entry](#the-worker-entry) and recompiles nothing; in the native package it didn't, so the worker loads the addon. That is why the engine import is **dynamic** — a static one is hoisted above the branch, and the worker would have paid for `./index.js` before it could ask. Also copied into the native `@fuzdev/tsv` by `scripts/build_napi_packages.ts` — one source for both packages (it imports its engine from `./index.js`, so each copy binds to its own package's engine), which the ESM loader bought like `locations.js` below. In the native package it is the *fallback*: the bin there is a napi-only dispatcher (`tsv_napi/npm/bin.js`) that execs the platform package's real `tsv_cli` binary, deferring to cli.js only when no binary is reachable — the dispatcher deliberately does NOT live in this shared source, so the wasm copy stays byte-identical and can never resolve a sibling-installed native binary. Every path it names itself — the `--list` and changed-path lines, `error:` lines, its traversal and argument errors, `parse`'s read failure — goes through its hand restatement of `tsv_discover::quote_path` (a name holding a control character or a double quote is C-quoted as `git ls-files` prints it; the binding's warnings arrive quoted), pinned beside the native rule by `scripts/test_npm.ts` (see [../../docs/cli.md §Multi-File Formatting](../../docs/cli.md#multi-file-formatting))
- `npm/locations.js` + `npm/locations.d.ts` — Pure-JS line/column reconstruction for the span-only `no-locations` wire; ships in the parse-capable packages, re-exported from index.js/browser.js by `patch_npm_package.ts`. Also copied into the native `@fuzdev/tsv` by `scripts/build_napi_packages.ts` — this file is the single source for both, which is what the napi loader being ESM bought (see [Line/Column Reconstruction Helper](#linecolumn-reconstruction-helper-npmlocationsjs))
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
| nodejs | —                  | —                 | `pkg/all/nodejs/` | — / — / `build:wasm:all:nodejs` (bench-only)                      |

The `pkg/all/deno` build feeds the benches and sidecar (it has every
export); the deno builds — subsets and `all` — are size-tracked by `binary_sizes.ts`. The
`npm` builds are the published artifacts: a wasm-pack `web`-target build
patched by `scripts/patch_npm_package.ts` into the multi-entry package shape
(Node auto-init entry, guarded browser entry, conditional `exports`,
metadata, README, the `reinstantiate` glue hook — plus `cli.js` and the `tsv`
bin for the `all` variant).
`deno task test:npm[:parse|:all]` builds the package and then runs Node tests
against it (the `all` variant adds CLI subprocess tests; the `:run` suffix —
e.g. `test:npm:run` — skips the rebuild, as in the publish/CI pipelines, and is
freshness-guarded: `scripts/check_staged_freshness.ts` aborts it when a staged
artifact is older than its sources), and `deno task validate:artifacts`
checks tight wasm size bounds plus a Deno runtime smoke of every built
bundle — the auto-init entries, and the two lazy ones with their
not-initialized guards — under the same freshness guard, since `pkg/` is
gitignored and a stale bundle sizes and smokes exactly as cleanly as a fresh
one (both run in the publish pipeline). The npm package itself covers
Node/browser/bundler consumers, so there is no standalone `web`-target
build beyond the npm artifacts; the `nodejs`-target `pkg/all/nodejs/` build
exists solely to feed the Node bench runner (`build:bench:node`).

The generated `tsv_wasm_bg.wasm.d.ts` is intentionally excluded from the
npm `files` list: it types direct `.wasm` ES-module imports, which the
package shape never uses (bytes via `readFileSync`, URL via `init()`), and
nothing in `tsv_wasm.d.ts` references it — matching blake3's packages.
