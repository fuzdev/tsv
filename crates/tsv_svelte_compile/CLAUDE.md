# tsv_svelte_compile

> Svelte-to-JS compiler (pinned to Svelte's `compile()` as oracle) plus the JavaScript canonicalizer that makes oracle comparison meaningful.

## Architecture Position

Depends on:

- [`tsv_lang`](../tsv_lang/CLAUDE.md) — `ParseError`, `Span`, the shared interner
- `tsv_svelte` — component parsing (`parse`) and the internal Svelte AST the transform walks
- `tsv_ts` — the internal TS AST the generator constructs, plus `parse_with_goal` and the canonical reprint (`format_canonical`)
- `tsv_css` — the parsed stylesheet the scoping analysis reads
- `tsv_html` — element classification (void elements)

Oracle: Svelte's own `compile()`. The compiler is measured against it not on raw
output bytes but on the *canonical reprint* of both sides (see the canonicalizer
contract below).

See [../../CLAUDE.md §Project Structure](../../CLAUDE.md#project-structure) for
project-wide conventions.

## Module Map

- `lib.rs` — the public API in the tsv free-function pattern:
  - `compile(source, &CompileOptions) -> Result<CompileOutput, CompileError>` —
    parses the component and runs the server transform. Generated JS prints
    through `format_canonical`, so it is canonical-form by construction
    (`canonicalize_js(output.js)` is a fixed point). The control-flow blocks
    `{#if}`/`{#each}`/`{#await}`/`{#key}`, `{@const}`, `{#snippet}`/`{@render}`,
    and **static component invocations** (`<Foo … />` →
    `Foo($$renderer, {…props})` / `$.spread_props`, with default-slot children as
    the implicit `children` snippet prop and `{#snippet}` children as named
    snippet props; dynamic/member components, named slots, `bind:`/`--css-var`/
    directives refuse) are covered (see the transform_server block emitters
    below). Shapes
    the transform does not cover yet — client generation, dev mode,
    instance-script exports in every *value* form (the oracle compiles
    `export const`/`function`/`{a}` via `$.bind_props`, not implemented; rejects
    `export default`/`export let` — a *type-only* export erases away and
    compiles), `generics`, a `lang` other than `"ts"`/`"js"`/`""` (the oracle's
    TypeScript flag tests `lang === 'ts'` exactly, so `lang="typescript"` is
    plain JS to it — tsv refuses rather than guess), TypeScript in a document
    with no `ts` flag (tsv's parser is TS-permissive where the oracle
    parse-errors — an over-acceptance), a comment
    inside an erased TypeScript region, the refuse-don't-erase TypeScript set
    (`enum` incl. `declare enum`, a value `namespace`, a constructor parameter
    property, a decorator, an `accessor` field, an `abstract` *property*, a
    bodiless class method, a class index signature, `import =`/`export =`/`export
    as namespace` — the last four are shapes the oracle mis-compiles into invalid
    JS), top-level `$:` legacy reactive
    statements (invalid in runes mode — a nested `$` label or a plain label is
    ordinary JS and clones through), `svelte/internal*` imports and
    `beforeUpdate`/`afterUpdate` imports from `svelte` (the oracle's runes-mode
    import rules), `{@debug}`, the deliberately-refused legacy attribute directives
    (a legacy `on:` directive and `let:` — a runes-only fence, not a gap) — an element `{...spread}` (alone, or
    co-present with `class:` / `style:` / `bind:` / the no-op drop family) is
    **emitted** as the fused
    `$.attributes(object, css_hash, classes, styles, flags)` call
    (`element.rs::emit_spread_attributes`): the whole attribute set becomes the
    object (plain attributes → `key: value` properties, a `bind:` core kind →
    its synthesized `value`/`checked` property at the bind's slot, event handlers /
    `defaultValue` dropped, spreads → `...expr`), the scope hash rides the
    `css_hash` argument (not concatenated into the class value as in the
    non-spread path; a static-class token OR a `class:` directive name scopes),
    the `class:` directives ride the `classes` argument (the oracle's
    `b.init(name, expr)` — identifier keys, case-preserved, with the
    object-shorthand collapse — `attribute_class_style::build_spread_class_object`), the
    `style:` directives ride the `styles` argument (a **FLAT** object, **no**
    `|important` partitioning — the divergence from the non-spread
    `$.attr_style` array — `attribute_class_style::build_spread_style_object`), and `<input>`
    / a custom element set the `flags` argument
    (trailing absent args elide, interior ones become `void 0`); a spread
    co-present with a legacy `on:`/`let:` refuses (`Refusal::RunesOnlyFence`),
    and a spread on a `<select>` (the `$$renderer.select`
    trap) or on a load-error element refuses (`SpreadOnSelect` /
    `SpreadOnLoadErrorElement`) — a `class:` / `style:`
    directive on a **regular element without a spread** is
    instead **emitted** as the fused `$.attr_class(base, hash, {…})` /
    `$.attr_style(base, {…})` call (`element.rs` /
    `attribute_class_style.rs`), and a `bind:` **core kind** on a regular element without a
    spread is
    **handled** by `attribute_bind::emit_bind_directive` (`bind:this` omits;
    `bind:value`/`bind:checked`/`bind:group` on `<input>` synthesize a
    `$.attr(...)` for a `$state`-rooted target; every other `bind:` refuses via
    `Refusal::BindDirective { name }`) — both the inline and spread `bind:` paths
    share one `attribute_bind::resolve_bind_directive` validity fork; the no-op drop
    family (`use:`/`transition:`/`in:`/`out:`/
    `animate:`/`{@attach}`) is instead **dropped** on a regular element, its
    expression still guarded (a stray rune / `await` refuses) and still walked for
    scope analysis, except a `use:` on a load-error element, which refuses because
    the oracle adds `onload`/`onerror` capture attributes there —
    top-level `await`,
    `<option>` / populated `<select>`/`<optgroup>` (the oracle emits closure
    calls / `<!>` anchors there), template-expression comments, and every
    `$`-prefixed identifier reference or call outside the sanctioned rewrites
    below (a store **read** — `$name` whose `$`-stripped base is a binding — in a
    template OR script position is sanctioned, emitting `$.store_get`; a store
    **write** `$name = v` / **update** `$name++` in a script or dropped-handler
    position is sanctioned too, emitting `$.store_set` / `$.update_store` — see
    `store_rewrite.rs`; a store **member** write (`$obj.x = 5` → `$.store_mutate`),
    a store **destructuring** write (`[$count] = …` → an IIFE), and a subscription
    whose base is bound in a nested scope (`store_invalid_scoped_subscription`)
    still refuse) — return `CompileError::Unsupported` with a
    clear description, never
    guessed output. Within the supported blocks, nested `{#each}` (the nested
    emission path is unvalidated — the unique-name orders themselves ARE modelled),
    a root-level `{@const}`, a destructured `{@const}`, a
    `{@const}` shadowing a `$derived` binding, a member/call rooted at a
    prop/import that is also shadowed in a nested scope (`needs_context`
    classification ambiguous), and a leading comment glued to the `<script>` line
    also refuse. Carried script comments alongside a template block, a component
    invocation, an expression-valued attribute, `{#snippet}`/`{@render}`, or
    hoisted imports **compile** — those emitters write template-region spans only,
    which no script-comment window reaches.
    The output is **self-validated by reparse** before it returns: generated JS
    that `tsv_ts` rejects surfaces as `CompileError::CorruptOutput` (a compiler
    bug — a divergent shape slipped every guard), never a silently invalid
    module. Always on — the reparse costs ~13% of the compile itself (release,
    measured over the fixture corpus; single-digit microseconds per component).
    Reach: it catches output the parser *rejects* (nested `export`, mis-built
    syntax). Output that parses as TypeScript (a passed-through annotation) is
    NOT a parse rejection — that class is caught by the second, independent
    self-check: `erase` is re-run over the finished program, and its
    `None`-means-unchanged contract makes "no change" a *proof* that no
    TypeScript-only node survived (`CompileError::TypeErasureLeak` otherwise).
    Both halves of the erasure — the script `Program` and each template
    expression at its borrow point — run before it, so **any** survivor is a
    compiler bug; it is what makes a missed borrow point loud rather than
    silent.
  - `canonicalize_js(source) -> Result<String, CanonicalizeError>` — the
    canonicalizer (below). Lives here because the compiler's own output
    idempotence checks and the oracle comparison both consume it.
- `refusal.rs` — the typed catalog of refusal reasons: every declined shape is
  a `Refusal` variant carried by `CompileError::Unsupported`, with a `Display`
  message (the human-readable reason `docs/checklist_svelte_compiler.md`
  quotes). The single source of truth for the refusal contract. The enum and its
  `thiserror` messages are inseparable (the message rides a `#[error(…)]`
  attribute per variant), so this file is the catalog and nothing else.
- `refusal_buckets.rs` — the catalog's **accounting projections**, an
  `impl Refusal` block in a sibling module (an inherent impl may live in any
  module of the defining crate, so the split is physical, not an API change).
  A stable `bucket_key` the corpus runner groups by directly (user-chosen names
  collapse to a `{placeholder}` so e.g. every event attribute shares one bucket);
  `every_variant` / `all_bucket_keys`, the enumerable bucket universe the
  conformance audit reads; and `is_deliberate_fence`. They answer a different
  question than the catalog does — not *what shape was declined and how it reads*
  but *how refusals are COUNTED* — and their audience is the corpus runner and
  `refusal_census.rs`, not a person reading an error. ⚠️ The decoupling is load-bearing,
  and is why the two are worth separating rather than merely being large: a bucket
  key is deliberately independent of `Display` **so a message can be reworded
  without shifting corpus buckets**. Keeping them in one file invites the shortcut
  of deriving one from the other, which would silently re-partition every
  historical corpus comparison the next time a message is reworded.
  `Refusal::is_deliberate_fence` splits the catalog in two: a
  **deliberate product fence**, which a runes-only compiler will never implement,
  versus an ordinary "not yet". The fenced set is the legacy **directive syntax**
  and the legacy **slot system**: `RunesOnlyFence` (a legacy `on:` event directive
  and `let:`), the legacy special-element tags `<slot>`, `<svelte:fragment>`,
  `<svelte:component>`, and `<svelte:self>`
  (`special_element_kind::SPECIAL_ELEMENT_FENCED_KINDS`), and `ComponentNamedSlot` (a `slot="…"`
  on a component's child — the *consumer* half of the same slot system those first
  two tags *declare*). Each is deprecation-warned or superseded by the oracle in
  Svelte 5, the slot system by the snippets this compiler already emits.
  **`<svelte:boundary>` is deliberately outside the set**: a first-class Svelte 5
  feature — and it now compiles, so it has no `TemplateNode` label at all; its
  residual refusals (an oracle-rejected attribute, the `failed=`/`pending=`
  attribute forms) are ordinary gaps. So is `ComponentDirective` — what a legacy `on:`/`let:` on
  a *component* raises instead of `RunesOnlyFence` — because that bucket also holds
  unimplemented `class:` / `use:` / `transition:` directives and cannot be fenced
  wholesale.

  Only the unfenced half counts against the achievable-parity denominator.
  `compile_corpus_compare` reads the classifier directly and prints the
  subtraction as its TARGET SET line (`oracle_accepted − fenced = achievable`),
  so the headline is mechanical rather than hand-derived. That `fenced` count is
  a **floor**: it counts a file whose FIRST refusal is a fence, while the
  conceptually right population is every file *containing* a fenced construct
  (the fence is permanent, so a fence behind an earlier refusal is equally
  unreachable). No cheap detector for containment is sound — a node-kind walk
  over-counts component `on:`/`let:` and constructs in SSR-dropped `{:catch}`
  regions, and a source regex over-counts comments — so the denominator stays
  deliberately too large and the parity rate a conservative under-estimate.

  The `refusal_census` **sizes** that floor without moving it: the runner asks it, per
  refused file whose first refusal was not itself a fence, whether a fence is
  present anyway, and reports the count as a separate non-participating line
  (`≥N further refused files CONTAIN a fenced construct`). It is deliberately NOT
  subtracted, for two reasons that survive being tempting. The census tests a
  node's KIND and never inspects an attribute list, so it reaches the fenced
  special-element tags but neither `RunesOnlyFence` nor `ComponentNamedSlot` —
  one of three sources, and the correction it offers is smaller than the
  population it still misses. And its residual error is **not one-directional**:
  it walks every child fragment including a dropped `{:catch}`, where
  `<svelte:self>` / `<svelte:fragment>` / `<svelte:component>` COMPILE rather
  than refuse, so subtracting would book an achievable file as unreachable.
  Together those would trade a rule that is exact and provably a floor for one
  that is neither statable nor provably signed — while raising the published
  parity rate with zero behavior change.
- `refusal_census.rs` — the **sole-blocker refusal census** (`refusal_census` /
  `refusal_census_buckets`, both public), a diagnostic, collect-don't-bail
  companion to `compile`. `compile` bails at the FIRST unsupported construct, so a
  corpus run's per-class counts are first-refusal-only and overstate what
  unlocking any one class would yield; the census instead enumerates every class
  it can independently detect per component, so the runner can price a class as
  the *sole* blocker of N files versus a *co*-blocker. ⚠️ Every class it detects is
  detected by **calling the same guard `compile` calls**, never a copy of the rule
  — a second implementation would drift and mis-price. `refusal_census_buckets` is
  the single source of truth for which bucket keys it attempts, so a class it
  cannot reach independently is declared rather than silently scored as zero.
  ⚠️ Unrelated to `element_census.rs` despite the name.
- `parity.rs` — **the comment-position-tolerant parity comparator**
  (`compare_canonical` → `Parity`). The compiler's parity bar over two canonical JS
  strings: byte-exact, or tolerated when they differ ONLY in comment *position*
  (same code, same comment sequence, no bundler annotation). See §The Parity Bar.
- `namespace.rs` — **the SSR namespace inference** (Svelte's `infer_namespace` /
  `check_nodes_for_namespace` / `determine_namespace_for_children`): the `svg`/`mathml`/`html`
  namespace of each fragment, threaded through emission so the whitespace pass removes
  collapsed inter-node whitespace under `svg` (matching the oracle). Also the
  ancestor-aware `element_is_svg`/`element_is_mathml` classifiers the whitespace,
  attribute-case, and spread-flag paths share (the `<a>`/`<title>` cases are svg only
  under an svg ancestor).
- `erase.rs` — **TypeScript type erasure**, the compiler's `remove_typescript_nodes`:
  a tree→tree pre-pass over the instance script's `Program` producing a type-free
  statement list, run BEFORE every analysis pass and before codegen (the oracle's
  phase-1 placement). Structural sharing via an `Option<T>` return — `None` means
  *unchanged*, so a subtree with no TypeScript beneath it is never rebuilt and
  nothing is allocated; a rebuilt node shallow-clones (children are `&'arena T`,
  so pointers move, never subtrees). The `Statement` and `Expression` matches are
  **exhaustive, no catch-all** — a new AST variant fails compilation here rather
  than silently passing TypeScript through, and `TSType`'s 23 variants are never
  visited (they hang off the dropped `Option` fields). That exhaustiveness plus
  the `None` contract is the whole safety argument: re-running the eraser over the
  *finished* program and getting no change PROVES no TypeScript survived — the one
  check that catches a missed erase, which the output reparse cannot (a surviving
  annotation still parses). Refuse-don't-erase for the runtime-bearing constructs
  and the ones the oracle mis-compiles (see `refusal.rs`); every erased source
  region is recorded, and a comment intersecting one refuses — because the
  oracle's surviving-comment placement is an emergent artifact of its printer's
  flush points over stale spans, not a portable rule. The window widens on **both**
  sides: forward to the next surviving token (so `let x: Foo /* c */ = v` counts),
  and backward to the previous one for a region *detached* from it (a `return_type`
  after `)`, an `implements` clause, a `<T>` list — the printer never queries the
  erased node's range, but the enclosing node's gap window still spans it, so the
  comment would otherwise print anyway, twice for `implements`). A whole-statement
  drop deliberately does **not** reach backward: a JSDoc above an erased `interface`
  survives onto the next statement, exactly where the oracle puts it.
  `erase_expression` is the per-expression entry point the **template's borrow
  points** use (`transform_server`'s `EmitEnv::erase`): every TypeScript-bearing
  markup position is a `tsv_ts` `Expression` reached through a small set of
  borrows, so erasure applies at the borrow and **the Svelte AST is never
  rebuilt**. The erased node is what every consumer of that borrow reads — the
  emitted argument, the static-fold gate beside it (a raw `x as T` would fold to
  UNKNOWN where the oracle folds `x`: a silent under-fold, not a refusal), and
  the shape predicates that switch on a node's variant.
- `build.rs` — synthetic-AST constructors over the **hybrid appendix buffer**:
  the print buffer is the host `.svelte` source plus an appendix of minted
  lexemes. Borrowed user subtrees keep their real host spans; minted
  literal/template-quasi text lives in the appendix at the spans the nodes
  claim; synthetic identifiers ride the interned-name channel
  (`IdentName { escaped: Some(symbol), raw_len: 0 }`, source-free — `ident_at`
  places one at a caller-chosen span, either fictional-low so header comment
  windows stay empty, or *stolen* from the node it replaces so authored gaps
  survive). Codegen owns zero precedence knowledge — the printer's
  `needs_parens` handles it.
- `analyze.rs` — the script binding table and the **static-evaluation port**:
  the oracle folds statically-known template expressions into the emitted text,
  so parity needs the same fold decision. The evaluator mirrors the oracle's
  abstract interpreter over a bounded domain (strings, f64, booleans,
  null/undefined + the STRING/NUMBER/FUNCTION/UNKNOWN sentinels) and refuses
  (`Gray`) anything it can't bound byte-exactly (the oracle's globals tables,
  string→number coercion, non-integer number stringification, …). Bindings
  mirror the oracle: props/updated/no-initial are UNKNOWN; rune inits evaluate
  through to their argument; shadowed names go `Opaque` (refuse-on-spine). Also
  hosts the statement-position rune-call recognizers `is_effect_call` and
  `is_inspect_call` (the latter matching a bare `$inspect(args)` or a single
  `$inspect(args).with(cb)`) that the script rewrite's drops key on.
- `rune_guard.rs` — the rune refusal walk plus the collection passes riding the
  same exhaustive traversal: refuses any `$`-prefixed identifier reference or
  `$`-rooted call outside the sanctioned rewrites — the sanctioned set now
  includes a `$bindable(fallback?)` default at a top-level `$props()` property, a
  statement-position `$inspect(…)`, the `$state.snapshot(x)` and `$props.id()`
  declarator inits, a template-position `$state.snapshot(x)` (→ `$.snapshot`,
  `template_value.rs`), and a **store access** (`$name` where the `$`-stripped base is a
  binding and not a rune — a bare reference OR a call/new **callee root** `$fn()` /
  `$obj.m()` / `new $C()`, via `store_read_exemption` shared by the identifier,
  call, and new arms), which the guard now EXEMPTS in a script or dropped
  position when the caller opts in via `WalkCtx::allow_store_reads` (a
  template-position store read is exempted by `template_value.rs`'s walk before it
  reaches the guard) — the store rewrite (`store_rewrite.rs`) or a dropped-region
  drop handles it. So the guard exempts those positions while still refusing every
  other `$bindable`/`$inspect`/`$state.snapshot`/`$props.id` (value/template
  positions, nested defaults, a wrong-arity or second `.with`, `$inspect.trace`, a
  nested-scope / optional-chained rune, …), a store read reaching the
  **template-value** or **pattern** guard (an unsupported wrapper position, where
  the caller passes no store exemption), a **shadowed** store base in a
  dropped-region position (`store_invalid_scoped_subscription`), and a
  `$name` whose base is not a binding (the oracle's `global_reference_invalid`) —
  refuses a derived-binding
  read no rewrite turns into `d()` — a pattern default, a read under an
  unsupported wrapper, or an escaped-identifier read whose decoded name is a
  `$derived` binding; a **script-position** read is EXEMPT when the caller opts in
  (`allow_derived_reads`, the script-body guards — the read is rewritten by
  `store_rewrite`), while a **write** to a derived binding (`d = v` / `d++`, out of
  scope — the oracle lowers it to `d(v)` / `$.update_derived(d)`) refuses on every
  path. Also refuses top-level `await`, and collects
  assignment/update roots (`updated`) and nested-scope declarations (shadow
  candidates) for the evaluator. Exhaustive matches on purpose — new AST
  variants fail compilation here instead of silently skipping the guard.
- `needs_context.rs` — the `needs_context` analysis (ports Svelte's phase-2
  accumulation): does the component require the
  `$$renderer.component(($$renderer) => …)` wrapper? Walks the whole un-folded
  instance + template AST (exhaustive matches) and sets the flag on any `new`
  expression, or a member/call whose root (`is_safe_identifier`) is not a plain
  identifier or is a prop/import binding — a plain local, a global, and rune
  bindings stay safe. A member/call rooted at a prop/import that is *also* bound
  in a nested scope is ambiguous for this name-based port and refuses, as does one
  rooted at an escaped identifier (classification not ported). Descends
  into `{#snippet}` bodies (a function-like subtree — a `new`/prop-rooted access
  there still fires the flag) and `{@render}` arguments. Also computes
  `uses_stores` in the same whole-component walk — the oracle's analysis-driven
  store-subscription gate: any valid `$name` store reference *anywhere* (read or
  write, emitted or dropped — an event handler, `{:catch}`) sets it, so the
  `var $$store_subs;` / `$.unsubscribe_stores(…)` injection fires for a store used
  only in a dropped handler too. It is decided here, NOT at emission time.
  Because this is the one walk that reaches **every** assignment, update and `bind:`
  in the component — both scripts, the template, and the dropped regions — it also
  hosts the port of the oracle's `validate_assignment` family
  (`phases/2-analyze/visitors/shared/utils.js:18`, itself one function reached from
  `AssignmentExpression`, `UpdateExpression` and `BindDirective` alike). One refusal,
  `Refusal::InvalidAssignmentTarget`, carries its three rules: `constant_assignment`
  (any `const`-declared binding in scope at the write — a top-level declarator or
  import local from either script via `collect_constant_names`, the set the `bind:`
  gate also reads as `unassignable_names`; a NESTED script `const` via `js_scope`;
  and a TEMPLATE-scoped one via `template_consts`, all three detailed below),
  `each_item_invalid_assignment` (an `{#each}` context binding,
  block-scoped to body + fallback) and `snippet_parameter_assignment` (a `{#snippet}`
  parameter, block-scoped to its body; NOT runes-gated in the oracle). The pattern
  recursion mirrors `validate_no_const_assignment` exactly — `ArrayPattern` elements
  and `ObjectPattern` property *values* only, so a `RestElement`, an
  `AssignmentPattern` default and a `MemberExpression` are accepted — while the
  each/snippet rules test the whole argument, as the oracle does. Membership is
  **scoped**, not merely name-based: beside the cumulative `shadowed` union the walk
  carries `js_scope`, a STACK of the JS bindings of the scopes currently OPEN around
  it (a function's parameters and name, a `catch` parameter, a `for`-head binding, a
  nested `let`/`const`/`var`/`class`/function), each entry carrying whether it is a
  `const`; a lookup scans backward, so the INNERMOST binding decides. ⚠️ Recording a
  binding is not the same as suppressing the rule: a nested `let`/parameter/`catch`
  binding carries no rule and the write is accepted, but a nested `const` is
  `declaration_kind: 'const'` to the oracle wherever it sits, so it carries
  `constant_assignment` itself and the write REFUSES. That is why the stack stores the
  kind rather than mere membership — a uniform "shadow ⇒ no rule" set compiled a write
  the oracle rejects, and the two nested orderings have opposite verdicts
  (`let a; { const a; a = 1 }` refuses, `const a; { let a; a = 1 }` compiles). The
  enumeration of declaration FORMS is separately allowed to be incomplete, but ⚠️ a miss
  there is **not unconditionally safe** — reading it as such was itself an
  over-acceptance. An unrecorded binding makes the write fall through, and what it falls
  through TO decides the direction: when the name is ALSO in a component-level set that
  set's rule fires and the write over-REFUSES (safe), but when the name is purely LOCAL
  nothing fires at all and the write is ACCEPTED — which for a `const` local is an
  over-acceptance and a bug. So a missing NON-const form is safe (it carries no rule
  either way): a `var` scopes to its block rather than its function, a `let`/`class`/
  function name is recorded where the walk reaches it rather than hoisted, and a class
  EXPRESSION's own name is unrecorded — the last two harmless because the oracle
  declares a class name `'let'`, not `const`. A missing `const` form is not: a `switch`
  therefore now gets ONE scope shared by all its cases (the oracle's `SwitchStatement:
  create_block_scope`) and a block's `const` declarations hoist into scope before its
  statements are walked (the oracle's scope pre-pass), closing two over-acceptances. The
  hoist is deliberately `const`-only — hoisting a rule-free binding could only remove a
  refusal. The other unsafe direction is a binding OUTLIVING its scope, which would
  suppress a genuine refusal; the stack's truncation forecloses it.

  The **template-scoped** consts — a `{@const}` name, a `{:then}`/`{:catch}` value,
  and the `{#each}` INDEX, all `declaration_kind: 'const'` to the oracle — are
  recorded in `js_scope`'s sibling `template_consts`, block-scoped at the extent the
  oracle's own scope covers (a `{@const}` to its enclosing fragment, entered before
  any of that fragment's nodes is walked so the oracle's scope pre-pass is mirrored;
  a `{:then}`/`{:catch}` value to that branch; an index to body + fallback). It is
  consulted after `js_scope` and before the each/snippet sets — the safe order, since
  the const rule fires at any pattern depth while those two fire only on a
  whole-identifier target. Because a `bind:` reaches the same validator, this is also
  where a bind to a template-scoped const is refused; `unassignable_names` sees
  top-level script statements only and is blind to them.

  ⚠️ The `{#each}` INDEX and the ITEM beside it take DIFFERENT rules, and conflating
  them is a bug in either direction: the item is `('each', 'const')` and
  `validate_no_const_assignment` EXCLUDES `kind === 'each'` in favor of
  `each_item_invalid_assignment`, while the index is `('template' | 'static',
  'const')` and carries `constant_assignment`.
- `store_rewrite.rs` — **store-access (and script-position `$derived` read)
  rewriting** for the instance script (the
  script analog of `template_value.rs`'s template value walk). A tree→tree pass over the
  FINAL synthetic body (after erasure + rune rewrites, so a read inside a
  `$.derived(() => …)` thunk is reached) with `erase.rs`'s `Option<T>`
  structural-sharing shape and exhaustive matches: a store **read** `$name` →
  `$.store_get(…)` at any depth; an **assignment** `$name = v` → `$.store_set(name,
  v)` and a compound `$name += v` → `$.store_set(name, $.store_get(…) + v)`
  (reconstructing the binary the oracle's `build_assignment_value` produces); an
  **update** `$name++`/`++$name`/`$name--`/`--$name` → `$.update_store[_pre]((…),
  '$name', name[, -1])`. It also rewrites a plain **`$derived` read** → `d()` (the
  script analog of the template value walk's bare-derived rewrite — a top-level
  initializer, a function body, a `$.derived(() => …)` thunk; the minted `d()`
  takes the callee's **tight** span so it never sweeps a carried script comment). A
  binding-position id (`let d = …`) is skipped, and a **write** to a derived
  (`d = v` / `d++`) and a *shadowed* derived name are refused upstream (the rune
  guard and `compile_server`), so they never reach the pass. Refuses a store member
  write (`$obj.x = 5`), a store destructuring
  write (`[$count] = …`), and a shadowed store base (`store_invalid_scoped_subscription`,
  `store_shadowed` = `nested_declared` ∪ `component.fn_declared`). Respects
  **name-only positions** (a non-computed member property / object-or-class key is
  a name, never a read) — the one place it diverges from `erase.rs`. Builders live
  in `build.rs` (`store_set`, `update_store`, sharing `store_subs_assign`/
  `store_base_value` with `store_get`; `call_expr` for the `d()` read).
- `snippet.rs` — the `{#snippet}` hoist analysis (name-based port of Svelte's
  `can_hoist_snippet`): which top-level snippets go to true module scope. Collects
  each snippet's free references (a flat scope-tracking walk) minus its bound
  names; a free reference to an instance binding (prop/`$state`/`$derived`/plain
  top-level decl — *not* imports/globals) blocks hoisting, and a name that is both
  an instance binding and a nested local is ambiguous and refuses. Hoistability is
  a fixpoint over snippet-to-snippet references. The reference collection is
  name-based, but the analysis **product** is keyed by snippet IDENTITY
  (`SnippetBlock::span.start`, a `HashSet<u32>` of hoistable top-level snippet
  spans read by `is_hoisted`), not name — only top-level snippets are inserted, so
  a NESTED snippet sharing a top-level snippet's name is never mistaken for it and
  lands in the body regardless of its twin's verdict (the module-export check
  `has_hoisted_snippet_named` keeps the name-based query it genuinely needs). Also
  collects every snippet name (render-callee classification, generated-name
  collisions).
- `attr_refs.rs` — the **shared template traversals**, so no analysis hand-writes
  its own walk and drifts (which is how the component-spread arm once existed in
  one and not the other). Three levels:
  - the element-attribute pair — `each_attribute_expression`, the emitted-path
    view (everything not refused at emission: plain values, a `{...spread}` on
    **either** element kind (a component's `$.spread_props` array element and a
    regular element's fused `$.attributes({ …spread })` object element both emit
    it), a
    `class:` / `style:` directive's expression-bearing value on a regular element
    (emitted as `$.attr_class` / `$.attr_style`), a `bind:` directive's target
    expression on a regular element (the oracle's analysis visits every bind
    expression regardless of SSR emission, so a snippet whose only instance-binding
    reference sits in a bind must not module-hoist),
    and the no-op drop family `use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`
    on a regular element, dropped-but-analyzed like an event handler; its
    `each_emitted_directive_name` companion surfaces the drop-family directive
    *names* plus a `style:` shorthand name an expression traversal can't reach; the
    refused positions — legacy `on:`/`let:` — are
    skipped, the refusal keeping their references out of output), and
    `each_reference_bearing_attribute_expression` (+ the directive-name and
    special-element entry points), the **dropped-fragment** view, which includes
    every position. An attribute shape that newly reaches emission must be added
    HERE so every analysis sees it at once;
  - `each_template_item`, the whole-fragment walk over the dropped-fragment view,
    yielding every borrowed expression (plus a `{#snippet}`'s `<T>` clause, which
    is TypeScript with no expression to yield). Its two consumers ask what a
    region *contains* rather than what it *emits* — the document-wide TypeScript
    gate and the rune guard over a dropped `{:catch}`. Exhaustively matched: a new
    template shape fails compilation rather than slipping past both;
  - `each_child_fragment`, the pure structural seam — the one
    exhaustively-matched answer to "which sub-fragments does this node contain"
    (element/special-element fragments, `{#if}` branches, `{#each}` body+fallback,
    `{#await}` pending/then/catch, `{#key}` fragment, `{#snippet}` body). The
    whole-document validator (`validate.rs`), the snippet-name collector
    (`snippet.rs`), the `$$index` name allocator (`blocks.rs`), and the sole-blocker
    refusal census (`refusal_census.rs`) recurse through it, so the recursion shape has a
    single home. A new `FragmentNode` variant, or a new child
    fragment on an existing variant, fails compilation HERE rather than drifting
    across hand-written copies. Three walks deliberately do NOT ride it, each
    carrying its own exhaustive match (no catch-all) and stating why in code. The
    scope-tracking / dropped-`{:catch}` walks (`needs_context.rs`, `snippet.rs`'s
    free-variable collector) opt out because their
    descent is entangled with per-node scope binding, the emission-vs-dropped
    distinction, and the `{#await}`-catch flag toggle, which a uniform enumeration
    can't carry without changing behavior. ⚠️ `element_census.rs` — NOT to be
    confused with `refusal_census.rs`, which does ride the seam — opts out because it
    descends a deliberately *different* node set (wider than emitted at
    `<svelte:boundary>` and `{:catch}`, narrower at the fenced special elements)
    while threading an ancestor `path` and `Owner` per frame.
  The SSR output **drops** four regions without visiting them — the `{#each}`
  key, the `{#key}` expression, an event-handler attribute, and the whole
  `{:catch}` branch — so no emission refusal can fire inside them. But the oracle
  decides TypeScript at *parse* time and rune placement at *analysis* time, both
  before it chooses what to emit, and it counts references wherever they sit. So
  a dropped region still gets all three walks (`transform_server`'s
  `refuse_template_typescript` / `guard_dropped_fragment`, and the analyses'
  dropped-fragment view) — but **not** the emission refusals, and not the
  derived-read rule, which is an emission rewrite rather than a validity rule.
  Those walks all ask what a region *references*. A fourth question — what a
  dropped node *is* — needs its own walk over node KINDS, because the oracle also
  keys some phase-2 facts on a node's mere **presence**, which dropping the region
  does not suppress. `guard_dropped_fragment` therefore runs
  `guard_dropped_presence` (`dropped.rs`) alongside the expression walk, recursing
  through `each_child_fragment` and reaching each node's attribute list.
  Presence-read facts run on **two axes**, and the second is the one that is easy
  to miss:
  - **emission** — the fact rides into the generated code. A `<slot>` in a
    `{:catch}` records into `analysis.slot_names` and widens the component
    signature to `($$renderer, $$props)`. Measurable one construct at a time.
  - **validation** — the fact feeds a whole-component check that can turn an
    otherwise-valid component into a compile *error*. A legacy `on:` in a
    `{:catch}` sets `analysis.event_directive_node`; with an `onclick` on any
    emitted element the oracle raises `mixed_event_handler_syntaxes`. This axis is
    invisible to a per-construct probe — it needs a second construct elsewhere in
    the component to fire.

  The scoping rule is **"refuse where the construct can affect the result"**,
  deliberately narrower than "a fence refuses everywhere": `<svelte:component>`,
  `<svelte:element>`, `<svelte:boundary>`, `<svelte:self>`, `<svelte:fragment>` and
  a `slot="…"` child are on neither axis and must keep compiling in a dropped
  `{:catch}` (`<svelte:boundary>` is not even fenced). `let:` is also on neither
  axis but refuses anyway, sharing `on:`'s fence bucket.
  `dropped_presence_refusal`'s exhaustive `FragmentNode` / `SpecialElementKind` /
  `AttributeNode` matches are what force a new variant through **both** questions.

  ⚠️ ONE axis-2 hole is **open**, an over-acceptance, not corpus-reachable:
  `{$$slots.x}` in a dropped region + an emitted `{@render}`
  (`slot_snippet_conflict`, `2-analyze/index.js:862`). `$$slots` is not fenced, so
  closing it means porting the oracle's whole-component validation rather than
  widening the presence match — tracked in `../../docs/checklist_svelte_compiler.md`.
  Its former sibling, a dropped `{#snippet}` + `export { … }` of it from a module
  script, is closed in `validate.rs` — and the rule is narrower than that phrasing:
  the error needs a snippet the oracle cannot HOIST, which a dropped one never is,
  while a top-level `{#snippet s()}` beside `export { s }` compiles on both sides.
- `validate.rs` — the **emission-independent validation** pass: one walk over the
  whole document (riding `attr_refs`'s `each_child_fragment` seam), run at the top
  of `analyze()` before any emission decision. Home for the oracle rules that fire
  wherever their construct sits — **including a region SSR drops** — so neither the
  emitters nor `guard_dropped_presence` alone can host them. Three parse-time rules:
  `attribute_duplicate` (per-element, kind+name keyed with
  `bind:` normalized onto plain, `this` never recorded) and
  `svelte_meta_invalid_placement` / `svelte_meta_duplicate` over the oracle's
  `root_only_meta_tags` (`<svelte:head>` plus the SSR-inert three; `<svelte:options>`
  is refused upstream). ⚠️ The inert three's placement/duplicate rules **moved here
  from `fragment.rs`**: an emitter never runs on a dropped region, so one of them in
  a `{:catch}` compiled. A rule whose inputs are not emission state belongs here, not
  at an emitter.

  Plus the five **snippet declaration/export** rules, from two oracle sites. Three ride
  the same walk: `declaration_duplicate`'s `Scope.declare` call site
  (`phases/scope.js:684-691`) as a per-**fragment** duplicate-snippet-name check — the
  scope is the fragment, not the component, so `<div>{#snippet a}…{/snippet}</div>`
  plus a root `{#snippet a}` is legal — and `snippet_shadowing_prop` /
  `snippet_conflict` (`SnippetBlock.js:59`/`:77`), both checked from the snippet's
  PARENT because the oracle's `path.at(-2)` is exactly "the node whose fragment holds
  this snippet". ⚠️ The two parent sets deliberately DISAGREE: shadowing is `Component`
  only, conflict also takes `<svelte:component>`/`<svelte:self>` — do not harmonize
  them. Shadowing also does not fire at depth. Two more rules run LATER, from
  `analyze()`, because their inputs are analysis products: `validate_top_level_snippets`
  (`declaration_duplicate` at `SnippetBlock.js:34` — a ROOT-fragment snippet whose name
  the instance script declares) after the binding table, and `validate_module_exports`
  (`snippet_invalid_export` / `export_undefined`, `index.js:823-836`) after the hoist
  analysis. ⚠️ That last one checks **module scope FIRST, snippet names second**: a
  hoistable top-level snippet's binding is written INTO module scope
  (`SnippetBlock.js:40-44`), so checking the snippet set first would reject every valid
  exported snippet.

  Plus the attribute rules of the oracle's single `validate_element` loop —
  `attribute_invalid_name`, `attribute_invalid_event_handler` (an `on…` attribute
  needs a SINGLE-expression value; the name test is `startsWith('on') && length > 2`,
  so a bare `on` is legal and `onx` is not, and a BARE `onclick` is rejected along
  with `onclick="foo"`) and `slot_attribute_invalid_placement`. That loop's only
  callers are `RegularElement.js` / `SvelteElement.js`, so a **component** is exempt
  from all three. The rules fire in the oracle's per-attribute order (one loop,
  first error wins: unquoted-sequence → sequence → name → event handler → slot), so
  a multi-error element reports the oracle's own bucket. ⚠️ TWO attribute rules are
  NOT element-only. `attribute_unquoted_sequence`
  (`refuse_unquoted_attribute_sequence` — an unquoted value of 2+ chunks like
  `href=/{path}`; the quote test is the oracle's last-chunk-end vs attribute-end
  span comparison) is `validate_attribute`, called from `shared/element.js:43` AND
  `shared/component.js:93` alike. And `attribute_invalid_sequence_expression` — a
  component reaches it through its own visitor (`shared/component.js:174`), which
  ALSO applies it to an `{@attach}` expression where the element half does not, so
  `<span {@attach a, b} />` compiles and `<Foo {@attach a, b} />` refuses. It
  therefore lives in a shared `refuse_unparenthesized_sequence` called from both
  paths rather than folded into either. Its parenthesization test is the oracle's backward SOURCE scan, not a span
  comparison: ESTree drops parens, so the byte before the sequence's start is the only
  record of them — which is why a nested `{[x, (y, z)]}` is legal while `{(x), y}` is
  not.

  Plus the analysis-phase `node_invalid_placement` (the HTML content model), for
  which the walk carries a `path` of `PathEntry`s — the oracle's `context.path`
  reduced to what its ancestor loop reads. Three distinct node roles, and collapsing
  any two is a bug: a **`Barrier`** (a component, `<svelte:element>`, `{#snippet}`)
  both resets `parent_element` and stops the walk; **`ParentReset`**
  (`<svelte:fragment>` alone) resets but is transparent to the walk — the oracle's one
  asymmetric node, unreachable today because the tag is fenced, modelled anyway; and
  **`Block`** (`{#if}`/`{#each}`/`{#await}`/`{#key}`) downgrades the violation to a
  WARNING, so tsv must NOT refuse below one. `<svelte:boundary>`, `<slot>`, `<title>`
  and the root-only meta tags are transparent to all three and are not pushed. The
  tables live in `html_tree.rs`.
- `html_tree.rs` — a faithful port of the oracle's `src/html-tree-validation.js`:
  the `disallowed_children` tables plus `is_tag_valid_with_parent` /
  `is_tag_valid_with_ancestor`, returning the oracle's own message. Deliberately a
  transcription, not a spec implementation — the oracle lists only what a browser
  *repairs*, so the HTML spec's full content model would over-refuse. Two
  transcription traps are called out in the module docs: the JS object spread
  REPLACES the `tr`/`tbody`/`thead`/`tfoot` entries (they lose their `direct` list),
  and the reset scan — including its custom-element short-circuit — is gated on
  `reset_by` being present, so only `dt`/`dd` reset.
- `text_class.rs` — the **target** languages' lexical character classes
  (`is_js_whitespace` / `js_trim` / `js_char_at` / `is_css_whitespace`), for the
  source scans that reason about text without tokenizing it. ⚠️ Rust's
  `char::is_whitespace` is the Unicode `White_Space` property, which differs from
  ECMAScript `WhiteSpace` in **both** directions — `U+FEFF` is JS whitespace but
  carries no `White_Space` property (so a Rust-classed scan **under**-reports, and
  `static\u{FEFF}{…}` was invisible to `script_collision.rs`'s static-block fence),
  while `U+0085` is the reverse (which only ever over-refuses). A scan whose
  whitespace notion is the HOST language's rather than the TARGET's is a recurring
  defect in this crate, so the class lives here once rather than being re-derived
  per scan.
- `transform_server.rs` — the SSR transform **orchestrator**: `compile_server`
  runs the phase-numbered pipeline (TypeScript erasure/gate, CSS scoping — the
  element census built and every selector chain matched against it **upfront** in
  `analyze()`, script analysis, snippet hoist analysis, script rewrite,
  `needs_context`, template emission, wrapping, assembly/print) and owns
  `EmitEnv`, the struct threaded through every emitter in the sibling modules
  below — the builder, the binding table, the derived-name set, the finished CSS
  scope (`CssScoping`, read-only — `element_scope` is a span lookup),
  block-scope overlays, snippet hoist state, and the erased-region windows
  every `EmitEnv::erase` call collects. Module scaffold: `import * as $ from
  'svelte/internal/server'`, then any instance-script `import` declarations
  hoisted to module scope in source order (an import inside the component
  function is invalid JS) + the exported component function. The whole body
  wraps in `$$renderer.component(($$renderer) => { … })` whenever
  `needs_context` fires (a dropped effect, the new/member/call analysis in
  `needs_context.rs`, or a non-empty `$bindable` set), which also forces the
  `$$props` parameter. A non-empty bindable set additionally emits
  `$.bind_props($$props, { … })` as the component body's last statement (a
  dropped `$inspect` never contributes here — its wrapper comes only from
  `needs_context`). Any valid store access (`EmitEnv::uses_stores`, computed
  upfront by `needs_context`, not at emission) injects
  `var $$store_subs;` as a component-body statement (after the `$props.id()` hoist,
  before the body) and `if ($$store_subs) $.unsubscribe_stores($$store_subs);` as
  the last statement (before any `$.bind_props`) — both at the component-body level
  and INDEPENDENT of the wrapper (a store access does not force `needs_context`).
  The script store rewrite (`store_rewrite.rs`) runs over the instance body between
  the rune-rewrite loop and `EmitEnv` construction, using the `store_names` /
  `store_shadowed` sets frozen there.
The **script side** is seven modules, split along the line a second transform
would need: five are target-independent (the oracle decides them before it
chooses what to emit), two mint server-module syntax. They are listed here in
pipeline order.

- `script_ts_gate.rs` — the document-wide TypeScript flag and gate
  (`document_ts_flag` / `refuse_template_typescript`) plus
  `self_check_no_typescript`, the type-erasure self-check that closes the loop on
  the finished program (see `erase.rs`). Oracle phase 1, target-independent:
  Svelte decides TypeScript at *parse* time for the whole document at once, so
  the decision belongs to the document, not to a `<script>` tag — both top-level
  scripts are considered in source order and the FIRST lang-bearing one decides
  (`lang === 'ts'` tested exactly, so `lang="typescript"` is plain JS to the
  oracle and tsv refuses rather than guess). The template half exists for the
  expressions that never reach output — the SSR-dropped `{#each}` key, the
  `{#key}` expression, the `{:catch}` branch, event handlers — whose TypeScript
  the erase self-check therefore cannot see. ⚠️ Both halves are sound only if
  `tsv_svelte`'s parser PRESERVES every TypeScript node it parses: a node the
  parser drops is one this cannot refuse (the block-pattern `: T` that was parsed
  and thrown away is the precedent).
- `script_decls.rs` — the **single exhaustive answer** to "what does this script
  declare at script scope?" (`each_script_declaration`; `ScriptDeclaration` =
  declarator / function / class / import-local, `VarScope` selecting whether a
  function-scoped `var` hoisted out of a nested block or for-head is included),
  plus the `plain_identifier_name` / `identifier_binding_name` helpers the script
  analyses share. The script-side analog of `attr_refs.rs`: both the
  binding-table analysis and the collision pre-pass route through it, so the
  `Statement` enumeration exists once; the match is exhaustive on purpose, so a
  new AST variant fails compilation instead of silently escaping a guard. Its
  `top` flag is what encodes strict-mode scoping — below the script's own
  statement list only a `var` reaches script scope — and its `porous` flag
  records whether a porous scope sat on the way up, because the oracle re-declares
  a hoisting `var` on the parent **without its initializer**
  (`scope.js:673-681`), which `ScriptDeclaration::Declarator::initial_dropped`
  carries to the consumer. A class body is deliberately **opaque** to it, and so
  is every expression position: a class **static block** is the one nested
  statement list that is not a scope at all in the oracle (`phases/scope.js` has
  no `StaticBlock` visitor), so a `var` there does declare at script scope with
  its initializer intact — but reaching every class body a script can hold means
  enumerating every expression position of every statement, a surface that
  shipped holes twice (a class expression in a for-head, in a `super_class`, in a
  property initializer — which is NOT a function scope, there being no
  `PropertyDefinition` visitor either — in a computed key, in a parameter
  default), each hole a silent MISMATCH. `script_collision.rs` covers the whole
  family with a lexical fence instead.
- `script_bindings.rs` — the top-level binding-table analysis
  (`analyze_script`/`analyze_declarator`), the module-script analysis
  (`analyze_module_script`), and the runes-mode import rules
  (`refuse_runes_invalid_import`). Oracle phase 2, target-independent: this
  classifies what the script *declares* and what the evaluator may fold, not what
  any transform emits. A `$state.snapshot` binding stays UNKNOWN to the evaluator
  even though the server unwraps it — the unwrap is the emission form, not the
  evaluation form: the oracle evaluates a rune declarator through its argument for
  `$state` / `$state.raw` / `$derived` only, and every other rune falls to its
  `default` arm and yields UNKNOWN (`phases/scope.js:469-503`), so a template read
  never folds (`$.escape(s)`). That holds however the argument itself evaluates —
  a plain `let` argument does not fold either. The duplicate-`$props()` flag is
  per-SCRIPT state, scoped to one `analyze_script` call, mirroring the oracle's
  fresh `has_props_rune` per script. v1 supports **plain** module scripts only: a
  module-scope rune, a `$name` store read, or a top-level `await` refuses (the
  corpus is rune-free, so a lossless over-refusal), and a supported module body
  emits verbatim post-erase.
- `script_collision.rs` — `refuse_rune_store_collision`, a pre-pass over the WHOLE
  component, run before the binding table is built. A rune keyword whose
  `$`-stripped stem is also a binding in scope at the instance script
  (`import { state } from './store'` beside a `$state` reference) is read by the
  oracle as a **store subscription**, not as the rune (`2-analyze/index.js`, the
  "create synthetic bindings for store subscriptions" loop), and the reference is
  deleted from `module.scope.references` before runes-mode inference — so the
  collision can flip the whole component out of runes mode. tsv models neither, so
  it refuses. The scope tested is the oracle's `instance.scope.get`, which walks
  **up** into the module scope (`scope.js:748`; the instance scope's parent IS
  `module.scope`) and never **down** — a function parameter, a block-scoped
  `let`, and a name bound in a nested function body are child scopes and keep
  compiling — plus the two nested forms that DO reach script scope, a hoisting
  `var` (modelled exactly, via `script_decls.rs`) and a class static block. The
  oracle's exemption (a binding whose `initial` *is* a rune call) is modelled,
  which is why the common `let state = $state(0)` shapes are unaffected; it reads
  the oracle's `binding.initial`, so a rune-initialized `var` that hoisted through
  a porous scope is **not** exempt (its initializer was dropped).

  Two deliberate imprecisions, both over-refusing on purpose — the direction
  matters, since a missed binding is a MISMATCH while an extra refusal is a gap:

  - the static-block **lexical fence** (`script_contains_static_block`): a
    component containing any `static { … }` refuses on its first rune reference,
    rather than traversing the expression positions a class body can hide in. The
    scan is complete for a static block **exactly as far as its whitespace class
    is ECMAScript's** — a static block is `static`, then trivia, then `{`, and its
    token always sits inside a statement's span, so the only way to miss one is to
    mis-classify the trivia. It therefore matches with
    `text_class::is_js_whitespace`, never Rust's `char::is_whitespace`: the two
    differ at `U+FEFF` (ECMAScript `WhiteSpace`, but not the Unicode `White_Space`
    property), and `static\u{FEFF}{ … }` was invisible to the fence, compiling the
    rune where the oracle emits a store read. Over-reporting stays harmless
    (`static` in a comment or string, a `/` that is division, a `U+0085` that JS
    would reject anyway) — measured at zero, none of the ~4900 `.svelte` files
    under the compile-corpus roots contains a static block;
  - the `$stem` REFERENCE test, a whole-document, boundary-checked source scan
    rather than an AST walk: tsv recognizes a rune at half a dozen scattered sites
    and a per-site check can miss one (an under-refusal = a MISMATCH), while one
    scan cannot. Its cost is over-refusing a document that merely mentions `$state`
    in a comment, a string, template text, or as a member/property NAME
    (`obj.$state`). The boundary test decodes CHARACTERS, not bytes — a byte-level
    shortcut reads NBSP's lead byte as identifier text and MISSES `$state (1)`
    written with one.
- `script_comments.rs` — `collect_script_comments`: which host comments carry into
  the synthetic program. **Server-specific**, because every rule here reasons about
  the *oracle's printer* (esrap) — where its single `comment_index` sits when a
  given block opens, and which synthetic span windows a carried comment falls
  into. Instance-script comments carry through (host-absolute spans; the imports
  print in a separate comment-free program, and the oracle relocates a script
  comment down into the component body — leading the first surviving statement —
  which the carry reproduces). A comment past the last **surviving** statement has
  no statement to lead and falls to the end of the synthetic function body (whose
  block span runs `[content.start, rbrace_end)`, so it is captured exactly once)
  while the oracle re-attaches it into the template — a position difference the bar
  tolerates. The exception is `template_emits_nested_block`: the oracle's printer
  walks one comment index, and opening a block with **no source `loc`** resets it
  to the end, DROPPING every comment not yet written — while opening a block that
  **has** a `loc` re-seeks that index absolutely, which can move it **backward**.
  So a loc-less block annihilates the index and the next loc-bearing one RECOVERS
  it. That recovery, not an exemption, carries the comment through the component
  body: the body block is assigned the instance script's `loc`, and a
  context-wrapped component reassigns the outer block to a fresh loc-less one
  around it, so the wrapper annihilates and the inner block seeks back. A template
  block gets no recovery — so a template emitting a nested block refuses
  (`CommentAfterLastStatementWithBlock`), a blunt "does one exist anywhere" scan
  that deliberately over-refuses the case where a loc-bearing head expression
  flushes the comment first, and likewise the block-free special elements
  (`<svelte:window>`, `<slot>`). The split is keyed to the pinned oracle's
  `reset_comment_index` behavior (esrap 2.2.12) — re-probe it if that pin moves.
  The same index recovery governs a **module-script** comment, which is why one is
  DROPPED rather than carried only when the module script comes FIRST: the component
  body block carries the instance script's `loc`, so opening it seeks forward past a
  comment that precedes the instance script and BACKWARD onto one that follows it, and
  a recovered comment is then flushed into the next loc-bearing node (a template
  expression it has nothing to do with). tsv drops it either way, so the
  module-second ordering refuses (`ModuleCommentAfterInstanceScript`). A second route
  to the same recovery — a block-bearing statement EARLIER in the module body, no
  instance script needed — is a known open mismatch; see
  `../../docs/checklist_svelte_compiler.md` §The open half.
  Divergent placement classes
  also still refuse —
  template-expression comments, comments inside dropped rune regions, and comments
  alongside a rune rewrite that mints a **script-region** span a comment window
  would sweep (`$derived` — the `$.derived(() => e)` thunk — and argument-less
  `$state()`). A template block, a component invocation, an expression-valued
  attribute, `{#snippet}`/`{@render}`, and hoisted imports emit **template-region**
  spans only, so a carried comment window can't reach them and they compile.
- `script_rewrite.rs` — the per-statement rune rewrites
  (`rewrite_script_statement`). Oracle phase 3, **server**: `$props()` →
  `$$props` (span-stolen),
  `$state(v)`/`$state.raw(v)` → `v` (`void 0` argument-less), `$derived(e)` →
  `$.derived(() => e)` — but the oracle's `b.thunk` runs `unthunk`, which
  collapses the arrow when its body is a call on a bare identifier whose
  arguments match its (empty) parameter list, so an argument-less call passes
  straight through (`$derived(get_library())` → `$.derived(get_library)`) —
  `$derived.by(f)` → `$.derived(f)`, statement-position
  `$effect`/`$effect.pre` dropped (forcing the wrapper) — statement-position
  `$inspect(args)` / `$inspect(args).with(cb)` (recognized by
  `analyze.rs::is_inspect_call`) also dropped, but WITHOUT forcing the wrapper
  (no `has_effects`): its arguments and `.with` callback are still guard-walked
  and its span pushed to `dropped_regions` (a comment inside refuses) — a
  `$props.id()` declarator SKIPPED (the transform hoists `const <name> =
  $.props_id($$renderer)` to the component body's first statement, forcing no
  wrapper; duplicate / non-identifier target / carried comment refuse) — a
  `$state.snapshot(x)` declarator UNWRAPPED to its argument `x` (like `$state`;
  both via `classify_rune_init`, which refuses an optional-chained init) — though
  UNLIKE `$state`, the snapshot binding stays UNKNOWN to the static evaluator, so a
  template read never folds (`$.escape(s)`). The unwrap is the emission form, not the
  evaluation form: the oracle evaluates a rune declarator through its argument for
  `$state` / `$state.raw` / `$derived` only, and every other rune — `$state.snapshot`
  included — falls to its `default` arm and yields UNKNOWN
  (`phases/scope.js:469-503`). That holds however the argument itself evaluates — a
  plain `let` argument does not fold either — a
  **top-level class declaration** rewritten by `rewrite_class_state_fields`: each
  DIRECT non-static, non-computed `$state(v)`/`$state.raw(v)` field UNWRAPPED to `v`
  (a no-arg `field = $state()` → a BARE field, value dropped, NOT `void 0` — the
  divergence from the argless declarator), every other member (a `$derived`/static/
  computed rune field, a method body, a nested class/class expression) taking the
  normal refusing guard walk (`walk_class_member_guarded`) so the guard-exempt set
  equals the unwrap set — reach-matched by construction, no undefined-`$state` MISMATCH;
  a field whose WHOLE argument is a LONE reactive-binding identifier
  (`$state($count)` / `$state(d)`) REFUSES (`ClassFieldStateReactiveArg`,
  `is_lone_reactive_binding`) — the oracle keeps that lone store/`$derived` read BARE
  in the field, but the store rewrite descends into class bodies unconditionally and
  would rewrite the kept argument to `$.store_get(…)`/`d()`, so a compound
  (`$state($count + 1)`) or plain-var argument compiles while the lone case is a safe
  over-refusal — a
  multi-declarator top-level declaration
  splitting into one declaration per declarator, source order (the oracle's
  shape; nested declarations and for-heads stay joined; comments alongside a
  multi-declarator refuse — the oracle re-anchors them inside the split). The
  `$props()` pattern itself is rewritten by `script_props.rs`, below.

  One shared helper, `script_walk_ctx`, builds the guard context every walk on
  this path uses: store reads and `$derived` reads are EXEMPTED (the store rewrite
  turns both into `$.store_get(…)` / `d()` after the loop) while both shadow
  refusals are deferred — the store's needs the full nested-scope set, the
  derived's is a whole-compile check in `compile_server`.
- `script_props.rs` — the `$props()` binding-pattern rewrite. Oracle phase 3,
  **server**: a rest element in the pattern gains the oracle's `$$slots,
  $$events` injection immediately before it, and a non-destructured `let props =
  $props()` becomes `let { $$slots, $$events, ...props } = $$props` — a plain
  destructure without a rest gets no injection. A top-level destructure default
  `= $bindable(fallback?)` becomes its fallback (`void 0` argument-less) with the
  bindable prop collected in source order for the trailing `$.bind_props($$props,
  { … })` (shorthand `{ key }` when the key equals its local, else `{ key:
  local }`). A `$bindable` in any UNrecognized shape — a non-identifier key
  (string/numeric/computed), a nested-pattern value, the wrong arity — survives
  the rewrite for the guard walk to refuse, a safe over-refusal even for a
  non-identifier-keyed prop the oracle would compile. When the component
  references `$$slots` the injected sanitize_slots const owns that name, so the
  destructured prop deconflicts by renaming (`$$slots: $$slots_` — the oracle's
  always-`_`-suffix rule; `$$events` never renames).
The **fragment walk and its shared primitives** are five modules. Unlike the
script side, they cannot split along a target-independence line — the whole
emission layer is server codegen — so the organizing principle is **role in the
emission pipeline**: one walk plus four primitives it and every per-node emitter
share. `fragment.rs` is the hub — the only module reaching all four primitives —
and none of the four calls back into it, so the sole remaining cycle is the
tree's own recursion (`emit_fragment` → `emit_element` → `emit_child_body`). The
one other edge inside the group is `dropped.rs` → `special_element_kind.rs` (for
the `SPECIAL_ELEMENT_SLOT` label); the primitives are otherwise mutually
independent. They are listed here in dependency order, walk first.

- `fragment.rs` — the per-fragment walk (`emit_fragment`). Static
  emission implements the oracle's normalization, derived from Svelte's own
  `clean_nodes`/`escape_html` and probe-verified: whitespace-only boundary
  text drops and edge runs trim per fragment; a text edge run abutting a
  non-text node collapses to one space (runs abutting `{expr}` stay — text +
  expression count as one text); interior whitespace is verbatim;
  `<pre>`/`<textarea>` preserve everything; a text-first fragment (component
  root or `{#each}` body — the oracle's `is_text_first` parent set) gets a
  `<!---->` prefix. `{expr}` → `$.escape(expr)` (a derived read, bare or nested,
  becomes `d()`; known evaluations fold as static text), `{@html expr}` →
  `$.html(expr)`; entities decode then re-escape (`[&<]` in text). The rules are
  keyed on a node's *neighbors*, so a second copy at any emitter would see a
  different neighbor list and silently change the rendered output — which is why
  they stay here rather than moving with the per-node emitters. Also
  `emit_child_body` (the block-body seam every control-flow emitter recurses
  through) and the hoisted-`<title>` emitter (`$$renderer.title(…)`, private —
  only `emit_fragment` reaches it).
- `body_builder.rs` — the `BodyBuilder` accumulator: alternating static text and
  interpolation expressions, flushed into a `$$renderer.push(…)` statement. A pure
  leaf: it imports only `build.rs`, and the orchestrator plus every per-node
  emitter imports it, making it the most depended-on module in the emission layer.
  (Deliberately stated as that invariant rather than as a consumer list — an
  enumerated list here went stale at each of the emitter splits, since every new
  emitter module inherits the dependency.) Single home of the oracle's
  `b.block([...state.init, ...build_template(state.template)])` shape — the
  init/template split `mark_init_end` / `push_init_statement` model — so no emitter
  reconstructs that ordering itself.
- `template_value.rs` — the **item-6 template-value substitution walk**
  (`wrap_value_expr` / `wrap_single` over the `rewrite_template_value` core), the
  single home every template value position routes through. It rewrites every read
  of a
  `$derived` binding — bare (`{d}`) or nested at any depth (`{d + 1}`, `{obj[d]}`,
  `{f(d)}`, `{d.x}`) — to `d()`, every `$state.snapshot(x)` sub-node to
  `$.snapshot(<processed x>)`, and every **store read** — a `$name` whose
  `$`-stripped base is a binding and not a rune (`bare_store_read`), NOT shadowed by a
  block-local overlay (a shadowed base is the oracle's
  `store_invalid_scoped_subscription`, left for the guard to refuse) — to
  `$.store_get(($$store_subs ??= {}), '$name', name)` (the store value reads `name()`
  when `name` is a `$derived`, the store the derived currently holds; the
  `var $$store_subs` / `$.unsubscribe_stores` injection is decided upfront by
  `needs_context`'s `uses_stores`, NOT flagged here; a store read in a top-level
  `{#snippet}` also blocks its module-hoist — `snippet.rs`),
  rebuilding only the spine down to each rewrite target
  (a `contains_rewrite_target` fast-path keeps target-free subtrees on the unchanged
  guarded path, byte-identical, and `contains_rewrite_target`/`rebuild_value` stay in
  lockstep on one node set). A derived read or snapshot under a node kind the walk
  does not descend (an object literal, an arrow, a tagged template) or a pattern
  default is left for the guard, which refuses it (a safe over-refusal); a
  **script-position** derived read is instead rewritten to `d()` by `store_rewrite`
  (not refused). Duplicating any of the three rewrites at a borrow point is
  dangerous in both directions — a missed one emits a bare `d` where the oracle
  emits `d()` (a MISMATCH), while one the guard does not know about turns a safe
  over-refusal into silently divergent output.
- `dropped.rs` — the guards for what the SSR output does **not** emit, the inverse
  of every emitter's job: `guard_dropped` (the `{#each}` key, the `{#key}`
  expression, an event handler, everything in a `{:catch}`), `guard_pattern` (a
  pattern emitted verbatim, where the derived rule stays ON), `guard_dropped_fragment`
  (a whole dropped branch — the references walk plus the presence walk
  `guard_dropped_presence` / `dropped_presence_refusal`, whose two-axis membership
  argument is detailed under `attr_refs.rs` above), and `guard_inert_special_element`
  (`<svelte:window>`/`<svelte:body>`/`<svelte:document>` — emitted as NOTHING, yet
  fully phase-2 validated). An emitter never visits a dropped region, so nothing it
  does can refuse what sits there; the oracle decides TypeScript at parse time and
  rune placement at analysis time, both before it chooses what to emit, so a dropped
  region still needs refusal-equivalent walking. The scoping rule — "refuse where
  the construct can affect the result", deliberately narrower than "a fence refuses
  everywhere" — lives in one exhaustive match rather than at each caller, because
  both directions are dangerous: refusing too little is an over-acceptance the
  corpus cannot see, refusing too much turns correct output into refusals for
  nothing.
- `special_element_kind.rs` — the macro-generated special-element
  handled-or-refused table: a label constant per refused kind, the
  `SPECIAL_ELEMENT_REFUSAL_KINDS` list, and the `special_element_refusal_kind`
  mapping, all from one set of rows, plus `SPECIAL_ELEMENT_FENCED_KINDS`. Read by
  four consumers that are asking a question rather than walking — `fragment.rs`'s
  dispatch (which refuses), `refusal_census.rs` (which detects the same shapes as
  co-blockers), `refusal_buckets.rs`'s `is_deliberate_fence`, and `dropped.rs`
  (`SPECIAL_ELEMENT_SLOT`). It is a macro because only
  the mapping is checked by exhaustiveness: a hand-written list beside it keeps
  compiling when a sixth kind appears, silently dropping that kind's key from the
  census's declared buckets and quietly skewing its exposure accounting.

The **block-level emitters** are two modules, riding the walk and primitives
above rather than belonging to them: each owns a family of template constructs
that splits the single template into multiple `$$renderer.push(…)` statements.
Both recurse back into `fragment.rs` through `emit_child_body`.

- `blocks.rs` — **control-flow blocks** split the single template into
  multiple `$$renderer.push(…)` statements, each block emitting its own
  statements between flushes and merging its closer/opener into the adjacent
  template: `{#if}` is a flat `if … else if … else` chain with per-branch
  single-quote-string anchor pushes (`<!--[N-->`, terminal `<!--[-1-->`,
  synthesized when `{:else}` is absent) and a merge-forward `<!--]-->` closer;
  `{#each}` is `const each_array = $.ensure_array_like(expr)` + a `for` loop
  binding `let CTX = each_array[IDX]` (both `each_array`/`$$index` names
  advance once per each block but in **different orders**, so they are allocated by
  different passes: the oracle mints `each_array` in the transform
  (`state.scope.root.unique`, pre-order — so emission order IS its order, and a
  dropped `{:catch}` consumes none), and `$$index` in the **scope-creation** pass,
  *after* recursing into body + fallback — post-order, over dropped regions too. The
  latter is therefore assigned upfront by `assign_each_index_names` and only looked
  up at emission; sharing one emission-order counter mis-numbers every document
  where one `{#each}` contains another or one sits in a `{:catch}`. `$$length` is
  fixed), the opener
  `<!--[-->` merging backward without `{:else}` or, with it, `each_array`
  hoisting before an `if (each_array.length !== 0) { … } else { … }` whose
  openers are string pushes; `{#await}` is a 4-arg
  `$.await($$renderer, expr, () => {pending}, (value?) => {then})` (empty
  `() => {}` fallbacks; `{:catch}` dropped) + a merge-forward closer; `{#key}`
  is a `<!---->` marker, a bare `{ … }` block, and a closing `<!---->` (key
  expression guard-walked then dropped, like an each key);
  **`<svelte:boundary>`** (`emit_boundary`) is an ISOLATED `<!--[-->` push, a bare
  `{ … }` block of children, and an isolated `<!--]-->` push — isolated because a
  fresh `BodyBuilder` flushes before each statement, so unlike `{#key}`'s marker the
  anchors never merge into an adjacent sibling's template. A `failed` snippet moves
  those three statements inside `$$renderer.boundary({ failed }, ($$renderer) => …)`
  with the snippet's `function` declaration emitted just above; a `pending` snippet's
  body REPLACES them under the `<!--[!-->` opener while the children are still
  compiled into a DISCARDED builder — load-bearing, not wasteful, since the oracle
  visits that fragment unconditionally and its `{#each}` consumes an `each_array`
  name. ⚠️ Emission is `failed`-first but VISIT order is children → `pending` →
  `failed`, and the generated names follow the visit order, so building children
  before the snippet functions is what keeps the two straight. The attribute set is
  validated against the oracle's closed `onerror`/`failed`/`pending` list (six
  distinct over-acceptances otherwise); `onerror` drops but is guard-walked, and the
  `failed=`/`pending=` attribute FORMS refuse. ⚠️ Emitting rather than refusing a
  boundary makes three **pre-existing, general** validation over-acceptances
  newly REACHABLE through one — a `<svelte:head>`/`<svelte:options>` inside it
  (`svelte_meta_invalid_placement`), a duplicate `onerror` (`attribute_duplicate`),
  and a duplicate snippet name (`declaration_duplicate`). Each fails identically
  with no boundary in the document, so the fix is the oracle's whole-component
  validations, never a boundary-scoped refusal; tracked in
  `../../docs/checklist_svelte_compiler.md`. `{@const}` hoists a
  `const` declaration to the top of its branch body and enters the evaluator's
  innermost block-scope overlay so later reads fold. Each/await locals and the
  `{:then}` value mask to UNKNOWN in that overlay; a block body that shadows a
  `$derived` name refuses. `<svelte:head>` emits `$.head(hash, $$renderer,
  ($$renderer) => { … })`.
- `snippet_emit.rs` — **snippets/render**: a `{#snippet}` becomes a
  `function name($$renderer, ...params) { … }` — hoisted to true module scope
  (its own program between imports and export) when `snippet.rs` deems it
  hoistable, else to its nearest enclosing block scope's init (a block-scope
  fragment collects the snippets of its whole element subtree and emits them
  first; parameters mask to UNKNOWN). A `{@render callee(args)}` becomes
  `callee($$renderer, ...args)` (`?.` preserved) with a trailing `<!---->` anchor
  unless the enclosing block's sole trimmed child is this render with a
  non-dynamic (local-snippet) callee — the `is_standalone` flag, inherited by
  element children.
The **element emitters** are two modules. `<svelte:element>` deliberately stays
WITH regular elements rather than forming a third: the attribute machinery is
shared between the two hosts "so they never drift", and splitting them apart would
need a third module for the `AttrHost` core and — worse — would *read* as licensing
a fork of those functions per host, which is exactly the drift the design prevents.
If `element.rs` still feels too large, raise it rather than splitting `AttrHost`'s
functions by host.

- `element.rs` — element emission: `emit_element` prints
  static HTML (void elements close `/>`) and routes a component invocation
  (`<Foo … />`) to `component::emit_component` — one dispatch, the only edge
  between the two modules. A regular element carrying a
  `{...spread}` routes its WHOLE attribute set through `emit_spread_attributes`
  → one fused `$.attributes(object, css_hash, classes, styles, flags)` call
  (`<name${$.attributes(…)}>`): `build_element_spread_object` builds the
  source-order object (plain attributes via `attribute::build_spread_object_property`,
  a `bind:` core kind's synthesized `value`/`checked` property at its slot via
  `attribute_bind::build_bind_object_property`, spreads as `...expr`), the scope hash
  rides `css_hash` (the element is scoped when any scoped compound — type/id/
  class/attribute/universal — matches it, a lookup via `EmitEnv::element_scope` into
  the upfront-matched `CssScoping` table), the `class:` directives ride
  `classes` (`attribute_class_style::build_spread_class_object` — identifier keys + shorthand)
  and the `style:` directives ride `styles`
  (`attribute_class_style::build_spread_style_object` — a FLAT object, no `|important`
  partition), `<input>` / a custom element (hyphenated tag or `is`
  attribute) set `flags` (`4` / `2`), and `elide_call_args` applies the oracle's
  `b.call` elision (trailing `void 0` dropped, interior padded). A co-present
  `on:`/`let:`, a `<select>`, or a load-error element refuses; the drop family is
  guarded-and-dropped. The non-spread path
  (`emit_plain_attributes`) pre-scans a
  regular element's `class:` and `style:` directives and defers them to
  `attribute_class_style::emit_class_directives` / `attribute_class_style::emit_style_directives` (each
  fused at its authored-`class`/`style` slot, or after all plain attributes when
  synthetic — the synthetic `class` before the synthetic `style`), and handles a
  `bind:` directive inline at its source slot via `attribute_bind::emit_bind_directive`.
  A **`<svelte:element this={…}>`** compiles to a statement-level
  `$.element($$renderer, TAG, attrsFn?, childrenFn?)` call (`emit_svelte_element`,
  routed from `fragment.rs` like a component): the TAG is the `'div'` literal
  (`this="div"`, parser-collapsed for a mixed value) or the erased/derived-rewritten
  expression (`this={expr}`), and the attributes/children are rendered into
  parameterless closures over the enclosing `$$renderer`. The attribute machinery is
  **shared** with regular elements via an `AttrHost::{Regular, Dynamic}` enum threaded
  through `emit_plain_attributes` / `emit_spread_attributes` /
  `build_element_spread_object` (so the two never drift): passing
  `name = "svelte:element"` makes the name-keyed guards fall through (never void /
  `<select>` / load-error / custom), and the only forks are the `bind:` handling (a
  `<svelte:element>` validates a `bind:this` via `attribute_bind::validate_dynamic_bind`
  and refuses every other bind) and the spread `flags` argument (always absent — a
  dynamic tag is never `<input>`/custom). A `<svelte:element>` in a component with a
  scoping `<style>` is **CSS-scoped** like a regular element: the element census
  holds it as a leaf and owner, a type/universal selector matches it unconditionally,
  and `emit_svelte_element` synthesizes the hash class into its attributes closure
  (`env.special_element_scope`). Its refusals split two ways, and the split is
  permanent-vs-temporary. **Fenced** (runes-only product scope, never to be
  implemented — `Refusal::is_deliberate_fence`, so outside the achievable-parity
  denominator): a `slot="…"` on a `<svelte:element>` component child, the
  special-element half of the named-slot fence, and a legacy `on:`/`let:`.
  **Deferred** (a real gap, safely refused meanwhile): `bind:focused` and the
  `omit_in_ssr` family.

  ⚠️ In `emit_plain_attributes` / `emit_spread_attributes` the `transition:` and
  `animate:` arms are byte-identical twins, but the neighboring **`use:` arm is
  not**: the plain path checks `is_load_error_element` and refuses before guarding,
  while the spread path already did that check once for the whole element, so only
  the guard remains. Do not fold all three into one helper.
- `component.rs` — component invocation: `emit_component` builds the
  `Foo($$renderer, {…props})` call — a plain object literal, or
  `$.spread_props([…])` when a `{...spread}` attribute is present — the implicit
  `children` snippet prop for default-slot content, and named `{#snippet}` children
  as named snippet props (`$$slots: { key: true, … }` alongside). A leaf with zero
  outgoing edges back to element / `<svelte:element>` emission. Single home of the
  props shape: the prop *order* is the oracle's source order across all three kinds,
  so a second builder assembling any one of them separately would reorder the
  object. Also `component_is_standalone_eligible`, which `fragment.rs`'s
  `is_standalone` recomputation reads.

The **attribute emitters** are three modules, split by what an attribute *is*
rather than by where it is emitted (each covers both the inline and spread paths,
so the shared validity forks stay whole). The dependency is one-way, with
`attribute` the base: `attribute_class_style` borrows five value-shaping helpers
from it, `attribute_bind` borrows exactly one of those five (`escape_html_attr`)
and calls `attribute_class_style` not at all, and neither is depended on back.
There is deliberately no `attribute_common.rs` — it would add a module for five
small functions and obscure that both halves genuinely depend on the base.

- `attribute.rs` — plain attribute emission: dynamic and mixed attributes →
  `$.attr(name, expr[, true])` with
  `$.stringify` interpolations (a mixed attribute whose every part folds
  statically emits a *static* attribute instead — attr-escaped `[&"<]`,
  folded value verbatim: no trim, no empty-class drop, boolean attributes
  keep the folded value; single-expression attributes never fold). Static
  text values inline: entities re-escape (`[&"<]` in static attributes);
  boolean attributes emit `name=""`; `class`/`style` values collapse+trim,
  and a string-valued `class` that collapses+trims to empty is dropped
  entirely (static path only — bare `class` keeps `class=""`, empty
  `style`/`id` stay).
  Also `build_attribute_value_expr` — the object-path value builder the element
  `{...spread}` object uses (the oracle's `build_attribute_value`, `is_component`
  false): boolean → `true`, single Text → HTML-escaped literal, single expression
  → the bare erased/wrapped value (`class` wrapped in `$.clsx` per `needs_clsx`),
  mixed → a folded (un-HTML-escaped) literal or `$.stringify` template — sharing
  the fold-or-template loop (`build_mixed_attr_value`) with `emit_mixed_attribute`,
  which alone HTML-escapes and pushes the full-fold static form. The two differ
  only in that escaping, so a second copy of the loop would drift on the fold
  decision — which changes whether an attribute emits as static text or as a
  `$.stringify` template. And
  `build_spread_object_property` — one `key: value` object property from a plain
  attribute (key lowercased, `shorthand` on a same-named identifier value), `None`
  for a dropped attribute (a single-expression event handler — still guarded — and
  `defaultValue`/`defaultChecked`). Hosts the five helpers the class/style half
  shares — `collapse_attr_whitespace`, `preceded_by_quote`, `class_needs_clsx`,
  `is_js_identifier`, and `escape_html_attr`, the attribute-position sibling of
  the fragment walk's text escape (`[&"<]` vs `[&<]` — a `"` is content in text
  and a delimiter here). `escape_html_attr` is the only one `attribute_bind`
  takes as well, and so the only edge from this module to both siblings.
- `attribute_class_style.rs` — the `class:` / `style:` directive builders, on
  both the inline and spread paths. Single home of the class-vs-style asymmetry,
  which is easy to collapse by mistake and wrong in both directions: `class`
  carries the CSS scope hash and `style` never does, `class` wraps a dynamic base
  in `$.clsx` and `style` takes the bare expression, and `|important` partitions
  the INLINE `$.attr_style` argument into a 2-element array while the SPREAD
  `styles` object stays FLAT. `emit_class_directives` — a regular element's
  `class:name={expr}` directives fuse with the authored `class` attribute (or
  the phase-2 synthetic empty `''`) into `$.attr_class(base, css_hash, { name:
  expr, … })` (the oracle's `build_attr_class`): the base is the static value /
  `$.clsx(expr)` / `''`; the scope hash concatenates into a string-literal base
  or rides the 2nd argument; the element is scoped when any scoped compound
  matches it (`EmitEnv::element_scope`, a lookup into the upfront-matched
  `CssScoping` table) — a type/id/attribute selector, not only a class token or
  `class:` name. A mixed-value
  `class="a {b}"` base refuses
  (`ClassDirectiveWithMixedClass`). And `emit_style_directives` — the `style:`
  analog (the oracle's `build_attr_style`): `$.attr_style(base, directives)`, TWO
  arguments (no css-hash — style is never scoped). The base mirrors the class base
  MINUS `$.clsx` (a dynamic `style={expr}` is the bare expression) and MINUS
  scoping; `directives` is a plain object `{ name: value, … }` or, when any
  directive carries `|important`, the 2-element `[ {normal}, {important} ]` array
  (empty `{}` normal object when all are important; source order within each
  group). Keys lowercase unless `--`-prefixed, then bare-identifier-or-quoted;
  values are the expression / a static literal / a shorthand's same-name
  identifier (object-shorthand `{ color }`). A mixed-value `style="a {b}"` base
  refuses (`StyleDirectiveWithMixedStyle`), a mixed directive value
  `style:x="a {b}"` refuses (`StyleDirectiveWithMixedValue`), and any modifier but
  a single `|important` refuses (`StyleDirectiveInvalidModifier`). `element.rs`'s
  attribute loop pre-scans the `class:` and `style:` directives and calls these at
  the authored slot (or after all plain attributes when synthetic). The two
  spread-path builders live here too: `build_spread_class_object` (the `classes`
  argument — identifier keys, case-preserved, with the object-shorthand collapse
  the oracle's `b.init` applies, checked on the RAW directive expression) and
  `build_spread_style_object` (the `styles` argument — a FLAT object, `|important`
  validated but NOT partitioned, reusing `build_style_property`).
- `attribute_bind.rs` — `bind:` resolution and emission, the most self-contained
  of the three (it calls into neither sibling). Single home of the bind validity
  fork `resolve_bind_directive`, which the inline and spread paths both read so
  the two can never drift — a divergence there would emit a `value`/`checked`
  property on one path and refuse on the other for the same authored bind. The
  `OMIT_IN_SSR_BINDS` list is likewise one place: the oracle skips those with an
  early `continue` *before* it visits the target, so a copy that drifted would
  either emit output the oracle omits or refuse a bind it accepts.
  `emit_bind_directive` — a `bind:` **core kind** on a regular element, emitted
  inline at its source slot (delegating to `resolve_bind_directive`, the validity
  fork the spread `build_bind_object_property` shares so the two never drift):
  `bind:this` omits (any variable, any element — no
  `$state` gate), but only for a valid bind target (an Identifier/member chain or a
  `{get, set}` pair); a non-lvalue target (a call/literal/logical) refuses
  (`bind_invalid_expression`). `bind:value` on `<input>` → `$.attr('value', expr)`;
  `bind:checked` on a static
  `<input type="checkbox">` → `$.attr('checked', expr, true)`; `bind:group` on a
  static-`type` `<input>` → a synthesized `$.attr('checked', <synth>, true)` where
  `<synth>` is `group.includes(<value>)` (checkbox) / `group === <value>`
  (radio/other), `<value>` the companion `value` attribute's value (still emitted at
  its own slot; no companion → the oracle silently drops the bind). The bind TARGET
  is gated to a `$state`-rooted `Identifier`/member chain (the crate's one supported
  bindable — the SAFE side of the oracle's assignable-lvalue rule); every other
  `bind:` (non-`<input>` target, `value` on `<textarea>`/`<select>`, `omit_in_ssr`
  media/dimension binds, `bind:open`, the content-editable trio, an invalid
  target/type, a non-`$state` target) refuses via `Refusal::BindDirective { name }`.
  Its spread-path counterpart is `build_bind_object_property` (a `bind:` core
  kind's `value`/`checked` property via the shared `resolve_bind_directive` —
  `bind:this`/a no-companion `bind:group` yield `None`, and an `omit_in_ssr` bind
  **refuses** on both the spread and inline paths, a safe over-refusal). Also
  `validate_inert_bind_target`, the fork `dropped.rs` reuses for an SSR-inert
  special element's bind. ⚠️ `build_companion_value` reimplements a subset of
  `attribute.rs`'s `build_attribute_value_expr` **deliberately** (no fold, no
  mixed) — deduping the two is a behavior change, not cleanup.

The **CSS scoping** pair is two modules, layered one-way: `element_census.rs`
builds the census and depends on nothing else in the crate, and `css_scope.rs`
matches selector chains against it (both the navigation entry points and the
`CensusNode` leaf type). Nothing points back — keeping the census a pure
structural product of the fragment tree, independent of what any selector asks of
it. Both run **upfront** in `analyze()`,
before any emission decision, so emission only ever performs a span lookup
(`EmitEnv::element_scope`) rather than matching. ⚠️ `element_census.rs` is
unrelated to `refusal_census.rs` despite the name — that one enumerates refusal
classes, this one enumerates scoping candidates.

- `element_census.rs` — the **upfront element census** (`ElementCensus`): one
  top-down walk over `root.fragment`, run in `analyze()`, producing a
  `CensusElement` per scoping candidate — a regular HTML element or a
  `<svelte:element>` (components excluded, matching the oracle's element list, which
  holds `RegularElement`/`SvelteElement`) — with an ancestor/sibling `path`, the
  upward navigability the Svelte AST lacks, and the substrate the combinator matcher
  navigates (`get_ancestor_elements` for descendant/child,
  `get_possible_element_siblings` / `get_possible_nested_siblings` / `loop_child` for
  `+`/`~`, with block-descent and the `{#each}` self-adjacency wrap-around). Each
  candidate is a `CensusNode { Regular(&Element), Dynamic(&SpecialElement) }`
  projecting both element types onto one leaf test; a `<svelte:element>` differs only
  in that a type selector matches it unconditionally (its runtime tag is unknown) and,
  as a possible sibling, it only PROBABLY exists (so it never triggers the `+`
  adjacent early-stop and carries no slot check — `css-prune.js:1041`/`1215`).
  Descends every SSR-reachable fragment (element/component/`<svelte:element>`
  subtrees, `{#if}` / `{#each}` / `{#await}`-pending+then / `{#key}` / `{#snippet}`
  bodies, `<svelte:head>`) but **not** `{:catch}` (dropped from output). The one
  deliberate exception is `<svelte:boundary>`, descended UNCONDITIONALLY — including
  the children a `pending` snippet discards: the oracle's CSS pass runs before it
  decides what to emit, so a selector matching only dropped boundary content is still
  KEPT and still scoped. Safe because `element_scope` is a span lookup at emission, so
  a marked-but-unemitted element contributes nothing. Everywhere else the census leaf
  set equals the emitted set — keeping the single-compound match byte-identical to
  the pre-census emission-fused result. A boundary OWNER is transparent to the
  ancestor walk and opaque to the upward sibling walk (`Owner::Boundary`, exactly
  `Owner::Head`'s pair of answers — the oracle's `is_block` set holds neither), so
  `div > p` across a boundary matches while `b + p` across one does not.
- `css_scope.rs` — CSS scoping: parses a rule's selector into a CHAIN of compounds
  (type / id / class / attribute / universal + trailing pseudo, joined by
  combinators), then matches the chain BACKWARD against the element census
  (`match_scope` → `apply_selector` / `apply_combinator`, a port of the oracle's
  `css-prune.js`; the leaf reuses the joint-AND predicate list —
  `relative_selector_might_apply_to_node` / `attribute_matches` — over a `CensusNode`,
  so a type selector matches a `<svelte:element>` unconditionally while id/class/
  attribute selectors route through its real attribute list). Every compound a
  match reaches gains the `svelte-tsvhash` class and every element the match touches
  is scoped (`CssScoping.scoped_elements`, read by `EmitEnv::element_scope` /
  `EmitEnv::special_element_scope`); the
  compound is **source-spliced** (appended after the last non-pseudo anchor, or
  replacing a bare `*`) — author whitespace preserved, not reprinted — with a
  per-`ComplexSelector` specificity bump (the first scoped compound a plain
  `.svelte-tsvhash`, each later one a zero-specificity `:where(.svelte-tsvhash)`,
  reset per comma `ComplexSelector`). **Supported**: the four combinators
  (descendant / child / `+` / `~`, including block-descent and the `{#each}`
  wrap-around) and basic `:global` (leading `:global(<compound>) .y`, trailing
  `:global(<compound>)`, a fully-global `:global(<compound>)`, and the bare
  `:global` combinator `div :global.x` → `div.x`). **Refused**: `:global{}` global
  blocks (nested rules), `:is`/`:where`/`:has`/`:not`, `:root`/`:host`, nesting, the
  `||` column combinator, a snippet/render-crossing combinator path (`CssCombinatorSelector`
  — the site-resolution product isn't built, a safe over-refusal), at-rules /
  `@keyframes` (`CssAtRule`), empty rules (`CssEmptyRule`), an enumerable dynamic
  attribute value (`CssDynamicAttributeMatch`), a non-ASCII case-insensitive operand
  (`CssCaseInsensitiveNonAscii`), and a chain matching no element
  (`CssSelectorNoMatch`).

Types: `CompileOptions { generate: Generate, dev: bool }` (default: `Server`,
non-dev), `CompileOutput { js, css, warnings }`, `CompileWarning { code, message }`
(minimal for now), and the two error enums (`CompileError`'s two bug variants —
`CorruptOutput` and `TypeErasureLeak` — are the compiler's two self-checks firing).

## The Canonicalizer Contract

`canonicalize_js` parses JavaScript as a strict module (`tsv_ts::Goal::Module`)
and reprints it through `tsv_ts::format_canonical`, which erases newline-derived
*authoring intent*:

- **blank lines are dropped** between statements;
- **expansion heuristics are off** — a construct that fits the print width
  collapses to one line whether or not the source had a newline after its opening
  delimiter; it breaks only when width forces it;
- **comments are preserved** in content and relative order, never dropped or
  merged; only their placement is normalized deterministically (an own-line
  comment may become a trailing comment of the preceding node). A construct
  carrying a `//` line comment before more content stays broken — trailing the
  comment onto a continuing line would swallow that content (inside a template
  interpolation it even makes the output unparseable), so comment presence
  overrides collapse there.

Two guarantees follow. **Idempotence**: canonicalizing an already-canonical string
reproduces it. **Authoring-independence**: two programs that differ only in
incidental whitespace reprint to the same string. Together these make a byte
difference between two canonical forms either a genuine code difference or a comment
*position* difference — the substrate the parity bar (below) refines.

The output is self-validated: `canonicalize_js` reparses its own reprint before
returning and surfaces a rejection as `CanonicalizeError::CorruptOutput` — a
canonicalizer bug is loud, never a silently corrupt comparison string.

Real content is *not* intent and survives verbatim: a newline inside a template
literal, a multi-line string via line continuation, and a mapped type's source
multi-line-ness (a deliberate un-erased residual — see the `format_canonical` seam
notes in `tsv_ts`).

## The Parity Bar (comment-position-tolerant)

The compiler is measured against Svelte's `compile()` on the *canonical reprint* of
both sides, but the bar is **not raw byte-equality** — it tolerates a comment
**position** difference (`parity::compare_canonical` → `Parity::{Exact,
CommentPosition, Divergent}`, re-exported as `compare_canonical`/`Parity`). Two
canonical forms count as parity when they differ ONLY in where comments sit — **same
code, same comment sequence, no bundler annotation involved**. Everything else (a
code difference, a dropped / doubled / reordered / content-changed comment) stays
`Divergent` = a MISMATCH = a bug.

Why: tsv preserves the author's comment placement (its comment philosophy — a
deliberate, cataloged divergence from prettier), while Svelte's printer (esrap)
relocates comments across operator/conditional boundaries the way prettier does. The
two then place the *same* comment on *different* AST nodes — genuinely different
bytes, but not a difference in the compiled **code**. Comment position in
machine-consumed compiled output carries no correctness signal, so pinning it would
flag cosmetic differences as bugs (and force refusing every comment tsv places its
own way). The relaxation aligns the bar with what matters — code + comment presence
+ semantic-comment binding.

The comparison (only on the byte-inequality failure path — the common case stays a
fast `==`):

1. **Same code** — clear `program.comments` on both parses and byte-compare the
   comment-free reprints (a comment-forced break vanishes with its comment, so
   same-code programs reprint identically). Soundness reduces to canonicalizer
   injectivity-on-code, which `canonicalize:audit` gates independently.
2. **Same comments** — the comment *sequence* (output order, exact content) must
   match, so a drop / double-print / reorder / content change is `Divergent`.
3. **Annotation guard** — a bundler annotation (`/* @__PURE__ */`, `@__NO_SIDE_EFFECTS__`,
   webpack/vite magic comments) is NOT position-neutral (moving it changes
   tree-shaking), so its presence falls back to strict byte-equality. JSDoc casts
   are safe — erasure unwraps every `JsdocCast` to a plain comment.

The relaxation is confined to the JS leg: CSS parity stays byte-exact, and the
fixture validator's oracle-freshness / expected-idempotence checks stay strict (a
comment-position-divergent fixture records the *oracle's* placement in
`expected_server.js`; ours is tolerated). The corpus runner surfaces the tolerated
count in a separate `comment_position` bucket so the tolerance is never silent.

## Tests

`src/tests/` is one file per feature (the module map above is the naming guide);
a new test goes in the file matching the feature it exercises, and reaches for an
existing helper in `src/tests/support.rs` rather than minting a new assertion
idiom. There is one helper per intent:

- `compile_js` / `compile_css` — the component compiles. Both route through one
  `compile_checked` seam asserting the whole acceptance contract (server output
  **and** a canonicalize fixed point), so a test reading only the CSS still pins
  the JS side; a bare `let _ = compile_js(src)` is the "this shape compiles" pin.
- `assert_unsupported(source, reason)` — refuses, pinning WHICH refusal via a
  substring of `Refusal`'s `Display`.
- `assert_parse_rejected` — fails at the parse stage.
- `assert_idempotent` / `assert_comments_lossless` — the canonicalizer's own
  contract, independent of compilation.

A refusal test always pins its reason. A bucket-agnostic "refuses somehow"
assertion passes when the right shape refuses for the *wrong* reason — the
general-rule-masking-a-specific-one failure mode — and it silently merges
distinct refusals that happen to share a callsite. Derive the reason from the
refusal the shape actually produces, never from the test's name.

## See Also

- Root [`../../CLAUDE.md`](../../CLAUDE.md) — build, test, and workflow commands
- `compile_fuzz` (in `tsv_debug`) — the differential fuzzer over this crate: it generates
  feature CROSS-PRODUCTS and grades each against the oracle. It exists because the corpus
  runner tests real components, so it exercises every feature while missing nearly every
  feature *pair* — and the refusal sub-group this crate's port makes structurally fragile
  (`GeneratedNameCollision`, `MemberCallAmbiguousRoot`, `DerivedReadShadowed`,
  `SnippetHoistAmbiguous`, `BlockScopeShadowsDerived`, `StoreScopedSubscription`) is
  inherently two-name: the port is **name-based where the oracle is scope-sensitive**. See
  [../../docs/compile_tooling.md](../../docs/compile_tooling.md).
- `tsv_ts` `format_canonical` — the intent-erased reprint entry point this crate drives
