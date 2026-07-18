# Script-position `$derived` read slice — design

> Worktree scratchpad. Oracle = svelte 5.56.4 (probed 2026-07-18). Goal: 0
> MISMATCH / 0 over-acceptance on the corpus. Compiler-arc slice — corpus
> `compile_corpus_compare` MISMATCH=0 is the gate. Census: "read of derived
> binding {name}" = **53 sole / 85 co** (top clean in-lane lever after modules).

## What it does
Today a `$derived` binding read in a **script position** (a function body, a
top-level initializer, a `$.derived(() => …)` thunk) REFUSES
(`Refusal::DerivedBindingRead`, `rune_guard.rs:568`). The oracle rewrites every
such read to `d()` — exactly as the template value walk already does for template
positions. This slice extends the derived-read → `d()` rewrite from
template-only to script positions, reusing the `store_rewrite` tree-transform.

## Oracle rules (probe-verified 2026-07-18)
- `function total() { return d + 1; }` → `return d() + 1;`
- `let snapshot = d;` (top-level) → `let snapshot = d();` — **NEVER folds** (script
  positions don't fold; only template text folds). Always `d()`.
- `let d2 = $derived(d + 1);` → `let d2 = $.derived(() => d() + 1);` — a derived read
  INSIDE a `$derived` init thunk rewrites too (the pass runs over the FINAL body, so
  the thunk is reached — same as store reads inside thunks).
- **Name-only positions are NOT reads** (probe): an object/class key (`{ d: 1 }`), a
  non-computed member property (`o.d`) stay verbatim — exactly like `store_rewrite`.
- **Shadowing** (probe): `function f(d) { return d; }` — the param `d` shadows the
  outer derived, so `return d` stays BARE. See §Shadowing.
- **Dropped regions already work** (probe): a derived read only in a dropped event
  handler (`onclick={() => log(d)}`) is already parity — NO change needed there.

## Mechanism (mirror the `$store` script-position slice)
1. **`rune_guard.rs`** — add `WalkCtx::allow_derived_reads: bool` + a builder
   `allow_derived_reads(self) -> Self` (mirror `allow_store_reads`). In
   `walk_expression`'s Identifier arm, gate the **plain-name** `DerivedBindingRead`
   refusal (`:568`) on `!allow_derived_reads` — when allowed, the read passes (the
   rewrite pass handles it). The **escaped-name** refusal (`:581`) stays
   UNCONDITIONAL (the rewrite can't reclassify an escaped read → a bare escaped `d`
   where the oracle emits `d()` would MISMATCH; consistent with needs_context/snippet
   escaped-refuse policy).
2. **`script_rewrite.rs`** — set `.allow_derived_reads()` on the SCRIPT-body guard
   sites (mirror where `.allow_store_reads(store_names, …)` is set: ~773/800/818/839/856,
   and check :352 — the instance-body statement guards). Do NOT set it on the
   pattern guard (`fragment.rs:776 guard_pattern`) — pattern-position derived reads
   MUST keep refusing (a `{#each xs as {v=d}}` / `{#await p then {x=d}}` default,
   item 14b: borrowing verbatim would MISMATCH; both keep the derived rule ON). The
   template value guard (`fragment.rs:741`) is unaffected (template derived reads are
   rewritten by `rewrite_template_value` BEFORE the guard).
3. **`store_rewrite.rs`** — extend `rewrite_value` to rewrite a plain Identifier
   whose name ∈ `derived_names` (the pass already TAKES `derived_names`) → `d()` (a
   `CallExpression` callee=`d`, no args — reuse the template's bare-derived-read
   builder from `build.rs`). Respect the same name-only-position guards the store
   path already has (non-computed member property / object-or-class key = a name,
   never a read). The pass already runs over the final body unconditionally, so a
   derived-only component (no stores) is covered; consider renaming the pass to
   reflect store+derived (writer's call; pre-stable, no compat).

## Shadowing (the one safe simplification)
A derived name shadowed by a nested scope (a param/local named the same) needs
per-occurrence scope resolution (`return d` inside `f(d)` = the param, not the
derived). The store path refuses a shadowed base (the oracle errors there); derived
shadowing is LEGAL, so that model doesn't transfer. **First-cut decision: REFUSE the
whole compile if a derived name is shadowed in a nested scope** (a name in the
nested-declared / `store_shadowed` set) — a safe over-refusal (0 MISMATCH), rare
(a nested local colliding with a `$derived` name), and philosophy-consistent
(ambiguous name-based → refuse). New `Refusal::DerivedReadShadowed { name }` (or
reuse an existing shadow refusal). If the rewrite pass already tracks descent scopes
cheaply, proper per-scope resolution is a fine upgrade — but the refuse-safe default
is the target; do NOT ship a scope-unaware rewrite that could mis-rewrite a shadowed
occurrence (that is a MISMATCH).

## Stays refused (unchanged)
- Pattern-position derived reads (item 14b) — `guard_pattern`.
- Escaped-identifier derived reads (classification not ported).
- A shadowed derived name (§Shadowing, safe over-refusal).

## Fixtures (`tests/fixtures_compile/runes/derived_read_script_*`)
Oracle-generated via `compile_fixture_init`. Cover:
- `fn_body` — `function total(){ return d + 1; }` → `return d() + 1`.
- `top_level_init` — `let y = d;` → `let y = d();` (no fold).
- `nested_in_derived` — `let d2 = $derived(d + 1);` → `$.derived(() => d() + 1)`.
- `member_and_key` — `{ d: 1 }` / `o.d` stay verbatim (name-only positions).
- `deep` — a derived read several statements/blocks deep.
- Refusal lib tests (assert the Refusal): a shadowed derived name
  (`let d=$derived(x); function f(d){return d}`), an escaped derived read, a
  pattern-default derived read (unchanged — still refuses).

## Gates before returning
- `cargo test --workspace --test compile_fixtures_tests` + `cargo test -p tsv_svelte_compile`.
- `deno task compile:corpus:compare` — parity UP from **1230**, MISMATCH 0, error 0,
  no over-acceptance.
- `deno task check` — full gate.
