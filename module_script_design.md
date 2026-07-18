# Module-script slice (v1) — design

> Worktree scratchpad design for the `<script module>` / `<script context="module">`
> slice. Oracle = svelte 5.56.4 (probed 2026-07-18). Goal: 0 MISMATCH / 0
> over-acceptance on the corpus. This is a compiler-arc slice — corpus
> `compile_corpus_compare` MISMATCH=0 is the gate.

## Scope (v1) — PLAIN module scripts only

Corpus fact (grep of all corpus roots): **all 57 module-script files are
rune-free** — module-scope `$state`/`$derived`/etc. does not occur. So v1
supports plain module scripts and **refuses module-scope runes** (a safe
over-refusal; v2 reclaims). Zero corpus yield lost.

**SUPPORT** a module `<script module>` / `<script context="module">` containing:
- `import` declarations → hoisted to module scope (AFTER instance imports).
- top-level `const`/`let`/`var`/`function`/`class` declarations, plain (non-rune).
- `export` forms `export const/let/var/function/class`, `export { x }`,
  `export { x } from 'm'`, `export * from 'm'` → emitted verbatim (post-erase).
- plain top-level statements/expressions.
- TypeScript erasure, gated on the **document** ts flag (which the module's
  `lang="ts"` can set — see §TS flag).
- module bindings participating in the evaluator (folds), reassignment marking,
  and needs_context (module imports + module-body triggers).

**REFUSE** (all safe — refuse, never MISMATCH/over-accept; corpus-gated):
- `export default` in module → oracle ERRORS `module_illegal_default_export`
  → new `Refusal::ModuleDefaultExport`.
- any module-scope RUNE (`$state`/`$derived`/`$effect`/`$props`/`$inspect`/
  `$bindable`/`$props.id`/`$state.snapshot`/`$host`) → reuse the rune guard's
  `Rune`/`DollarPrefixedIdentifier` (walk the module body WITHOUT
  `allow_store_reads`). v1 defers the oracle's module `$state`→v / `$derived`→
  `$.derived(()=>e)` rewrites.
- a module-scope store read (`$name`) → oracle ERRORS `store_invalid_subscription`;
  the guard's `DollarPrefixedIdentifier` covers it (no store exemption).
- top-level `await` in module → guard's `TopLevelAwait`.

## Oracle rules (all probe-verified 2026-07-18)

### Emission ordering
```
import * as $ from 'svelte/internal/server';   // runtime import
<instance imports, source order>               // existing import_program tail
<module imports, source order>                 // NEW — append to import_program
<module non-import body, source order>         // NEW module_body program (comment-free)
<hoisted snippets, if any>                      // existing hoisted_program
export default function Input(...) { ... }      // existing export_program
```
Anchor: probe A. Instance imports precede module imports; the module body
(non-import statements) follows ALL imports, before hoisted snippets / the fn.

### Exports (probe B1/B2/B3)
- `export function` / `export { a }` / `export const` / `export let` /
  `export class` / `export var` → emitted VERBATIM (post-erase).
- `export default` → oracle errors `module_illegal_default_export` → REFUSE.

### Comments (probe CMT + `/* license */`)
- The oracle **DROPS module-script comments** (leading, trailing — both `//`
  and `/* */` gone). Instance-script comments still carry (unchanged).
- tsv reproduces the drop by emitting the module body as a **comment-free
  program** (`comments: Vec::new()`) — `format_canonical` prints from the
  program's comment list, not by positional source lookup (same mechanism the
  import_program uses). So module comments drop → PARITY. No refusal needed.
- ⚠️ `collect_script_comments` (script_rewrite.rs:86) currently REFUSES any
  comment outside the instance content span → a module comment would hit
  `TemplateComments`. FIX: add a module drop-zone — a comment fully within
  `root.module.content.span` is SKIPPED (continue; not carried, not refused).
  A comment outside BOTH scripts (real template) still refuses `TemplateComments`.

### TypeScript flag (probe E)
The oracle's document-wide ts flag = the **first lang-bearing `<script>` in
source order** tests `=== 'ts'`. `document_ts_flag` (script_rewrite.rs:200)
reads only `root.instance` today. FIX: consider BOTH scripts — among {module,
instance} that carry a `lang` attr, pick the earliest by content-span start,
use ITS lang (ts→true, js/empty→false, other→refuse `LangInstanceScript` — or a
module analog). A script without `lang` is skipped. Module TS erases under the
flag (probe E: `interface`/`: number`/`as number` erased in the module).

### Bindings / folds (probes H, R1, R2, C-none-here)
- module `const K = 5` → template `{K}` folds to `5` (probe H). So module
  bindings MUST feed the evaluator binding table (`analyze_script(module_body)`
  into the SAME `bindings`).
- a module `let` reassigned (by a module fn OR an instance fn) → non-foldable
  (probes R2-analog / "module let reassigned by instance fn"): `{cnt}` →
  `$.escape(cnt)`. So reassignment marking must cover the module body, and the
  whole-component reassignment walk (name-based) already catches cross-scope.

### needs_context (probes: module-import-member, module-new, instance-ref-module-import)
The oracle's needs_context walk **includes the module body AND module imports**:
- a module import used in the template (`{api.foo()}`) → wrapper (probe 1).
- a module-body `const y = x.z()` on an import → wrapper.
- a module-body `new Foo()` → wrapper.
- an instance-body member-call on a module import → wrapper.
- a module `const obj` member (`{obj.x}`) → NO wrapper (plain local).
So: (a) module import NAMES must feed `collect_context_roots` (needs_context.rs:204),
and (b) the needs_context trigger walk must cover the module body. Add a
`module_body: &[Statement]` param to `analyze_component` and walk it alongside
`instance_body` (context_roots + the new/member/call trigger walk +
reassignment collection). Module has no props → only imports register.

## Implementation map (integration points)

1. **`analyze()` (transform_server.rs:326)** — remove the `root.module.is_some()`
   refusal (line 331-333). Add:
   - `document_ts_flag` fix (both scripts, first-in-source).
   - erase the module body (`erase::erase_statements`), gated on the doc ts flag
     the same way instance is (`erased.typescript && !ts_document` → refuse).
   - a module statement pass: per (erased) module statement — refuse
     `export default` (`ModuleDefaultExport`); guard-walk (no store exemption)
     to refuse runes/stores/top-level-await + collect `updated`/`nested_declared`;
     split imports (→ new `module_imports` vec) from body (→ `module_body` vec).
   - `analyze_script(module_body, ...)` into the shared `bindings` (folds).
   - collect module import names → feed to `analyze_component` + snippet
     `import_names` (imports don't disqualify hoisting).
   - thread `module_body` into `analyze_component(root, source, instance_body,
     module_body, store_names)`.
   - store the erased module imports + module body on the `Analysis` product
     (new fields `module_imports`, `module_body`) for `compile_server`.
2. **`analyze_component` (needs_context.rs:136)** — new `module_body` param;
   `collect_context_roots` over instance+module; trigger walk over both;
   reassignment collection over both.
3. **`collect_script_comments` (script_rewrite.rs:38)** — module drop-zone
   (skip comments within `root.module.content.span`).
4. **`compile_server` assembly (transform_server.rs ~939)** — append
   `module_imports` to `import_body` (after instance user_imports); build a new
   comment-free `module_program` (the erased module body) inserted between the
   imports and the hoisted snippets; add it to the concatenated
   `format_canonical` print AND to the `self_check_no_typescript` program list.
5. **`Refusal` (refusal.rs)** — new `ModuleDefaultExport` (Display +
   bucket_key). Repurpose/keep `ModuleScript` for any genuinely-unhandled module
   construct fall-through (or remove if the guard+export refusals cover all).
6. **census.rs** — `ModuleScript` leaves the "refused" set for plain modules;
   add `ModuleDefaultExport` to the census bucket list.

## Refusal-boundary reasoning (why each refusal is safe)
- `export default` / module runes / module stores / top-level await: the oracle
  ERRORS or v1-defers; refusing is never an over-acceptance (the oracle rejects
  them too, or we simply don't emit) and never a MISMATCH.
- Module comments: DROPPED (parity), not refused.

## Fixtures (compile fixtures — `tests/fixtures_compile/module/`)
Oracle-generated via `compile_fixture_init`. Cover:
- `plain` — imports + const + export const + a plain fn.
- `exports` — `export function` + `export { a }` + `export let` + `export var`
  + `export class`.
- `import_ordering` — module import + instance import (pin the instance-before-
  module order) + module body after imports.
- `const_fold` — module `const K = 5` + template `{K}` folds to `5`.
- `reassigned` — module `let cnt = 0` reassigned by a module fn → template
  `{cnt}` stays `$.escape(cnt)`.
- `needs_context` — module import used in template member-call → wrapper fires.
- `comment_dropped` — module comment present → dropped, parity (verifies the
  drop-zone, not a refusal).
- `ts` (lang="ts" module) — `interface`/`: T`/`as T` erased.
- Refusal fixtures (compile refuses → no expected_server.js; a `_refuse` marker
  per the compile-fixture convention, or a lib test): `export default`, a
  module `$state`, a module store read.

## Gates before returning
- `cargo test --workspace --test compile_fixtures_tests` (offline parity).
- `cargo test -p tsv_svelte_compile` (lib tests).
- `deno task compile:corpus:compare` → parity UP, MISMATCH 0, over-accept 0.
- `deno task check` (full gate; known-red tsv_lang toolchain clippy is
  pre-existing/out-of-lane if it appears).
