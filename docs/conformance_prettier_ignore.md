# Prettier Conformance: format-ignore / prettier-ignore

The freeze directives are one rule applied across all three languages, so they carry one
doc rather than a share of each language's. The terminology, the `◆reason` tags, and the
decision framework live in [conformance_prettier.md](./conformance_prettier.md).

## Format-ignore directive

A comment can suppress formatting of the construct that follows it. tsv honors its own tool-neutral `format-ignore` family — `<!-- format-ignore -->`, `// format-ignore`, `/* format-ignore */`, and the range markers `format-ignore-start` / `format-ignore-end` — **in addition to** prettier's `prettier-ignore` family, which tsv keeps for compatibility with prettier-authored code (corpus files use it). Recognition is centralized in `tsv_lang::is_format_ignore_directive` and the two range predicates, shared across the TypeScript, CSS, and Svelte printers.

For a whole-construct freeze the `prettier-ignore` family matches prettier (both emit the construct raw), so those need no divergence fixture of their own; the type-member *list* positions are where tsv follows its own rule (cataloged in **On type-member lists** below, where tsv freezes the first member of an intersection rather than the whole node, preserves the directive's authored position, holds the list's per-line layout, or stays inert to a trailing directive). The `format-ignore` family is tsv-native: prettier doesn't recognize it, so prettier reformats the construct while tsv preserves it — that difference is the divergence. Most fixtures pair the spellings in one input: a `prettier-ignore`d construct (preserved by both tools, so unchanged in `output_prettier`) sits beside a `format-ignore`d one (reformatted only by prettier), making the `format-ignore` construct the sole divergence and doubling as a prettier-compatibility check. The `basic` (template node) and `js_css` (embedded `<script>` + `<style>`) Svelte fixtures carry this control, as do both standalone fixtures.

- `format-ignore` in `<script>` / `<style>` — ◆design_choice — [js_css](../tests/fixtures/svelte/syntax/format_ignore/js_css_prettier_divergence/)
- `format-ignore` template element — ◆design_choice — [basic](../tests/fixtures/svelte/syntax/format_ignore/basic_prettier_divergence/)
- `format-ignore` nested CSS — ◆design_choice — [css_nested](../tests/fixtures/svelte/syntax/format_ignore/css_nested_prettier_divergence/)
- `format-ignore` at-rule-body declaration — ◆design_choice — [css_atrule_decl](../tests/fixtures/svelte/syntax/format_ignore/css_atrule_decl_prettier_divergence/)
- `format-ignore-start` / `-end` range — ◆design_choice — [range](../tests/fixtures/svelte/syntax/format_ignore/range_prettier_divergence/)
- `format-ignore` standalone `.ts` — ◆design_choice — [ts_standalone](../tests/fixtures/typescript/syntax/comments/format_ignore_prettier_divergence/)
- `format-ignore` standalone `.css` — ◆design_choice — [css_standalone](../tests/fixtures/css/syntax/comments/format_ignore_prettier_divergence/)
- comment beside a hoisted section **inside** a range — ◆design_choice — [range_interior_comment](../tests/fixtures/svelte/syntax/prettier_ignore/range_interior_comment_prettier_divergence/)
- glued nodes **inside** a range (byte-verbatim vs prettier's inter-node re-layout) — ◆design_choice — [range_glued](../tests/fixtures/svelte/syntax/prettier_ignore/range_glued_prettier_divergence/)

**A range does not pin a section's position.** A `<script>` / `<style>` / `<svelte:options>` written *inside* a range is still lifted to the component root and printed at its canonical position, and its bytes are cut out of the frozen slice — leaving them there emits the section twice, which the parser rejects (`Duplicate instance script found`). Prettier does the same, so the plain case needs no divergence ([range_section_hoist](../tests/fixtures/svelte/syntax/prettier_ignore/range_section_hoist/)); a comment sitting beside such a section diverges ([range_interior_comment](../tests/fixtures/svelte/syntax/prettier_ignore/range_interior_comment_prettier_divergence/)), and the seam the cut leaves behind follows the byte-verbatim rule ([range_glued](../tests/fixtures/svelte/syntax/prettier_ignore/range_glued_prettier_divergence/)): tsv freezes the whole slice including inter-node whitespace, where prettier freezes node *content* but re-lays out the whitespace between nodes.

All but the two standalone entries are Svelte-embedded; those two pin the **standalone**
`.ts` / `.css` paths (acorn-typescript / `parseCss` + `tsv_ts` / `tsv_css`
directly), so the directive is covered in every language outside a Svelte host
too.

**On type-member lists (union / intersection / tuple / type parameters / type
arguments).** The ignore directives also target individual **members** of a type-member
list under one symmetric rule (**Rule A — list-item freeze**) with a **total,
placement-only classification** per directive, exception-free — **placement keys the
freeze, not the comment's spelling** (an own-line block comment behaves like an
own-line line comment):

- **own-line** (the only thing on its physical line, whitespace aside) — in the leading
  gap or between members — freezes the **following** member, the first member and every
  later member identically, in unions and intersections alike;
- **anything else is inert** — a directive sharing its line with anything else (trailing
  a member, a separator, an opening delimiter, or a declaration head, or glued before a
  member) is an ordinary comment.

A redundant paren around a frozen member
is transparent (the inner node is frozen, the clarity paren re-synthesized outside the
frozen slice; a fully redundant paren is dropped). This is the same behavior every
existing honored list position already carries — an own-line directive between `{` and
the first class member freezes that member, not the body. The ordinary member-freeze
fixtures `union_prettier_ignore_first_member` and
`union_prettier_ignore_between_members` match prettier, as do the other member-list
families' — `tuple_prettier_ignore_member` (tuple element lists),
`type_params_prettier_ignore_member` (type-parameter declarations across function /
interface / class / arrow hosts), and `type_arguments_prettier_ignore_member`
(type-argument lists in type position, call-site, and `new`-expression); tsv diverges
where freezing only the first intersection member, preserving the author's directive
position, holding the union's per-line layout, or refusing a *non-own-line* directive is
more defensible:

- First member of an **intersection** — ◆design_choice ◆prettier_bug — an own-line
  directive freezes only the first member (Rule A), the separators are parent-owned and
  the later members reformat; a leading `&` on a multi-member intersection normalizes
  away. Prettier freezes the **whole** intersection verbatim — it has no intersection
  printer, so its handling is an unmaintained emergent passthrough (and not even a stable
  contract: the between-members entry below shows it losing the freeze at its own fixed
  point). tsv keeps `input.svelte`; the fully-frozen `prettier_variant_frozen` is
  prettier-stable but tsv normalizes it to input —
  [intersection first member](../tests/fixtures/typescript/types/intersection_prettier_ignore_first_member_prettier_divergence/)
- Own-line directive between **intersection** members — ◆comment_preservation ◆prettier_bug —
  tsv keeps the directive own-line and freezes only the following member; prettier
  relocates it (pass 1 trails the preceding `&`, still freezing the member — the recorded
  `output_prettier`) and its **fixed point** collapses the intersection inline, floating
  the directive to the statement end and **losing the freeze** (non-idempotent, pinned via
  `audit_signature.txt`) — [intersection between members](../tests/fixtures/typescript/types/intersection_prettier_ignore_between_members_prettier_divergence/)
- Multi-line frozen union member — ◆design_choice — tsv keeps the union broken one member
  per line (its layout whenever a member spans lines); prettier glues the next member onto
  the frozen slice's last line (`} | (b1 & b2)`) — [multiline member](../tests/fixtures/typescript/types/union_prettier_ignore_multiline_member_prettier_divergence/)
- **Glued directive is inert** — ◆design_choice — a directive on the same line as what
  follows it (`type T = /* prettier-ignore */ A | B`,
  `a | /* prettier-ignore */ {x:1} | b`, `let v: /* prettier-ignore */ {…}`) is an
  ordinary comment in tsv: the placement rule is exception-free, and only an own-line
  directive freezes. Prettier honors the glued placement — freezing the whole union at
  the leading position, the adjacent member at a member gap (from anywhere in a glued
  comment run), the child at a single-child head, the whole mapped type from the
  in-bracket key position, and gluing a container flat around a multi-line frozen
  slice. Each fixture's `prettier_variant_frozen` pins prettier's stable frozen form,
  which tsv normalizes —
  [union](../tests/fixtures/typescript/types/union_prettier_ignore_glued_inert_prettier_divergence/),
  [tuple](../tests/fixtures/typescript/types/tuple_prettier_ignore_glued_inert_prettier_divergence/),
  [type params](../tests/fixtures/typescript/types/type_params_prettier_ignore_glued_inert_prettier_divergence/),
  [type arguments](../tests/fixtures/typescript/types/type_arguments_prettier_ignore_glued_inert_prettier_divergence/),
  [type heads](../tests/fixtures/typescript/types/type_heads_prettier_ignore_glued_inert_prettier_divergence/),
  [annotation](../tests/fixtures/typescript/types/annotation_prettier_ignore_glued_inert_prettier_divergence/)
- **After-open-brace directive is inert** — ◆design_choice — a directive trailing an
  opening `{` (`const o = { // prettier-ignore`, `interface A { // prettier-ignore`,
  `function f() { // prettier-ignore`) shares its line with the brace, so it is inert:
  the comment stays on the brace line and the first member or statement formats
  normally. Prettier relocates the directive to its own line and freezes the first
  member — a form that is stable under **both** tools (own-line is a placement tsv
  honors; each fixture's `variant_frozen` pins it) —
  [object](../tests/fixtures/typescript/expressions/objects/prettier_ignore_after_brace_inert_prettier_divergence/),
  [type members](../tests/fixtures/typescript/types/type_members_prettier_ignore_after_brace_inert_prettier_divergence/),
  [class/enum/block](../tests/fixtures/typescript/syntax/comments/prettier_ignore_after_brace_inert_prettier_divergence/)
- Union-valued **list member** (`[{ a: 1 } | { b: 2 }, c]`, `Foo<{ a: 1 } | { b: 2 }>`) —
  ◆design_choice — an own-line directive above a tuple element / type argument /
  type parameter that is itself a union freezes the **whole item** in tsv (Rule A: the
  container's gap freezes its member, whatever the member's shape — operators and all);
  prettier's union `types[0]` redirect reaches *into* the item and freezes only the first
  union member, reformatting the rest. The item-level scope is the consistent reading of
  the list rule — the directive targets the member the author pointed at, not a fragment
  of it —
  [union-valued member](../tests/fixtures/typescript/types/union_valued_member_prettier_ignore_prettier_divergence/)
- Trailing directive (`| ({ x: 1 } & a2) // prettier-ignore`) — ◆design_choice — prettier
  freezes the **preceding** member backward; tsv is permanently **inert** to a trailing
  directive (both members reformat normally), honoring a directive only where it
  **precedes** the node. The `prettier_variant_frozen` control pins that tsv never starts
  honoring this position, and that a trailing directive must not freeze the **following**
  member (both members carry perturbable object interiors so a misbound freeze in either
  direction is visible) —
  [trailing inert](../tests/fixtures/typescript/types/union_prettier_ignore_trailing_inert_prettier_divergence/),
  [tuple trailing inert](../tests/fixtures/typescript/types/tuple_prettier_ignore_trailing_inert_prettier_divergence/)
  (the tuple-family control for the same rule). The same inertness holds at the
  pre-arc honored member emitters, where prettier's backward freeze keeps a perturbed
  preceding member frozen while tsv formats both members —
  [object trailing inert](../tests/fixtures/typescript/expressions/objects/prettier_ignore_trailing_inert_prettier_divergence/),
  [type-member trailing inert](../tests/fixtures/typescript/types/type_members_prettier_ignore_trailing_inert_prettier_divergence/)
- Directive trailing the **alias head** (`type A = // prettier-ignore⏎ { x: 1 } | b`) —
  ◆design_choice — trailing per the classification (content before it on its line, value
  on the next line), so tsv is inert: the comment stays where the author put it and the
  value reformats. Prettier attaches it leading to the value, relocates it **own-line**,
  and freezes the whole value; the relocated form is then dual-stable (`variant_frozen`) —
  own-line before the value is a placement tsv honors too —
  [trailing head](../tests/fixtures/typescript/types/union_prettier_ignore_trailing_head_prettier_divergence/)
- Directive trailing an **annotation head** (`let v: // prettier-ignore⏎ { y: 2 } | c1`) —
  ◆design_choice — same inert classification; prettier keeps the directive trailing the
  `:` but freezes the annotation (no relocation), so its frozen form
  (`prettier_variant_frozen`) is one tsv normalizes —
  [trailing annotation head](../tests/fixtures/typescript/types/union_prettier_ignore_trailing_annotation_head_prettier_divergence/)
- `format-ignore` on a union member — ◆design_choice — tsv honors `// format-ignore` at the
  member position identically to `// prettier-ignore`; prettier recognizes only its own
  family and reformats the member (paired with a `prettier-ignore` control) —
  [union format-ignore](../tests/fixtures/typescript/types/union_format_ignore_prettier_divergence/)
- Frozen member with a paren-**shell** comment — ◆comment_preservation — a frozen
  **redundant**-paren member whose shell holds a comment (`(/* keep */ a1)`) is kept
  verbatim WITH its paren, so the comment survives; prettier strips the redundant paren and
  relocates the comment (`/* keep */ a1`). Comment preservation outranks redundant-paren
  removal under a freeze —
  [paren shell comment](../tests/fixtures/typescript/types/union_prettier_ignore_paren_shell_comment_prettier_divergence/)
- **Single-member** union or intersection under a freeze — ◆design_choice — a 1-element
  union/intersection collapses (drops its leading `|`/`&`) when reformatted, so a member-only
  freeze is non-idempotent. tsv keeps the operator for a **leaf / object** sole member
  (`| {a:1}`, `& {a:1}` — idempotent) where prettier drops it (`{a:1}`); a **composite** sole
  member is transparent (tsv collapses and applies Rule A to the inner Union/Intersection, so
  `| a1&a2` → `a1 & a2`, the opposite family's first-member behavior) —
  [single member union](../tests/fixtures/typescript/types/union_prettier_ignore_single_member_prettier_divergence/),
  [single member intersection](../tests/fixtures/typescript/types/intersection_prettier_ignore_single_member_prettier_divergence/)
- Parenthesized **nested-union** member — ◆design_choice ◆comment_preservation — when the
  frozen member is itself a parenthesized union (`| (| a&1 ... | b&2)`), tsv keeps the
  author's whole member verbatim: the paren layout as written, and an **in-paren** own-line
  directive stays in its authored position, applying Rule A *inside* the inner list (only
  the following inner member freezes; later inner members reformat normally). Prettier's
  binding is paren-transparent instead: it hoists an in-paren directive own-line above the
  member and re-synthesizes the parens tight around its frozen inner-union slice — which
  includes the inner leading `|`, yielding `(| aaaaaaaaaaaaa&1` — reflowing the author's
  paren layout. Surfaces on prettier's own corpus in
  `tests/format/typescript/prettier-ignore/prettier-ignore-nested-unions.ts` (an `unknown`
  in the format gate counts); a fixture pins it when the nested-paren freeze is next
  touched.

**On single-child type positions (annotation `:` / alias `=` / constraint `extends` /
type-parameter default `=` / named-tuple `label:` / mapped-type `]:` value and `[K in ...]`
key / conditional `?` · `:` branches and `extends` head / function- and constructor-type
return `=>` / predicate `is` / `as` · `satisfies` / indexed-access `[` index /
angle-assertion `<` / required-paren interior).** The same placement classification honors
a directive before a head's single child:
an own-line directive between the head token and the child freezes the child whole —
unless the (paren-transparent) child is a union or intersection **the directive is
adjacent to**, in which case the member rules above apply unchanged (first member
freezes). Adjacency is what makes that split well-defined: the composite claims the
directive through its own leading run, which crosses only whitespace and the
transparent `|` / `&` / `(`. At the conditional branches the interposing `?` / `:`
token blocks that run, so a composite branch has nothing to bind to and freezes
**whole**, operators and all — prettier's scope there too. The ordinary fixtures
`alias_prettier_ignore_value`, `annotation_prettier_ignore_union_member`,
`named_tuple_prettier_ignore_element`, `mapped_prettier_ignore_signature` (a
directive above a mapped type's `[K in ...]: V` clause freezes the whole clause — the
mapped type's sole-member analog), and `angle_prettier_ignore_own_line` (the
angle-bracket assertion's `<`→type gap, where prettier itself keeps the directive
own-line and freezes) match prettier. tsv diverges where prettier relocates
the directive, or where its mapped-type handler freezes content the directive doesn't
precede:

- Own-line directive kept own-line at a single-child head — ◆comment_preservation
  ◆prettier_bug — at the annotation `:`, constraint `extends`, default `=`, named-tuple
  `label:`, mapped-type `]:`, conditional-branch `?` · `:`, conditional-`extends`,
  function/constructor-return `)` · `=>`, predicate `is`, `as` · `satisfies`, and
  indexed-access `[` heads, prettier relocates an own-line directive before a
  non-composite child to trail the head (`let v: // prettier-ignore`) and dedents the
  frozen slice; tsv keeps the author's own-line placement and freezes the same span. A
  head-trailing directive is inert under tsv's placement classification, so the relocated
  form would lose the freeze on tsv's second pass — and prettier's relocated form is not
  even self-stable: its own second pass loses the freeze at the property-signature,
  constraint/default, named-tuple, mapped-value, conditional-`extends`, indexed-index,
  and `as`/`satisfies` positions, and the predicate position crosses three forms before
  settling (trail `is` → trail the body's `{` → lead `return`) — all non-idempotent,
  pinned via each fixture's `audit_signature.txt`. (The conditional-branch and return-`=>`
  relocations are the self-stable minority; the return-`)` gap — whose child is the whole
  `=> T` annotation rather than a type — relocates non-idempotently but never loses the
  freeze, its second pass only rejoining the `=> T` onto the `)` line.) Prettier itself
  keeps the directive own-line
  at the alias `=` and assertion `<` heads and before every union child —
  [annotation](../tests/fixtures/typescript/types/annotation_prettier_ignore_own_line_prettier_divergence/),
  [type-param constraint/default](../tests/fixtures/typescript/types/type_param_prettier_ignore_value_prettier_divergence/),
  [named tuple](../tests/fixtures/typescript/types/named_tuple_prettier_ignore_own_line_prettier_divergence/),
  [mapped value](../tests/fixtures/typescript/types/mapped_value_prettier_ignore_own_line_prettier_divergence/),
  [conditional branches](../tests/fixtures/typescript/types/conditional_prettier_ignore_branch_prettier_divergence/),
  [conditional extends](../tests/fixtures/typescript/types/conditional_prettier_ignore_extends_prettier_divergence/),
  [fn/ctor return](../tests/fixtures/typescript/types/function_type_prettier_ignore_return_prettier_divergence/),
  [predicate](../tests/fixtures/typescript/types/type_predicate_prettier_ignore_type_prettier_divergence/),
  [as/satisfies](../tests/fixtures/typescript/expressions/as_satisfies_prettier_ignore_type_prettier_divergence/),
  [indexed index](../tests/fixtures/typescript/types/indexed_access_prettier_ignore_index_prettier_divergence/)
- Frozen item over a **leading-edge paren shell** — ◆comment_preservation — the gaps that
  hand a stripped shell's leading `//` to their own emitter
  ([§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)'s
  leading-EDGE entries) do it by widening their comment window over that run. A **frozen**
  item is the one place that must not happen: it is emitted as a verbatim source slice over
  its own span, shell and comment included, and the suppression the widened window is paired
  with reaches only a doc the printer BUILDS. Against a slice there is no emitter to
  suppress, so the run prints twice. The window — and the freeze verdict itself — are
  therefore taken on the item's unwidened span. tsv and prettier agree on the whole form at
  the type-argument list, type-alias `=` RHS, tuple element and union member; at the
  conditional's `?` · `:` branches and the function type's `=>` return the directive
  relocation of the bullet above applies unchanged —
  [frozen gap shell](../tests/fixtures/typescript/types/head_paren_shell_frozen_gap_line_comment_prettier_divergence/)
- Mapped-type key: in-bracket directive freezes the binding only — ◆design_choice — an
  own-line directive inside the bracket, before `K in ...`, freezes only the binding in
  tsv, while prettier's mapped-type handler freezes the whole mapped type — including
  the `]: V` value side and the `[` that *precedes* the directive. The freeze scope
  follows the directive's position: it freezes the construct it precedes —
  [mapped key](../tests/fixtures/typescript/types/mapped_prettier_ignore_key_prettier_divergence/)
- Required-paren interior: the directive freezes the inner child only — ◆design_choice
  ◆comment_preservation — an own-line directive inside a **required** paren (an array
  type's function-type element) freezes the inner function type it precedes — Rule A's
  child scope — with the directive kept own-line inside the parens and the paren + `[]`
  re-synthesized outside the frozen slice. Prettier freezes the **whole**
  `((…) => void)[]` unit instead and relocates the directive out of the parens to trail
  the alias `=` (own-line under it at its 2-pass fixed point, freeze intact, pinned via
  `audit_signature.txt`) —
  [required paren interior](../tests/fixtures/typescript/types/array_element_paren_prettier_ignore_interior_prettier_divergence/).
  The re-synthesized `[]` is ordinary printed output, so a comment in it follows the
  ordinary suffix rule (inside the brackets, per
  [§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)'s
  parenthesized array-suffix entry) rather than riding the freeze; prettier hoists that one before
  the `[` as it relocates the directive —
  [required paren interior, bracket comment](../tests/fixtures/typescript/types/array_element_paren_prettier_ignore_bracket_comment_prettier_divergence/)

**On annotation heads (the gap *before* a `:`).** The classification also reaches the
other side of an annotation's `:` — the gap between a head and the `:` itself, which a
line comment in that gap is what makes reachable at all (it is the only canonical shape
where a `: type` leads its own line). An own-line directive there freezes the **whole
`: type` annotation**: the node the directive precedes, exactly as everywhere else. A
union or intersection value rides *inside* that frozen span rather than applying the
member rules — the composite's own leading run stops at the interposing `:`, so it can
never claim the directive, and the two rules can't both fire. An optional `?` marker
between the head and the `:` is inside the freeze rather than before it — the frozen
span starts wherever the directive's own line ends, so `?: T` freezes as a unit. tsv
keeps the directive own-line at every one of these heads; prettier relocates it to
trail the head:

- Own-line directive kept own-line at an annotation head — ◆comment_preservation
  ◆prettier_bug — at a binding's `:` (class property, parameter, variable,
  index-signature key), an index signature's value `:`, and a signature's return `:`,
  prettier freezes the same `: type` span but relocates the directive to trail the head
  (`a // prettier-ignore`, `[k: string] // prettier-ignore`, `fn() // prettier-ignore`)
  and dedents the frozen slice; tsv keeps the authored own-line placement, which is also
  the only idempotent one (a head-trailing directive is inert under the placement
  classification, so the relocated form loses the freeze on tsv's second pass).
  Prettier's own relocated forms are not self-stable either — the class property's
  directive slides onto the initializer `=`, the index signature's back inside the
  brackets, the return type's into the function **body** — losing the freeze
  (non-idempotent, pinned via each fixture's `audit_signature.txt`) —
  [binding](../tests/fixtures/typescript/types/binding_prettier_ignore_annotation_prettier_divergence/),
  [index signature](../tests/fixtures/typescript/types/index_signature_prettier_ignore_annotation_prettier_divergence/),
  [return type](../tests/fixtures/typescript/types/return_type_prettier_ignore_annotation_prettier_divergence/)
- Property signature: prettier re-binds the directive **past the `:`** —
  ◆design_choice ◆comment_preservation ◆prettier_bug — at an interface / type-literal
  property signature prettier pulls the `:` back onto the key and freezes the *type*
  alone (`a: // prettier-ignore⏎{x:   1}`), reaching a different node than the one the
  author pointed at; tsv's scope follows the directive's position, so the whole
  annotation freezes. With a second comment already in the gap prettier also **merges
  the two onto one line** (`b: // prettier-ignore // c`), making the second `//`
  ordinary text — a content loss the own-line placement avoids —
  [property signature](../tests/fixtures/typescript/types/property_signature_prettier_ignore_annotation_prettier_divergence/)
- ⚠️ A **second** comment in the index signature's `]`→`:` gap has no prettier oracle at
  all: prettier oscillates forever between the two placements (the plain-comment case is
  pinned by
  [index_signature_bracket_colon_multi_comment](../tests/fixtures/typescript/types/type_members/index_signature_bracket_colon_multi_comment_prettier_divergence/)),
  so the directive fixture keeps the single-directive shape and the multi-comment
  interaction rides the other three heads.

**On parameter lists.** A parameter list is a member list like any other, and Rule A
applies to it unchanged: an own-line directive in the `(`→first-parameter gap or between
two parameters freezes the **following parameter**, whatever its form — the slice covers
a parameter property's modifiers (`public p = 1`), a rest parameter's `...`, an optional
`?`, a default, and any parameter decorators, since all of them are part of the parameter
the directive precedes. A lone huggable parameter expands rather than hugging, because a
hug would pull the directive off its own line and make it inert. Every host shares this:
function declarations, function expressions and methods, arrows, `{#snippet}`
parameters, method / call / construct signatures, function and constructor types, and an
index signature's `[`→key gap. Prettier agrees at all of them, so the ordinary fixtures
`params_prettier_ignore_member`, `signature_params_prettier_ignore_member`,
`index_signature_prettier_ignore_key`, and the Svelte
`snippet/params_prettier_ignore_member` **match** prettier. tsv diverges at one interior
position, and under the standing glued classification:

- Directive between a parameter's **decorators** and its **binding** —
  ◆design_choice ◆comment_preservation ◆prettier_bug — tsv freezes the binding, the node
  the directive precedes, with the decorators printing normally outside the frozen slice.
  Prettier freezes nothing at a parameter property, and at a plain binding it re-binds
  the directive *past the name* (trailing `c`, freezing only the `: T` annotation) — a
  form whose own second pass floats the directive up to trail the last decorator and
  loses the freeze (non-idempotent, pinned via `audit_signature.txt`) —
  [parameter binding](../tests/fixtures/typescript/typescript_specific/decorators/parameter_prettier_ignore_binding_prettier_divergence/)
- **Glued directive is inert** in a parameter list too — ◆design_choice — the same
  exception-free placement rule; prettier honors the glued placement and, for a
  multi-line frozen parameter, keeps the list flat around the frozen slice
  (`prettier_variant_frozen` pins its stable form) —
  [params glued](../tests/fixtures/typescript/declarations/function/params_prettier_ignore_glued_inert_prettier_divergence/)

**On argument and element lists.** Rule A again, unchanged: an own-line directive in the
`(`→first-argument / `[`→first-element gap or between two items freezes the **following
item** — a call's, a `new`'s and a dynamic `import()`'s arguments, and an array literal's
or array pattern's elements. The slice is the item's own node span, so a spread or rest
`...` rides inside it (the `...` is part of what the directive precedes), and an argument
needing clarity parens keeps them around the frozen slice (`(a = b  +  c)`). A lone
huggable argument expands rather than hugging, for the same reason as a lone parameter.
An array hole contributes only its comma, so the element after one still freezes.
Two node-level facts ride with the slice, both because the printed form of an argument is
wider than its own span: a block comment **glued** before the item is OWNED by it and would
travel inside the doc the slice replaces, so the freeze claims it; and a
`SequenceExpression` prints its own grouping parens (`fn((0, 1))` passes ONE argument), so
the freeze re-synthesizes those too. Prettier agrees at every one of these positions, so the
ordinary fixtures `calls/args_prettier_ignore_member`,
`calls/chained/args_prettier_ignore_member`, `calls/import_args_prettier_ignore_member`,
`new/args_prettier_ignore_member`, `arrays/elements_prettier_ignore_member`,
`patterns/prettier_ignore_element`, `parenthesized/jsdoc_cast_prettier_ignore_interior` (the
directive inside a JSDoc cast's own parens, which freezes the cast's inner), and the Svelte
`expressions/call_args_prettier_ignore_member` (the embedded-TS route, where the frozen
slice is a raw range in host coordinates) all **match** prettier on the FREEZE. Two layouts
part from it, and only one is a freeze question:

- Directive in the `(`→argument gap of a lone **multiline-template** argument —
  ◆design_choice ◆comment_preservation — the call expands where prettier hugs. This is the
  only argument position where prettier HAS a hug to disagree with (the flat form of
  `printCallExpression`'s `isTemplateLiteralSingleArg` branch, comment-independent there),
  and the flat concat has no line of its own to put the directive's run on: it would land on
  the `(`'s line, inert, and the freeze would be gone on the second pass. Prettier hugs and
  stays frozen because it decides a directive by comment *attachment* rather than by
  placement. All four call spellings agree, and with nothing glued to the backtick the
  newline before it declines the hug anyway, so both tools expand —
  [template argument](../tests/fixtures/typescript/expressions/calls/template_arg_prettier_ignore_expands_prettier_divergence/)
- The flat **test-call** layout, which is a comment-position question rather than a freeze
  one: see [§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)'s
  test-call entry.

**On module and declarator lists.** Rule A once more, over the remaining comma lists: an
own-line directive in the `{`→first-item gap or between two items freezes the **following
item** — an import's or export's named specifiers, a `with { … }` clause's import
attributes, and a variable declaration's declarators (in a statement and in a `for`
header's init clause alike). The slice is the item's own node span, so an inline `type`
modifier, a string specifier, a declarator's annotation and initializer, and a
destructuring binding all ride inside it, and the separating `,` stays parent-owned. The
list's first gap opens just past the delimiter, which for a declarator list is the
`const`/`let`/`var` keyword and for an `import`'s leading binding the `import` keyword
itself — so the first specifier, first attribute, first declarator and a whole `* as ns`
namespace binding all freeze from that gap. Prettier agrees at the braced lists and at the
inter-declarator gaps, so the ordinary fixtures
`imports/specifiers_prettier_ignore_member`, `exports/specifiers_prettier_ignore_member`,
`imports/attributes_prettier_ignore_member` and
`variable/declarations_prettier_ignore_member` all **match** prettier. tsv diverges at two
places, both because prettier decides a directive by comment *attachment* where tsv decides
by placement:

- Own-line directive **between two module specifiers** — ◆design_choice ◆prettier_bug — tsv
  freezes the following specifier, like every other member list. Prettier's module-specifier
  comment handler re-binds an own-line comment whose preceding node is an
  `ImportSpecifier` / `ExportSpecifier` as that specifier's **trailing** comment, so its
  freeze runs **backward**: the preceding specifier is emitted verbatim and the following
  one reformats. `divergent_variant_backward` pins the direction — with the preceding
  specifier perturbed instead, prettier keeps it frozen while tsv normalizes it and freezes
  forward. The forward direction is the consistent reading of the list rule, and the same
  reason a trailing directive is permanently inert —
  [specifiers between](../tests/fixtures/typescript/modules/imports/specifiers_prettier_ignore_between_prettier_divergence/)
- Directive in a **declaration-header gap** (`const`/`let`/`var`→first declarator,
  `import`→binding) — ◆design_choice ◆comment_preservation — tsv freezes the item and keeps
  the directive on **its own line**, leaving the keyword alone on the line above. Prettier
  pulls it flush against the keyword (`const // prettier-ignore`) and freezes anyway; tsv
  cannot follow, since a directive sharing its line with anything else is inert under the
  placement floor and the relocated form would lose the freeze on tsv's own second pass.
  Each fixture's `divergent_variant_flush` pins prettier's stable flush form, which tsv
  reads as inert and reformats. When the gap already holds another comment the two tools
  agree — that comment takes the keyword line and the directive is own-line either way —
  [declarator head](../tests/fixtures/typescript/declarations/variable/declarations_prettier_ignore_head_prettier_divergence/),
  [namespace binding](../tests/fixtures/typescript/modules/imports/namespace_prettier_ignore_binding_prettier_divergence/)

**A header-gap directive keeps its own line even where nothing freezes.** The rule above is
stated on the emitter, not on the freeze: a directive tsv relocated onto the keyword's line
would be **inert**, so tsv never relocates one — at a `function`/`class` head, where neither
tool freezes, just as at the declarator head, where both do. Prettier reflows it flush and
reformats; the one-sided invariant is deliberate, because it keeps every header gap eligible
to start honoring a directive later instead of having an emitter silently destroy it first.
tsv is likewise inert to the flush form and never moves a comment *up*, so prettier's stable
output is a form tsv only re-indents (`divergent_variant_flush`, the pre-existing keyword→value
hang) —
[keyword-gap own line](../tests/fixtures/typescript/syntax/comments/keyword_gap_prettier_ignore_own_line_prettier_divergence/)

**On delimiter-owned value heads, and on sequence operands.** A construct that holds a single
value behind a delimiter of its own — a `for` header's `(`→init, `;`→test and `;`→update
clauses and a for-in/for-of header's `(`→left clause, a **condition head**'s `(` (`if` /
`else if` / `while` / `do…while`, a `switch` discriminant, a `catch` parameter), a restricted
production's grouping `(` (`return` / `throw` / `yield`), and a Svelte
`{…}` value (`bind:` / `on:` / `class:` / `style:` and an expression tag) — freezes that
**whole value** when an own-line directive sits in the gap. The slice is the value's own node
span, so the delimiter that closes it (the header's `;`, the `in`/`of` keyword, the condition's
`)`, the grouping `)`, the closing `}`)
stays parent-owned, and a sibling clause or attribute the freeze does not reach still
normalizes. Prettier agrees at every one of those positions, so the ordinary fixtures
`for/clauses_prettier_ignore_head`, `statements/condition_prettier_ignore_head`,
`return_throw/operand_prettier_ignore_head` and
`bind/value_prettier_ignore_head` **match**.

A test position carries one more parent-owned fact: the **clarity parens the printer supplies**
for a value that would otherwise read as a typo — an assignment prints `if ((a = b))`, and
`for (; (a = b); )` — belong outside the frozen slice, exactly as an argument's do, so the
frozen inner keeps the parens around it (`if (⏎// prettier-ignore⏎(aaa  =  bbb))`, prettier
agreeing at both hosts).

Inside a sequence the classification is Rule A again, unchanged: an own-line directive in an
**inter-operand** gap freezes only the **following** operand, and the operands on either side
of it reformat. The two rules meet at a sequence's leading gap, where the directive leads the
*sequence node* rather than its first operand — so the whole sequence rides inside one
verbatim slice, in a `for` clause, a `return`/`throw` operand and a Svelte value alike
(`sequence/operands_prettier_ignore_member` covers the inter-operand half and matches
prettier). Two node-level facts the slice carries are the same ones every value-side freeze
carries: a block comment glued before the value is **owned** by it and rides inside the doc
the slice replaces, so the freeze claims it; and a `SequenceExpression` prints its own
grouping parens *outside* its node span, so a frozen sequence operand re-synthesizes them or
loses its grouping.

A **glued** directive is inert here as everywhere — `for (/* prettier-ignore */ i = 0; …)`,
`return /* prettier-ignore */ a + b`, `if (/* prettier-ignore */ a + b)` — where prettier honors
the glued placement and freezes;
each fixture's `prettier_variant_frozen` pins prettier's stable frozen form, which tsv
normalizes ([clauses glued
inert](../tests/fixtures/typescript/statements/for/clauses_prettier_ignore_glued_inert_prettier_divergence/)).

tsv diverges at six places:

- Directive written in an **empty `for` clause slot** — ◆comment_preservation — it stays in
  that slot, so it freezes nothing: the clause it would freeze is on the other side of the
  `;`. Prettier moves it across into the next clause and freezes there. This is the freeze
  consequence of the already-sanctioned empty-slot rule
  ([§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)), and it follows
  from the general one — the directive that freezes a value is the one printed above it. The
  relocated authoring is dual-stable (`variant_relocated`): there the directive really does
  lead the test clause, and tsv freezes it too —
  [empty slot inert](../tests/fixtures/typescript/statements/for/empty_slot_prettier_ignore_inert_prettier_divergence/)
- `yield` / `yield*` operand — ◆comment_preservation ◆prettier_bug — tsv freezes and keeps the
  hanging-paren layout the own-line comment forces; prettier relocates the directive onto the
  keyword's line and strips the parens (the pre-existing `yield` relocation in
  [§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation), here
  carrying the directive). Its relocated form is **not a fixed point** — the
  next pass reformats the plain-`yield` operand and **loses the freeze**, pinned in
  `audit_signature.txt`. `return` / `throw` match prettier —
  [yield operand](../tests/fixtures/typescript/statements/return_throw/yield_operand_prettier_ignore_head_prettier_divergence/)
- `for` init that is a **declaration** — ◆prettier_bug — prettier's frozen slice swallows the
  header's `;` and then emits the separator after it (`let i  =  0, j = 1;;`), producing a
  four-clause header that **does not parse**. tsv keeps the separator parent-owned, as at
  every other frozen list item —
  [init declaration](../tests/fixtures/typescript/statements/for/init_declaration_prettier_ignore_head_prettier_divergence/)
- **for-in / for-of left clause** — ◆comment_preservation ◆prettier_bug — prettier relocates the
  directive flush against the `(` (`for (// prettier-ignore⏎const  xxx …`), a placement tsv never
  writes; and where the left is a **declaration** its frozen slice is followed by a `;`, giving
  `for (const  xxx; of yyy)` — the same unparseable output as the `for`-init form above. A
  **pattern** left freezes correctly for both, so there only the relocation differs. tsv keeps
  the directive on its own line, which holds the header in the standing for-in/for-of
  line-comment layout (binding, keyword and iterable each on their own line) — for **both**
  spellings, unlike an ordinary block comment, which still rides inline. The same placement
  rule holds in the header's keyword→binding gap, where nothing freezes yet —
  [in/of left](../tests/fixtures/typescript/statements/for/in_of_left_prettier_ignore_head_prettier_divergence/)
- Frozen **function-binding sequence** in a `bind:` value — ◆comment_preservation ◆prettier_bug —
  tsv emits the getter/setter pair bare, as it does for every other function-binding value.
  Prettier parenthesizes it (which Svelte reads as a grouped expression, not a binding pair)
  and then **drops the directive entirely** on its second pass, reformatting the value — the
  same non-idempotent loss the plain-comment case already has —
  [bind value sequence](../tests/fixtures/svelte/directives/bind/value_sequence_prettier_ignore_head_prettier_divergence/),
  sibling [function_comment_inline_block](../tests/fixtures/svelte/directives/bind/function_comment_inline_block_prettier_divergence/)
- Directive in a **`{…}` value gap** — ◆design_choice ◆comment_preservation — tsv keeps it on
  its own line, so the value takes the broken block form; prettier pulls it flush against the
  `{` (`class:active={// prettier-ignore`) and freezes anyway. This is the `{…}` instance of
  the header-gap rule above — the flush form is inert under the placement floor, so following
  prettier would lose the freeze on tsv's own second pass. Per the placement-only
  classification, **both spellings** take the broken form: a `//` directive ends in a hardline
  of its own, while a `/*…*/` one is emitted inline into a softline-hung group, so the block
  half is pinned explicitly across every braced value shape (directive value, expression tag,
  `bind:` value, `bind:` function-binding sequence). `bind:` needs no divergence for the line
  spelling, where it already writes the broken form —
  [braced value own line](../tests/fixtures/svelte/syntax/prettier_ignore/braced_value_own_line_prettier_divergence/)

  The value's **closing** `}` follows the ordinary rule, freeze or not: a trailing run ending
  in a line comment already ended the line, so the closer reuses that break rather than adding
  a second one (which would render as a blank line above it). Reusing a break also means
  inheriting its column, so the run's final break is emitted **dedented** out of the content's
  indent — the `}` then lands where it lands with no trailing comment at all, rather than one
  level deeper because a comment happened to be there. This is what
  `build_prefixed_head_doc` does one delimiter out for the prefixed heads, and the unprefixed
  `{…}` values — expression tag, attribute value, `bind:` value — owe the identical shape —
  [braced value trailing line](../tests/fixtures/svelte/syntax/prettier_ignore/braced_value_trailing_line_prettier_divergence/)

`yield`'s hanging-paren layout carries its own pre-existing comment relocation (see
[§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)); the freeze rides on it rather than adding a
second divergence.

**On assignment-family value heads.** The same head rule, with the assignment operator as the
delimiter: an own-line directive in an `=`→value, `:`→value or `=>`→body gap freezes that
**whole value**. The hosts are a declarator initializer (`const a =`, a `for` header's init
declarator, and a Svelte `{@const a =}`), an assignment
RHS (`a =`, a compound `a +=`, and each segment of a chain), an object property value (`k:`), a
class field value (`f =`, `static f =`, `#f =`, `accessor f =`), an enum member value
(`A =`), an arrow's expression body (`=>`), and a default value (a
parameter default, a destructuring default, an array-pattern default). The slice is the value's
own node span, so the binding, the operator and the enclosing list stay parent-owned and a
sibling declarator, member or property the freeze does not reach still normalizes. Prettier
agrees at every unprefixed host, so the ordinary fixtures
`statements/variable/init_prettier_ignore_head`, `expressions/assignment/rhs_prettier_ignore_head`,
`statements/for/init_declarator_prettier_ignore_head`,
`expressions/arrow/body_prettier_ignore_head`
and `svelte/tags/const/value_prettier_ignore_head` **match**.

A **ternary branch** (`?`→consequent, `:`→alternate) and a **`case`→test** are the same shape
one family over and are still absent from the rule, so a directive there is inert while
prettier honors it. Tracked gaps, not sanctioned differences.

The clarity parens rule carries over unchanged — an initializer that is an assignment prints as
`const a = (b = c)`, and those parens are the printer's, so the frozen inner keeps them around
it. So does the placement rule: the pattern-family gaps trail an ordinary own-line comment onto
the operator's line (`aaa = // c`, a relocation
[§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation) already sanctions at
`param_default_*_comment_prettier_divergence`), but an **honored directive keeps its own line**
there, since the trailing placement is inert under the floor and following it would lose the
freeze on tsv's own second pass. That is the declaration-header rule of §On module and declarator
lists, one delimiter out.

**The slice ends at the value; the shell around it does not.** Because the clarity parens (and a
sequence's own required pair) sit OUTSIDE the frozen slice, the gap between the slice's end and
that `)` is an ordinary paren-shell gap — and a printer that synthesizes its own `(`…`)` owns the
gap inside it, so a comment left unclaimed there is dropped outright rather than relocated. The
frozen and unfrozen forms therefore answer it identically: **which** shell is retained, **where**
the comment renders, and **whether** it defers past the terminator are questions about the gap,
not about what renders between the parens.

**Which emitter owns that gap is the host's, not the freeze's**, and the two arrangements give
two different answers — each of them the host's *unfrozen* answer, which is the whole claim:

- A host that owns everything inside its own boundary — a **declarator initializer** (statement
  or `for` header), an **assignment RHS**, an **arrow body** — emits the gap itself, so the
  comment stays **inside** the surviving pair:
  a block inline, a `//` with the pair opened around it (inline it would swallow the `)`). A
  shell the value does not need strips, and the block then defers past the `;` — except in a
  `for` header, whose `ForClauseSeparator` tail licenses no deferral at all: the clause's `;`
  ends the declarator the comment was written in, so the block stays inline there. These are the
  hosts the fixtures below pin.
- A host whose **enclosing** seam claims the gap — a **class field**, an **object property
  value**, an **enum member value**, a **parameter default** — floats the comment out past the
  pair
  (`(bbb  =  ccc); /* c */`), exactly as its unfrozen twin does. tsv matches prettier at the class
  field and the enum member; it parts at the object property's `//` (which prettier hoists to lead
  the property, the
  standing relocation family) and at a **frozen parameter default**, where prettier's ignore range
  covers the pair and so keeps the comment inside it. That last parting is a **tracked gap, not a
  sanctioned difference** — the frozen and unfrozen forms agree under tsv, but prettier's do not.

tsv diverges at six places:

- **A frozen value's surviving shell** — ◆comment_preservation ◆prettier_bug — prettier
  **throws** on a comment in that gap (`Comment "c" was not printed`): its ignore path replaces
  the value with the verbatim range and never visits the comment past that range's end, while
  the pair still prints. There is no prettier output to compare against, so the fixtures carry
  `prettier_rejects.txt`; tsv keeps the comment where the author wrote it, matching its own
  unfrozen twins (`init_assignment_paren_block_comment`,
  `init_assignment_paren_line_comment_prettier_divergence`) — the declarator initializer
  [init paren comment](../tests/fixtures/typescript/statements/variable/init_prettier_ignore_paren_comment_prettier_divergence/),
  the assignment RHS
  [rhs paren comment](../tests/fixtures/typescript/expressions/assignment/rhs_prettier_ignore_paren_comment_prettier_divergence/)
  and the `for` header's init declarator
  [init declarator paren comment](../tests/fixtures/typescript/statements/for/init_declarator_prettier_ignore_paren_comment_prettier_divergence/).
  The **arrow body** is the one host of this family where prettier survives the shape — its
  grouping pair strips rather than throwing — so that host has an oracle and keeps its own
  entry below
- **A frozen value's REDUNDANT shell** — ◆comment_preservation — the shell strips, so prettier
  survives and there is an oracle. Both tools defer the **block** past the `;`; on the **line**
  spelling tsv retains the shell and keeps the `//` inside it, where prettier strips anyway and
  carries the comment past the `;` onto a line it does not own. The same parting the unfrozen
  twin already records, and prettier reaches its own block answer only on a second pass (chain
  pinned) —
  [init redundant paren comment](../tests/fixtures/typescript/statements/variable/init_prettier_ignore_redundant_paren_comment_prettier_divergence/).
  A `for` header's init declarator parts on the **line** spelling for the same reason and on the
  block for none — its clause `;` is not a terminator, so the block stays inline and matches —
  [init declarator redundant paren comment](../tests/fixtures/typescript/statements/for/init_declarator_prettier_ignore_redundant_paren_comment_prettier_divergence/)
- **A frozen arrow body's shell** — ◆comment_preservation — tsv retains the author's pair and
  keeps the comment inside it, a block inline and a `//` with the pair opened around it, exactly
  as the unfrozen arrow body does. Prettier strips the grouping pair and relocates the comment
  out, floating a `//` past the body's `;` and moving a block outside a *required* object pair,
  which re-associates it from the object to the whole expression; its second pass moves the
  blocks again (chain pinned). The unfrozen twin is already sanctioned at
  `arrows/body_paren_comment_prettier_divergence` —
  [body paren comment](../tests/fixtures/typescript/expressions/arrow/body_prettier_ignore_paren_comment_prettier_divergence/)
- **A directive inside the before-`=` continuation** — ◆comment_preservation — when a comment
  before the `=` drops `= value` to a continuation line, the `=`→value gap inside it keeps its
  own rule: an own-line directive still keeps its own line and still freezes. Prettier relocates
  the before-`=` comment past the operator (the family divergence §Comment relocation already
  sanctions) and honors the freeze either way —
  [before-`=` value-head freeze](../tests/fixtures/typescript/declarations/variable/before_eq_comment_value_head_freeze_prettier_divergence/)
- **Enum member value** — ◆comment_preservation — the one host of the family where prettier
  relocates an own-line `=`→value comment onto the `=`'s line (it agrees with tsv at the
  declarator, the class field, the assignment RHS and the object value). tsv keeps the line the
  author gave it, directive or not: trailing the operator a directive is inert under the floor,
  so following the relocation would cost the freeze on tsv's own second pass. Prettier
  demonstrates exactly that loss — its own pass 2 floats the directive past the value
  (`Bbb = ccc + ddd // prettier-ignore`) and the freeze is gone, the same second-pass loss its
  `enum` / `namespace` **body** heads show below —
  [member init head](../tests/fixtures/typescript/declarations/enum/member_init_prettier_ignore_head_prettier_divergence/)
- **Default value** — ◆design_choice — tsv breaks the enclosing list around the frozen value,
  because the directive's own line is a mandatory break inside that list and a list holding a
  break prints expanded — the same layout a plain own-line comment in that gap already produces.
  Prettier keeps the list flat and glues the closer to the frozen value's last line
  (`function fn(aaa =⏎…⏎bbb  +  ccc) {}`, `const [iii =⏎…⏎jjj  ||  kkk] = lll`) —
  [default head](../tests/fixtures/typescript/expressions/assignment/default_prettier_ignore_head_prettier_divergence/)

**On statement positions.** Rule A once more, over statements. An own-line directive in a
statement **list** — a `switch` body's `{`→first-case and between-case gaps, a case label's
`:`→first-statement and between-statement gaps — freezes the **following** member over its own
node span, exactly as it already does in a program body and a block body. A statement **head**
— the `)`→consequent and `else`→alternate gaps, every loop's →body gap (`while`, C-style `for`,
for-in / for-of, and `do`, which introduces its body with no `)` of its own), a `label:`→body
gap, the `}`→`catch` / `}`→`finally` gap, and the →`{` gap of every braced-body **declaration**
head (`class`, `interface`, `enum` / `const enum`, and `namespace` / `module` / `declare
global`) — freezes the single statement, clause or body that follows it. The slice is that
node's own span, so a `case` label rides inside its own frozen case while the sibling cases
normalize, and a declaration's name, type parameters and `extends` clause stay parent-owned
while the body freezes. A **block** body freezes with its braces, and a loop's
collapsed-empty-block form (`for (…) {}`) yields to the verbatim slice.

Those heads relocate an ordinary own-line comment — a labeled statement's trails the label
(`lll: // c`), and a declaration head's trails its last token (`class Aaa // c`,
`enum Aaa // c`, `namespace Aaa // c`) — and there, as at the declaration headers of §On module
and declarator lists, an **honored directive keeps its own line** instead: the trailing
placement is inert under the floor, so following the relocation would lose the freeze on tsv's
own second pass. The four declaration heads answer both questions on one pair of seams —
`Printer::gap_frozen_span` resolves the slice, `Printer::build_header_pre_body_doc` places the
run and picks the pre-`{` separator.

Prettier agrees at the list positions and at the `if` heads — including the freeze's SCOPE
there, which the fixtures' `unformatted_spaces` variants pin by perturbing the head outside the
slice — so the ordinary fixtures `statements/switch/case_prettier_ignore_head`,
`statements/switch/consequent_prettier_ignore_head`,
`statements/if/branch_prettier_ignore_head` and
`statements/loops/body_prettier_ignore_head` **match**. tsv diverges at three heads:

- **`catch` / `finally` clause** — ◆comment_preservation — prettier moves the directive inside
  the clause's block body and freezes the **first statement** there, so its `catch` binding
  normalizes while tsv freezes the whole clause. The plain-comment form of the same relocation
  is already sanctioned at
  [catch_between_comment](../tests/fixtures/typescript/statements/try/catch_between_comment_prettier_divergence/) —
  [handler head](../tests/fixtures/typescript/statements/try/handler_prettier_ignore_head_prettier_divergence/)
- **Declaration body** — ◆comment_preservation — tsv freezes the whole body at all four heads.
  Prettier splits: at `class` and `interface` it pulls the `{` up onto the head line, moves the
  directive inside the body and freezes the **first member**; at `enum` and `namespace` it
  relocates the directive onto the *header* line, where the freeze does not survive its own
  second pass — pass 1 keeps the body verbatim under `enum Aaa // prettier-ignore`, pass 2
  normalizes it, so the authored directive ends up affecting nothing. That is the same
  second-pass loss tsv's own-line placement exists to avoid, one formatter over —
  [class body head](../tests/fixtures/typescript/class/body_prettier_ignore_head_prettier_divergence/),
  [interface body head](../tests/fixtures/typescript/declarations/interface/body_prettier_ignore_head_prettier_divergence/),
  [enum body head](../tests/fixtures/typescript/declarations/enum/body_prettier_ignore_head_prettier_divergence/),
  [namespace body head](../tests/fixtures/typescript/declarations/namespace/body_prettier_ignore_head_prettier_divergence/)
- **Labeled body** — ◆design_choice — a SCOPE difference rather than a relocation: prettier
  freezes the whole labeled statement (its comment attaches to the `LabeledStatement`, since a
  `:` begins no node), so a spaced label survives; tsv freezes the body the directive actually
  precedes and normalizes the label. Pinned by a `prettier_variant_label_spaces` form, and
  shown to be the freeze's doing by the same label normalizing without a directive —
  [labeled body head](../tests/fixtures/typescript/statements/labeled/body_prettier_ignore_head_prettier_divergence/)

One statement position is **inert by agreement**: a directive between a **decorator** and its
declaration (`@dec⏎// prettier-ignore⏎export class D {}`) freezes nothing, in prettier or in
tsv. The decorator belongs to the declaration it decorates, so the gap is inside the statement
rather than before it, and there is no following member for the rule to bind to.

**On declaration heads and parenthesized statements.** The last two statement-level heads are
the ones where prettier **relocates the directive out of the gap** and freezes anyway — the
`export`→declaration, `export default`→value and `export =`→value gaps, and the `(`→expression
gap of a statement whose parens the printer keeps. tsv freezes the same node at all four and
keeps the directive
where the author wrote it, which at the two `export` heads is the uniform declaration-header
layout an ordinary comment already takes there (the keyword alone on its line, the continuation
indented) and at the paren head is inside the broken parens. Both spellings behave alike;
placement keys the freeze.

The `export` keyword is parent-owned and stays outside the slice, while decorators written
*after* it belong to the declaration and ride inside it. The gap one delimiter *earlier* — the
`export`→`=` interior of `export =` — is not a head at all: a `=` begins no node, so Rule A has
nothing to bind to and a directive there freezes nothing (prettier reaches past the `=` and
freezes the value). At the paren head the parens are the
printer's, so they too stay outside — and when a frozen slice's own leftmost token needs them
(`{ bbb: 2 }.ccc`, which would otherwise reparse as a block) the shell goes around the **whole**
slice, since a verbatim slice has no interior for the printer to wrap. Where the parens are
merely redundant tsv drops them and the directive leads the statement, matching prettier
(`statements/expression_statement_paren_dropped_prettier_ignore_head`).

- **`export`→declaration head** — ◆comment_preservation — prettier pulls the directive flush
  onto the `export` line (`export // prettier-ignore`) and freezes anyway; tsv keeps the
  author's line, since the flush placement is inert under the floor and would lose the freeze
  on the second pass. The plain-comment form of the same relocation is already sanctioned at
  [export_declaration_line_comment](../tests/fixtures/typescript/syntax/comments/export_declaration_line_comment_prettier_divergence/) —
  [named head](../tests/fixtures/typescript/modules/exports/named_declaration_prettier_ignore_head_prettier_divergence/),
  [default head](../tests/fixtures/typescript/modules/exports/default_declaration_prettier_ignore_head_prettier_divergence/),
  [`export =` head](../tests/fixtures/typescript/modules/exports/export_equals_prettier_ignore_head_prettier_divergence/)
- **Paren-kept expression statement** — ◆comment_preservation ◆prettier_bug — prettier hoists
  the directive out before the `(` and glues the frozen slice back inside parens on one line.
  On the leftmost-token case it also drops the shell, emitting `{ bbb:  2 }.ccc;`, which does
  not reparse —
  [paren head](../tests/fixtures/typescript/statements/expression_statement_prettier_ignore_head_prettier_divergence/)
- **`export`→class gap of a decorator-FIRST class** — ◆comment_preservation
  ◆content_preservation — `@dec⏎export⏎// c⏎class C {}`. The declaration's span opens at the
  decorator, so this gap is *inside* it, and a directive there freezes nothing in either tool —
  the mirror image of the decorator→declaration gap above. Prettier hoists a line comment above
  `export` and trails a block comment on the decorator; tsv keeps both in place. tsv previously
  **dropped** every comment in this gap, scanning it over an inverted range —
  [before export](../tests/fixtures/typescript/typescript_specific/decorators/before_export_comment_prettier_divergence/)

**On prefixed Svelte braced heads.** The head rule reaches one delimiter further out: a `{`
that carries a **prefix** before its value. An own-line directive in the prefix→value gap
freezes that whole value — the tags `{@html }`, `{@render }`, `{@attach }` and `{@debug }`, the
`{...}` spread attribute, and every block head (`{#if }` / `{:else if }`, `{#each }` and an
`{#each}` key's own `(`, `{#key }`, `{#await }`). The prefix, the `as` clause, the key's parens
and the closing `}` are all parent-owned and stay outside the slice; a sibling tag, spread or
block the freeze does not reach still normalizes. `{@debug}`'s slice is the identifier **list**
(first identifier through last), which is what that tag normalizes. `{@const}` is not in this
family — its `=` makes it an assignment head, where prettier agrees
([svelte/tags/const/value_prettier_ignore_head](../tests/fixtures/svelte/tags/const/value_prettier_ignore_head/)).

Three Svelte positions look like heads but are not: `{#snippet ⟨name⟩}`, `{#each … as ⟨pattern⟩}`
and `{#await … then ⟨pattern⟩}` reject a comment in that gap in **both** parsers, so there is
nothing to bind to.

Every one of these is a divergence, for a single reason: prettier **relocates** the directive
flush onto the prefix's line (`{@html // prettier-ignore`, `{#if // prettier-ignore`) and
freezes anyway. That placement is inert under tsv's floor, so following it would lose the freeze
on tsv's own second pass. The unprefixed `{…}` values already have an own-line form to fall back
on (`wrap_in_block_structure` — the `{⏎…⏎}` block that
[bind/value_prettier_ignore_head](../tests/fixtures/svelte/directives/bind/value_prettier_ignore_head/)
pins); a prefixed head had none, so it takes the same shape one prefix out — the prefix alone on
its line, the directive and the frozen slice indented below it, and the closing token dangling.
The block heads already reach that geometry whenever a leading line comment breaks the head, so
only the directive's own line is new there. Inside a whitespace-significant element (`<pre>` /
`<textarea>`) the dangle is suppressed as always and the closer hugs the slice.

- **Prefixed tag heads** — ◆comment_preservation —
  [`{@html}` / `{@render}` / `{@attach}`](../tests/fixtures/svelte/tags/prefixed_value_prettier_ignore_head_prettier_divergence/),
  [`{...}` spread](../tests/fixtures/svelte/attributes/spread_prettier_ignore_head_prettier_divergence/)
- **Block heads** — ◆comment_preservation — the `}` dangle is the block layout tsv already
  takes for a broken head ([§Svelte: Blocks](./conformance_prettier_svelte.md#svelte-blocks)) —
  [block heads](../tests/fixtures/svelte/blocks/head_prettier_ignore_prettier_divergence/)
- **A frozen head whose value takes clarity parens** — ◆comment_preservation ◆prettier_bug —
  an assignment value is parenthesized by the printer (`{@html (a = b)}`), and those parens
  stay **outside** the verbatim slice, like the prefix and the `}`. The rule keys on the
  **value**, so every braced position answers it identically — the prefixed heads (tag,
  block, `{...}` spread) and the unprefixed `{…}` values (attribute value, expression tag)
  alike; the lone exception is `{@const}`'s initializer, where the paren is fully redundant
  and normalizes away frozen or not. Prettier has no freeze to
  compare against here: its `remove_parens` pass **deletes** the directive along with the
  wrapper it attached to, so the value normalizes and the comment is lost outright. Also a
  `_svelte_divergence` — the same pass moves the parser's attachment (§Comment Attachment
  Differences in [conformance_svelte.md](./conformance_svelte.md)) —
  [assignment head](../tests/fixtures/svelte/tags/assignment_prettier_ignore_head_svelte_prettier_divergence/)
- **`{@debug}` head** — ◆comment_preservation ◆prettier_bug — prettier supplies no freeze
  semantics to compare against, because it **deletes** every comment inside a `{@debug}`,
  directive included (the content loss
  [debug_comment](../tests/fixtures/svelte/tags/debug/debug_comment_prettier_divergence/)
  already catalogs). tsv preserves the comment, so it must also answer where the comment goes
  and what it freezes, and answers both the way the rest of the family does —
  [debug head](../tests/fixtures/svelte/tags/debug/value_prettier_ignore_head_prettier_divergence/)

See [directives.md](./directives.md) for the user-facing reference.

