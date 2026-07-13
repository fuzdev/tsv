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
    `{#if}`/`{#each}`/`{#await}`/`{#key}` and `{@const}` are covered (see the
    transform_server block emitters below). Shapes the transform does not cover
    yet — client generation, dev mode, components (`<Foo>` / `<Foo.Bar>`, which
    the oracle emits as `Foo($$renderer, {})` calls), instance-script exports in
    every form (the oracle compiles `export const`/`function`/`{a}` via
    `$.bind_props`, not implemented; rejects `export default`/`export let`),
    non-JS instance scripts — `generics`, or a `lang` other than `"js"`/`""`
    (type stripping not implemented; `lang="js"`/`lang=""` compile as plain JS),
    `{#snippet}`/`{@render}`/`{@debug}`, directives/spread, top-level `await`,
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
  - `canonicalize_js(source) -> Result<String, CanonicalizeError>` — the
    canonicalizer (below). Lives here because the compiler's own output
    idempotence checks and the oracle comparison both consume it.
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
  through to their argument; shadowed names go `Opaque` (refuse-on-spine).
- `rune_guard.rs` — the rune refusal walk plus the collection passes riding the
  same exhaustive traversal: refuses any `$`-prefixed identifier reference or
  `$`-rooted call outside the sanctioned rewrites, refuses derived-binding
  reads outside bare emitter positions and top-level `await`, and collects
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
  in a nested scope is ambiguous for this name-based port and refuses.
- `transform_server.rs` — the SSR transform: module scaffold
  (`import * as $ from 'svelte/internal/server'`, then any instance-script
  `import` declarations hoisted to module scope in source order — an import
  inside the component function is invalid JS — + the exported component
  function), instance-script statements borrowed with rune rewrites —
  `$props()` → `$$props` (span-stolen; a rest element in its pattern gains the
  oracle's `$$slots, $$events` injection immediately before it, and a
  non-destructured `let props = $props()` becomes
  `let { $$slots, $$events, ...props } = $$props` — a plain destructure without
  a rest gets no injection), `$state(v)`/`$state.raw(v)` → `v`
  (`void 0` argument-less), `$derived(e)` → `$.derived(() => e)`,
  `$derived.by(f)` → `$.derived(f)`, statement-position `$effect`/`$effect.pre`
  dropped — a multi-declarator top-level declaration splitting into one
  declaration per declarator, source order (the oracle's shape; nested
  declarations and for-heads stay joined; comments alongside a multi-declarator
  refuse — the oracle re-anchors them inside the split) — the whole body
  wrapping in
  `$$renderer.component(($$renderer) => { … })` whenever `needs_context` fires (a
  dropped effect, or the new/member/call analysis above), which also forces the
  `$$props` parameter — the template folded into one `$$renderer.push(\`…\`)` with
  `{expr}` → `$.escape(expr)` (a bare derived read becomes `d()`; known
  evaluations fold as static text), `{@html expr}` → `$.html(expr)`, dynamic
  and mixed attributes → `$.attr(name, expr[, true])` / `$.attr_class` /
  `$.attr_style` with `$.stringify` interpolations, and minimal CSS scoping
  (single class selectors: the `svelte-tsvhash` class appended to matched
  elements and **source-spliced** into the style text — the author's
  whitespace is preserved, not reprinted). Static emission implements the
  oracle's normalization, derived from Svelte's own `clean_nodes`/`escape_html`
  and probe-verified: whitespace-only boundary text drops and edge runs trim
  per fragment; a text edge run abutting a non-text node collapses to one
  space (runs abutting `{expr}` stay — text + expression count as one text);
  interior whitespace is verbatim; `<pre>`/`<textarea>` preserve everything;
  entities decode then re-escape (`[&<]` in text, `[&"<]` in static
  attributes); boolean attributes emit `name=""`; `class`/`style` values
  collapse+trim; void elements close `/>`; a text-first fragment (component
  root or `{#each}` body — the oracle's `is_text_first` parent set) gets a
  `<!---->` prefix. **Control-flow blocks** split the single template into
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
  `$derived` name refuses. Instance-script comments carry through into the
  synthetic program (host-absolute spans; the import prints as a separate
  comment-free program so no window bridges a low anchor to its appendix
  source literal); divergent placement classes refuse — comments after the
  last script statement (the oracle re-attaches them into the template),
  template-expression comments, comments inside dropped rune regions, and
  comments alongside `$derived` / argument-less `$state()` /
  expression-valued attributes (window-sweep hazards).

Types: `CompileOptions { generate: Generate, dev: bool }` (default: `Server`,
non-dev), `CompileOutput { js, css, warnings }`, `CompileWarning { code, message }`
(minimal for now), and the two error enums.

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
