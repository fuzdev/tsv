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
    object-shorthand collapse — `attribute::build_spread_class_object`), the
    `style:` directives ride the `styles` argument (a **FLAT** object, **no**
    `|important` partitioning — the divergence from the non-spread
    `$.attr_style` array — `attribute::build_spread_style_object`), and `<input>`
    / a custom element set the `flags` argument
    (trailing absent args elide, interior ones become `void 0`); a spread
    co-present with a legacy `on:`/`let:` refuses (`Refusal::NonPlainAttribute`),
    and a spread on a `<select>` (the `$$renderer.select`
    trap) or on a load-error element refuses (`SpreadOnSelect` /
    `SpreadOnLoadErrorElement`) — a `class:` / `style:`
    directive on a **regular element without a spread** is
    instead **emitted** as the fused `$.attr_class(base, hash, {…})` /
    `$.attr_style(base, {…})` call (`element.rs` /
    `attribute.rs`), and a `bind:` **core kind** on a regular element without a
    spread is
    **handled** by `attribute::emit_bind_directive` (`bind:this` omits;
    `bind:value`/`bind:checked`/`bind:group` on `<input>` synthesize a
    `$.attr(...)` for a `$state`-rooted target; every other `bind:` refuses via
    `Refusal::BindDirective { name }`) — both the inline and spread `bind:` paths
    share one `attribute::resolve_bind_directive` validity fork; the no-op drop
    family (`use:`/`transition:`/`in:`/`out:`/
    `animate:`/`{@attach}`) is instead **dropped** on a regular element, its
    expression still guarded (a stray rune / `await` refuses) and still walked for
    scope analysis, except a `use:` on a load-error element, which refuses because
    the oracle adds `onload`/`onerror` capture attributes there —
    top-level `await`,
    `<option>` / populated `<select>`/`<optgroup>` (the oracle emits closure
    calls / `<!>` anchors there), template-expression comments, and every
    `$`-prefixed identifier reference or call outside the sanctioned rewrites
    below — return `CompileError::Unsupported` with a clear description, never
    guessed output. Within the supported blocks, nested `{#each}` (unreproducible
    unique-name order), a root-level `{@const}`, a destructured `{@const}`, a
    `{@const}` shadowing a `$derived` binding, a member/call rooted at a
    prop/import that is also shadowed in a nested scope (`needs_context`
    classification ambiguous), a leading comment glued to the `<script>` line,
    and any block alongside carried script comments also refuse.
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
  quotes) and a stable `bucket_key` the corpus runner groups by directly
  (user-chosen names collapse to a `{placeholder}` so e.g. every event
  attribute shares one bucket). The single source of truth for the refusal
  contract.
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
  declarator inits, and a template-position `$state.snapshot(x)` (→ `$.snapshot`,
  `fragment.rs`), so the guard exempts those positions while still refusing every
  other `$bindable`/`$inspect`/`$state.snapshot`/`$props.id` (value/template
  positions, nested defaults, a wrong-arity or second `.with`, `$inspect.trace`, a
  nested-scope / script-position / optional-chained rune, …) — refuses a derived-binding
  read the template value-walk does not rewrite to `d()` (a pattern default, a
  script-position read, a read under an unsupported wrapper, or an escaped-identifier
  read whose decoded name is a `$derived` binding) and top-level `await`, and collects
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
  there still fires the flag) and `{@render}` arguments.
- `snippet.rs` — the `{#snippet}` hoist analysis (name-based port of Svelte's
  `can_hoist_snippet`): which top-level snippets go to true module scope. Collects
  each snippet's free references (a flat scope-tracking walk) minus its bound
  names; a free reference to an instance binding (prop/`$state`/`$derived`/plain
  top-level decl — *not* imports/globals) blocks hoisting, and a name that is both
  an instance binding and a nested local is ambiguous and refuses. Hoistability is
  a fixpoint over snippet-to-snippet references. Also collects every snippet name
  (render-callee classification, generated-name collisions).
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
    `fragment_has_*` refusal predicates ride it through the `fragment_any`
    combinator — each supplying only its own narrow per-node test — and the
    snippet-name collector recurses through it, so the recursion shape has a single
    home. A new `FragmentNode` variant, or a new child fragment on an existing
    variant, fails compilation HERE rather than drifting across the copies (which
    is how `fragment_contains_block` came to skip `SpecialElement` while its
    siblings recursed; unifying it onto the seam realigns that descent). The
    scope-tracking / dropped-`{:catch}` walks (`needs_context.rs`, `snippet.rs`'s
    free-variable collector) keep their own exhaustive matches on purpose: their
    descent is entangled with per-node scope binding, the emission-vs-dropped
    distinction, and the `{#await}`-catch flag toggle, which a uniform enumeration
    can't carry without changing behavior.
  The SSR output **drops** four regions without visiting them — the `{#each}`
  key, the `{#key}` expression, an event-handler attribute, and the whole
  `{:catch}` branch — so no emission refusal can fire inside them. But the oracle
  decides TypeScript at *parse* time and rune placement at *analysis* time, both
  before it chooses what to emit, and it counts references wherever they sit. So
  a dropped region still gets all three walks (`transform_server`'s
  `refuse_template_typescript` / `guard_dropped_fragment`, and the analyses'
  dropped-fragment view) — but **not** the emission refusals, and not the
  derived-read rule, which is an emission rewrite rather than a validity rule.
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
  `needs_context`).
- `script_rewrite.rs` — the document-wide TypeScript flag and gate
  (`document_ts_flag`/`refuse_template_typescript`), the top-level
  binding-table analysis (`analyze_script`/`analyze_declarator`), and the
  per-statement rune rewrites (`rewrite_script_statement`) — `$props()` →
  `$$props` (span-stolen; a rest element in its pattern gains the oracle's
  `$$slots, $$events` injection immediately before it, and a non-destructured
  `let props = $props()` becomes `let { $$slots, $$events, ...props } =
  $$props` — a plain destructure without a rest gets no injection), a
  top-level `$props()` destructure default `= $bindable(fallback?)` → its
  fallback (`void 0` argument-less) with the bindable prop collected in source
  order for the trailing `$.bind_props($$props, { … })` (shorthand `{ key }`
  when the key equals its local, else `{ key: local }`),
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
  both via `classify_rune_init`, which refuses an optional-chained init) — a
  multi-declarator top-level declaration
  splitting into one declaration per declarator, source order (the oracle's
  shape; nested declarations and for-heads stay joined; comments alongside a
  multi-declarator refuse — the oracle re-anchors them inside the split). Also
  `collect_script_comments`: instance-script comments carry through into the
  synthetic program (host-absolute spans; the import prints as a separate
  comment-free program so no window bridges a low anchor to its appendix
  source literal); divergent placement classes refuse — comments after the
  last script statement (the oracle re-attaches them into the template),
  template-expression comments, comments inside dropped rune regions, and
  comments alongside `$derived` / argument-less `$state()` /
  expression-valued attributes (window-sweep hazards) — and
  `self_check_no_typescript`, the type-erasure self-check that closes the
  loop on the finished program (see `erase.rs`).
- `fragment.rs` — the per-fragment walk (`emit_fragment`) and its
  `BodyBuilder` accumulator (alternating static text and interpolation
  expressions, flushed into a `$$renderer.push(…)` statement). Static
  emission implements the oracle's normalization, derived from Svelte's own
  `clean_nodes`/`escape_html` and probe-verified: whitespace-only boundary
  text drops and edge runs trim per fragment; a text edge run abutting a
  non-text node collapses to one space (runs abutting `{expr}` stay — text +
  expression count as one text); interior whitespace is verbatim;
  `<pre>`/`<textarea>` preserve everything; a text-first fragment (component
  root or `{#each}` body — the oracle's `is_text_first` parent set) gets a
  `<!---->` prefix. `{expr}` → `$.escape(expr)` (a derived read, bare or nested,
  becomes `d()`; known evaluations fold as static text), `{@html expr}` →
  `$.html(expr)`; entities decode then re-escape (`[&<]` in text). The
  `guard_dropped`/`guard_pattern`/`guard_dropped_fragment`/`wrap_single`/
  `wrap_value_expr` family prepares a borrowed template expression for a
  synthetic call argument slot, guarding stray runes and rewriting a derived
  read (bare or nested) to `d()`. `wrap_value_expr`'s core `rewrite_template_value`
  is the **item-6 template-value substitution walk**: it rewrites every read of a
  `$derived` binding — bare (`{d}`) or nested at any depth (`{d + 1}`, `{obj[d]}`,
  `{f(d)}`, `{d.x}`) — to `d()`, and every `$state.snapshot(x)` sub-node to
  `$.snapshot(<processed x>)`, rebuilding only the spine down to each rewrite target
  (a `contains_rewrite_target` fast-path keeps target-free subtrees on the unchanged
  guarded path, byte-identical, and `contains_rewrite_target`/`rebuild_value` stay in
  lockstep on one node set). A derived read or snapshot under a node kind the walk
  does not descend (an object literal, an arrow, a tagged template), a pattern
  default, or a script-position read is left for the guard, which refuses it (a safe
  over-refusal).
- `blocks.rs` — **control-flow blocks** split the single template into
  multiple `$$renderer.push(…)` statements, each block emitting its own
  statements between flushes and merging its closer/opener into the adjacent
  template: `{#if}` is a flat `if … else if … else` chain with per-branch
  single-quote-string anchor pushes (`<!--[N-->`, terminal `<!--[-1-->`,
  synthesized when `{:else}` is absent) and a merge-forward `<!--]-->` closer;
  `{#each}` is `const each_array = $.ensure_array_like(expr)` + a `for` loop
  binding `let CTX = each_array[IDX]` (both `each_array`/`$$index` names
  advance once per each block in source order, `$$length` fixed), the opener
  `<!--[-->` merging backward without `{:else}` or, with it, `each_array`
  hoisting before an `if (each_array.length !== 0) { … } else { … }` whose
  openers are string pushes; `{#await}` is a 4-arg
  `$.await($$renderer, expr, () => {pending}, (value?) => {then})` (empty
  `() => {}` fallbacks; `{:catch}` dropped) + a merge-forward closer; `{#key}`
  is a `<!---->` marker, a bare `{ … }` block, and a closing `<!---->` (key
  expression guard-walked then dropped, like an each key); `{@const}` hoists a
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
- `element.rs` — element and component emission: `emit_element` prints
  static HTML (void elements close `/>`) and routes a component invocation
  (`<Foo … />`) to `emit_component`, which builds the `Foo($$renderer,
  {…props})` call — a plain object literal, or `$.spread_props([…])` when a
  `{...spread}` attribute is present — the implicit `children` snippet prop
  for default-slot content, and named `{#snippet}` children as named snippet
  props (`$$slots: { key: true, … }` alongside). A regular element carrying a
  `{...spread}` routes its WHOLE attribute set through `emit_spread_attributes`
  → one fused `$.attributes(object, css_hash, classes, styles, flags)` call
  (`<name${$.attributes(…)}>`): `build_element_spread_object` builds the
  source-order object (plain attributes via `attribute::build_spread_object_property`,
  a `bind:` core kind's synthesized `value`/`checked` property at its slot via
  `attribute::build_bind_object_property`, spreads as `...expr`), the scope hash
  rides `css_hash` (the element is scoped when any scoped compound — type/id/
  class/attribute/universal — matches it, a lookup via `EmitEnv::element_scope` into
  the upfront-matched `CssScoping` table), the `class:` directives ride
  `classes` (`attribute::build_spread_class_object` — identifier keys + shorthand)
  and the `style:` directives ride `styles`
  (`attribute::build_spread_style_object` — a FLAT object, no `|important`
  partition), `<input>` / a custom element (hyphenated tag or `is`
  attribute) set `flags` (`4` / `2`), and `elide_call_args` applies the oracle's
  `b.call` elision (trailing `void 0` dropped, interior padded). A co-present
  `on:`/`let:`, a `<select>`, or a load-error element refuses; the drop family is
  guarded-and-dropped. The non-spread path
  (`emit_plain_attributes`) pre-scans a
  regular element's `class:` and `style:` directives and defers them to
  `attribute::emit_class_directives` / `attribute::emit_style_directives` (each
  fused at its authored-`class`/`style` slot, or after all plain attributes when
  synthetic — the synthetic `class` before the synthetic `style`), and handles a
  `bind:` directive inline at its source slot via `attribute::emit_bind_directive`.
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
  `<svelte:element>` validates a `bind:this` via `attribute::validate_dynamic_bind`
  and refuses every other bind) and the spread `flags` argument (always absent — a
  dynamic tag is never `<input>`/custom). A `<svelte:element>` in a component with a
  scoping `<style>` is **CSS-scoped** like a regular element: the element census
  holds it as a leaf and owner, a type/universal selector matches it unconditionally,
  and `emit_svelte_element` synthesizes the hash class into its attributes closure
  (`env.special_element_scope`). Deferred as safe refusals: a `slot="…"` on a
  `<svelte:element>` component child (would MISROUTE the `FragmentNode::Element`-only
  named-slot detection), a legacy `on:`/`let:` (the runes-only fence), and
  `bind:focused`/the `omit_in_ssr` family.
- `attribute.rs` — attribute emission: dynamic and mixed attributes →
  `$.attr(name, expr[, true])` / `$.attr_class` / `$.attr_style` with
  `$.stringify` interpolations (a mixed attribute whose every part folds
  statically emits a *static* attribute instead — attr-escaped `[&"<]`,
  folded value verbatim: no trim, no empty-class drop, boolean attributes
  keep the folded value; single-expression attributes never fold). Static
  text values inline: entities re-escape (`[&"<]` in static attributes);
  boolean attributes emit `name=""`; `class`/`style` values collapse+trim,
  and a string-valued `class` that collapses+trims to empty is dropped
  entirely (static path only — bare `class` keeps `class=""`, empty
  `style`/`id` stay). Also `emit_class_directives` — a regular element's
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
  the authored slot (or after all plain attributes when synthetic). Also
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
  Also `build_attribute_value_expr` — the object-path value builder the element
  `{...spread}` object uses (the oracle's `build_attribute_value`, `is_component`
  false): boolean → `true`, single Text → HTML-escaped literal, single expression
  → the bare erased/wrapped value (`class` wrapped in `$.clsx` per `needs_clsx`),
  mixed → a folded (un-HTML-escaped) literal or `$.stringify` template — sharing
  the fold-or-template loop (`build_mixed_attr_value`) with `emit_mixed_attribute`,
  which alone HTML-escapes and pushes the full-fold static form. And
  `build_spread_object_property` — one `key: value` object property from a plain
  attribute (key lowercased, `shorthand` on a same-named identifier value), `None`
  for a dropped attribute (a single-expression event handler — still guarded — and
  `defaultValue`/`defaultChecked`). And the three spread-directive builders:
  `build_bind_object_property` (a `bind:` core kind's `value`/`checked` property via
  the shared `resolve_bind_directive` — `bind:this`/a no-companion `bind:group` yield
  `None`, and an `omit_in_ssr` bind **refuses** on both the spread and inline paths, a
  safe over-refusal), `build_spread_class_object` (the `classes` argument —
  identifier keys, case-preserved, with the object-shorthand collapse the oracle's
  `b.init` applies, checked on the RAW directive expression), and
  `build_spread_style_object` (the `styles` argument — a FLAT object, `|important`
  validated but NOT partitioned, reusing `build_style_property`).
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
  bodies, `<svelte:head>`) but **not** `{:catch}` (dropped from output), so the census
  leaf set equals the emitted set — keeping the single-compound match byte-identical to
  the pre-census emission-fused result.
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
difference between two canonical forms a genuine code difference — the parity bar
for oracle comparison.

The output is self-validated: `canonicalize_js` reparses its own reprint before
returning and surfaces a rejection as `CanonicalizeError::CorruptOutput` — a
canonicalizer bug is loud, never a silently corrupt comparison string.

Real content is *not* intent and survives verbatim: a newline inside a template
literal, a multi-line string via line continuation, and a mapped type's source
multi-line-ness (a deliberate un-erased residual — see the `format_canonical` seam
notes in `tsv_ts`).

## See Also

- Root [`../../CLAUDE.md`](../../CLAUDE.md) — build, test, and workflow commands
- `tsv_ts` `format_canonical` — the intent-erased reprint entry point this crate drives
