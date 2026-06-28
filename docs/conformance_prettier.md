# Prettier Conformance

Prettier was tsv's initial guide, and the formatter still tracks it for the common case — but tsv has its own identity and makes **intentional, cataloged choices** to diverge where they're more defensible. This document catalogs those divergences along with bugs that tsv does not replicate.

## Terminology

**Matched**: tsv produces identical output to Prettier — the goal and the common case (measure current rates with `deno task corpus:compare:format --all --summary`).

**Unmatched**: tsv produces different output. The suffix `_prettier_divergence` marks these fixtures. This document explains WHY for each case.

## Reasons tsv Differs

- Spec violation — Prettier violates CSS/HTML/JS spec. tsv action: tsv follows the spec
- Stable quirk — Prettier preserves multiple forms without normalizing. tsv action: tsv normalizes consistently
- Prettier bug — Prettier is non-idempotent or emits invalid output. tsv action: tsv produces stable, valid output
- Parser compat — Prettier's output breaks Svelte's parser. tsv action: tsv produces Svelte-compatible output
- Print width — Prettier allows lines to exceed printWidth. tsv action: tsv breaks to stay within limit
- Tabs-only indent — Prettier mixes tabs and spaces under --use-tabs. tsv action: tsv uses whole tabs only
- BOM stripping — Prettier preserves byte-order marks. tsv action: tsv strips them
- Semantic preservation — Prettier changes meaning (strips parens). tsv action: tsv preserves original semantics
- Comment preservation — Prettier moves comments to different syntactic position. tsv action: tsv preserves comment position
- Content preservation — Prettier silently drops user comments. tsv action: tsv preserves all comments
- Design choice — Other deliberate behavior differences. tsv action: Documented rationale in fixture

> Most `Comment preservation` and `Content preservation` divergences live in the prose-form [TypeScript: Comments](#typescript-comments) and [CSS: Comments](#css-comments) catalogs, not the Reason-tagged catalog lists — they're the largest divergence category but don't fit a one-word Reason tag.

## Decision Framework

**When to match Prettier:**

- Cosmetic choices (spacing preferences, quote styles)
- Output that's valid and reasonable
- Unclear which approach is "better"

**When to differ:** any reason in [Reasons tsv Differs](#reasons-tsv-differs) above. The three cross-cutting principles — comment position, print width, and tabs-only indentation — are detailed below.

### Comment Position Philosophy

**A formatter should not move a comment to a different syntactic position — unless
the move is lossless and the position carries no authorship signal.** Comment
placement is usually a deliberate authoring choice — it communicates what the comment
refers to — so preserving it is tsv's default and its single largest category of
divergence from Prettier (see [TypeScript: Comments](#typescript-comments)).

Prettier's comment handling is its weakest area. It routinely moves comments from
between syntactic boundaries into adjacent blocks, parens, or other positions, changing
the apparent association — and frequently **losing information** (two comments merging
onto one end-of-line, the second `//` becoming text; or reordering them). tsv treats
comment position as semantic and preserves it wherever that distinction is real.

**Principles:**

1. **Comments between an operator and its operand stay there.** If the user wrote
   `? foo : // about bar`, the comment stays after `:`. Prettier moves it to trailing
   on `foo`, changing its association from the false branch to the true branch.
2. **Trailing comments stay trailing.** `foo // comment` keeps the comment on `foo`.
3. **Same-line block comments stay same-line.** `extends T /* c */ ?` keeps the
   comment after `T`, not moved after `?`.
4. **Both positions are valid when dual-stable.** When the user's chosen position is
   idempotent, preserve it. Don't collapse to one canonical form — that destroys the
   distinction between "comment about X" and "comment about Y".
5. **The deciding test is information loss, not position purity.** Preserve a comment's
   position when relocating it would lose information — the canonical case is Prettier's
   end-of-line relocation *merging* two comments onto one line (the second `//` becomes
   text) or reordering them; tsv keeps them distinct (the name→`=`/`:`/`?` binding
   divergences). But where relocating is **lossless** *and* the position carries no
   signal — a same-line line comment past a *pure separator*, e.g. a list element's
   comma (`A // c⏎, B` → `A, // c`; the comma is structure and the comment trails the
   element either way) — tsv trails like Prettier rather than manufacturing a divergence
   for a meaningless distinction.

**When reviewing comment-related fixes:** Default to preserving position. Match
Prettier's repositioning only when the move is lossless *and* the position carries no
authorship signal (a pure-separator trail), or when the original position is clearly
wrong (e.g., comment inside a token boundary). Otherwise — and whenever relocating
would merge, reorder, or drop a comment — preserve and create a `_prettier_divergence`
fixture.

### Uniform Forced-Continuation Indent

A direct corollary of comment-position preservation, and tsv's most cross-cutting
comment-layout rule. When a **line** comment forces part of a construct onto a new
line — a `//` runs to end-of-line, so whatever the author wrote after it cannot stay
on that line — tsv keeps the comment where it was written and drops the following
token to a continuation line **indented one level**. The continuation then reads as
part of its construct, not as a sibling statement or member.

One rule, applied at every site where a line comment splits a construct's head from
its tail:

- **Declaration and module headers** — keyword→name, keyword→`{`, binding→`from`,
  `*`→`as`, and every other header gap (`import // c⏎{ a } from 'm'`,
  `export // c⏎const x = 1`). See [Declaration- and module-header line-comment
  continuation indent](#comment-relocation).
- **Prefix type operators** — the `keyof`/`typeof` operand hang
  (`type A = keyof // c⏎\t\tB`), shared via `append_keyword_value_line_comments` with
  type-parameter constraint/default values and class-property initializers. See
  [Prefix type-operator operand hang](#comment-relocation).
- **`: Type` annotations** — the colon→type continuation (`prop: // c⏎\tType`), via
  the shared `build_type_annotation_doc`, **uniformly for union, intersection, and
  simple types** and in **every** context: property signatures
  ([annotation_simple](../tests/fixtures/typescript/types/comments/annotation_simple_prettier_divergence/)),
  variable declarations, class properties, function parameters/return types, and
  intersection types
  ([annotation_continuation_indent](../tests/fixtures/typescript/types/comments/annotation_continuation_indent_prettier_divergence/)),
  plus an index signature's key-type
  ([index_signature_key_type_line_comments](../tests/fixtures/typescript/types/type_members/index_signature_key_type_line_comments_svelte_prettier_divergence/))
  and value-type
  ([index_signature_value_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_value_line_comment_prettier_divergence/)).
- **Before-`:` key/binding gap** — the complement of the colon→type case: a line
  comment between a key/binding name (or its `?`/`!` marker) and the `:`
  (`prop // c⏎\t\t: T`) keeps the comment after the marker and indents the whole
  `: type` continuation one level, via the shared `build_marker_colon_line_continuation`.
  Uniform across index signatures
  ([index_signature_key_colon_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_key_colon_line_comment_svelte_prettier_divergence/)),
  property signatures and class properties — key→`:`
  ([key_colon_line_comment](../tests/fixtures/typescript/syntax/comments/key_colon_line_comment_prettier_divergence/))
  and `?`→`:`
  ([optional_marker_line_comment](../tests/fixtures/typescript/syntax/comments/optional_marker_line_comment_prettier_divergence/)),
  variable bindings
  ([binding_key_colon_line_comment](../tests/fixtures/typescript/declarations/variable/binding_key_colon_line_comment_prettier_divergence/)),
  and function parameters
  ([param_key_colon_line_comment](../tests/fixtures/typescript/declarations/function/param_key_colon_line_comment_prettier_divergence/)).
  Prettier keeps the continuation flush — and for property signatures / class
  properties relocates the comment to end-of-line.
- **Index-signature bracket gaps** — the `]`→value-`:` continuation
  (`[k: T] // c⏎\t: V`). See [Index signature `]`→value-`:`](#comment-relocation).

The indent is tsv's own layout choice; prettier handles each site differently — it
relocates the comment (into braces/parens, after `from`/`as`/`;`), floats it past
`;`, keeps the continuation **flush**, or — for a multi-member union after `:` —
also indents. So this rule yields a prettier **match** only where prettier happens to
indent too (`build_type_annotation_doc`'s union case); everywhere else it is a
deliberate `_prettier_divergence`. The payoff is internal consistency: every forced
continuation reads the same regardless of which construct the comment split.

### Print Width Philosophy

**Prettier treats `printWidth` as a soft target.** Lines may exceed it in various edge cases (fill algorithm boundaries, block expressions, template literals, certain constructs that "don't look good" when wrapped).

**tsv treats `printWidth` as a hard limit.** If content exceeds 100 characters, tsv breaks it when possible. This is a deliberate design choice affecting many divergences in this document:

- Block expression conditions (`{#if}`, `{#each}`, etc.) with logical operators
- Template literal interpolations
- Fill algorithm edge cases (101-char boundaries)
- Single-specifier imports
- Various "short expression" tolerances

The benefit: predictable output that respects the configured line length. The tradeoff: some constructs may break where Prettier would keep them inline.

### Tabs-Only Indentation Philosophy

**tsv renders all indentation as whole tabs and never mixes tabs with alignment spaces.**
When breaking a union type, Prettier wraps each member's type doc in
`align(2, …)` (`union-type.js`) to offset the 2-char `|` prefix. With
`--use-tabs`, Prettier's indentation algorithm renders that 2-column offset as
a **sub-tab alignment**: content that is further indented rounds the offset up
to a whole tab, but a closing delimiter sitting at the offset column is emitted
as `tabs + 2 spaces`. The result mixes tabs and spaces on a single indentation
level.

tsv rounds the 2-column offset up to one tab everywhere. At
`tabWidth = 2` the two are the same visual width; only the representation
differs (`⟨n+1 tabs⟩}` vs `⟨n tabs⟩··}`). This keeps indentation
tab-width-agnostic: a reader viewing tabs at any width sees consistent
structure, whereas Prettier's 2-space offset assumes the prefix is exactly 2
columns wide. Cataloged in [Tabs-Only Alignment](#tabs-only-alignment).

---

## Catalog

### CSS: At-Rules

- @container spacing — Spec violation — [container_spacing](../tests/fixtures/css/at_rules/container_spacing_prettier_divergence/)
- @container line wrap — Print width — [container_long](../tests/fixtures/css/at_rules/container_long_prettier_divergence/)
- @import line wrap — Print width — [import_media_query_long](../tests/fixtures/css/at_rules/import_media_query_long_prettier_divergence/)
- @media boolean spacing — Spec violation — [media_boolean_spacing](../tests/fixtures/css/at_rules/media_boolean_spacing_prettier_divergence/)
- @media line wrap — Print width — [media_long](../tests/fixtures/css/at_rules/media_long_prettier_divergence/)
- @scope whitespace — Stable quirk — [scope_complex](../tests/fixtures/css/at_rules/scope_complex_prettier_divergence/)
- @scope newlines — Stable quirk — [scope_selector](../tests/fixtures/css/at_rules/scope_selector_prettier_divergence/)
- @supports line wrap — Print width — [supports_long](../tests/fixtures/css/at_rules/supports_long_prettier_divergence/)
- SCSS directive numbers — Design choice — [scss_directive_number_preserved](../tests/fixtures/css/at_rules/scss_directive_number_preserved_prettier_divergence/)

**Spec violations**: CSS Syntax 3 §4.3.4 specifies that an identifier immediately followed by `(` tokenizes as a `<function-token>`, not as an `<ident-token>` plus `(`. Media Queries 4 §3 explicitly notes: "Whitespace is required between a 'not', 'and', or 'or' keyword and the following '(' character, because without it that would instead parse as a `<function-token>`." Container Queries (CSS Conditional 5) use the same grammar pattern. Prettier normalizes this for `@supports` but not `@media` or `@container`.

**SCSS directive numbers**: SCSS/Sass directives (`@include`, `@mixin`, `@if`, `@for`, `@each`, `@while`, `@function`, `@return`, `@debug`) are not standard CSS. Per CSS Syntax 3 §5.4.2 an unknown at-rule's prelude is consumed as an opaque list of component values with no defined grammar, so tsv preserves it verbatim (e.g. `@include foo(.5s)` stays `.5s`, `@include baz(1.50)` stays `1.50`). Prettier keeps a hardcoded SCSS-directive list whose params it re-parses as a value AST and number-normalizes (`.5s`→`0.5s`, `1.50`→`1.5`). tsv applies number normalization only to contexts whose grammar it parses — declaration values and `@media`/`@supports` preludes (see [CSS: Values](#css-values) "Number dot-ident"); unrecognized directive preludes stay raw. Both outputs are valid CSS (`.5` and `0.5` are the same `<number>` token); the divergence is one of scope, not correctness.

**Media comma-list wrapping (not a divergence)**: a comma-separated `@media` query list (Media Queries 4 §"media query list") that exceeds print width is broken at every top-level comma — one query per line, one indent level — matching Prettier exactly. The `@media line wrap` divergence below applies only to a _single_ `and`-joined query (no comma), which Prettier never wraps but tsv does.

### CSS: Selectors

- Column combinator `||` — Parser compat — [column](../tests/fixtures/css/selectors/combinators/column_prettier_divergence/)
- :nth-child() An+B — Stable quirk — [nth_child](../tests/fixtures/css/selectors/pseudo_class/nth_child_prettier_divergence/)
- Pseudo-args indent (single compound) — Design choice — [compound_args_indent](../tests/fixtures/css/selectors/pseudo_class/compound_args_indent_long_prettier_divergence/)
- Nested pseudo-args indent — Design choice — [nested_where_is](../tests/fixtures/css/selectors/pseudo_class/nested_where_is_long_prettier_divergence/)

**Pseudo-args indent**: When a pseudo-class/element's argument list breaks, prettier indents it an extra level if the enclosing compound's flat `selector-selector` node count exceeds 2 (`shouldIndent = node.nodes.length > 2` in `printer-postcss.js`). That extra `indent` exists to align the continuation lines of a complex selector broken at its combinators; for a *single* compound (no combinator) there is no continuation to align, so it only nests the pseudo args one level deeper than the rule body. tsv keys the indent on combinator presence instead — a complex selector that spans more than one compound indents its continuation (matching prettier), and a pseudo's broken args always indent exactly one level relative to their pseudo. So a combinator-bearing selector matches prettier, while a single compound's pseudo args sit one level in with the `)` aligned to the selector it closes. Uniform, and no node counting. Combinator-bearing case (which matches prettier): [combinators/pseudo_args_long](../tests/fixtures/css/selectors/combinators/pseudo_args_long/).

### CSS: Values

- Ratio in media queries — Stable quirk — [ratio](../tests/fixtures/css/values/ratio/ratio_prettier_divergence/)
- Transform list wrap — Print width — [transform_long](../tests/fixtures/css/values/functions/transform_long_prettier_divergence/)
- Space-separated value wrap — Print width — [space_separated_long_wrap](../tests/fixtures/css/values/lists/space_separated_long_wrap_prettier_divergence/)
- Comma+space value boundary — Print width — [comma_space_separated_long](../tests/fixtures/css/values/lists/comma_space_separated_long_prettier_divergence/)
- Number dot-ident — Spec violation — [number_dot_ident](../tests/fixtures/css/values/numbers/number_dot_ident_prettier_divergence/)
- Block-valued custom prop — Design choice — [block_value](../tests/fixtures/css/values/variables/block_value_svelte_prettier_divergence/)
- Empty custom-prop value — Stable quirk — [empty_value](../tests/fixtures/css/values/variables/empty_value_prettier_divergence/)
- Empty value + `!important` — Prettier bug — [empty_value_important](../tests/fixtures/css/values/variables/empty_value_important_prettier_divergence/)
- var() value-less fallback — Prettier bug — [var_empty_fallback_degenerate](../tests/fixtures/css/values/variables/var_empty_fallback_degenerate_prettier_divergence/)

**Space-separated value wrap**: Prettier doesn't wrap CSS space-separated values (e.g., `box-shadow`) when they exceed print width. A 101-char `box-shadow: var(--a) color-mix(...)` stays on one line. tsv wraps at the print width boundary, breaking between space-separated values. This respects the configured print width rather than allowing arbitrary overflows.

**Comma+space value boundary**: When comma-separated values contain space-separated parts (like multiple `box-shadow` values), Prettier tolerates lines exceeding printWidth. tsv breaks to stay within 100 chars. See `comma_space_separated_long/` for the matching behavior at 100 and 102 chars.

**Number dot-ident**: tsv matches Prettier's number normalization for all valid CSS — scientific-notation exponents (`1E+2`→`1e2`, `5e0`→`5`), trailing/leading zeros (`1.50`→`1.5`, `.5`→`0.5`), and a trailing dot before a terminator (`1.`→`1`, `1.e1`→`1e1`). This applies to declaration values and to `@media`/`@supports` preludes; `@container` preludes are left raw, matching Prettier. The lone divergence is the _invalid_ sequence `<number>.<ident>` (e.g. `1.px`, `1.foo`): Prettier merges it into a dimension (`1px`), but per CSS Syntax 3 §4.3.3 that is three tokens (`<number>` `<delim .>` `<ident>`), not a dimension — so tsv preserves the source verbatim. This only arises in invalid CSS; Prettier itself keeps `url(1.png)` unmerged.

**Block-valued custom property**: A custom property whose entire value is a top-level `{...}` block is valid per CSS Variables Level 1 §2.1 (`<declaration-value>`, any token sequence with balanced brackets) and appears in Prettier's own corpus. Prettier formats the block contents on their own indented lines like a nested rule body (closing `}` on its own line, then `;`); tsv preserves the value as a single opaque single-line expression. Both forms are stable/idempotent under their respective formatters. (Svelte's CSS parser rejects this form outright with `css_expected_identifier`, so this is also a `_svelte_divergence` — see [conformance_svelte.md](./conformance_svelte.md).)

**Empty custom-property value**: An empty custom-property value is valid — `<declaration-value>?` is optional (CSS Variables 1 §"Custom Property Value Syntax"). CSS Syntax 3 §"Consume a declaration" trims leading **and** trailing whitespace from a declaration's value, so `--a:;`, `--a: ;`, and `--a:     ;` all parse to the same empty value; the spacing is not significant. Prettier preserves whatever spacing the source has (multiple stable forms — `prettier_variant_compact`/`prettier_variant_spaces`); tsv normalizes to a single space (`--a: ;`), the form CSS Variables 1 §"Serializing Custom Properties" mandates ("an empty custom property … must serialize with a single space as its value"). Non-custom empty declarations (`color:;`) remain a parse error — a value is required there. A variant — an empty custom-property value carrying `!important` (`--a: !important;`) — exposes a prettier **non-convergence** bug: prettier adds a space before `!important` on every pass (`--a:!important` → `--a: !important` → `--a:  !important` → …) and never reaches a fixed point, so it can't serve as a formatter oracle. tsv normalizes to the single-space form and is idempotent; guarded by [empty_value_important](../tests/fixtures/css/values/variables/empty_value_important_prettier_divergence/), whose `prettier_nonconvergent.txt` marker makes the validator live-verify the non-convergence (rule F5) instead of running the prettier-anchored rules.

**var() value-less fallback (prettier non-idempotency)**: A `var()` whose fallback contains no real token — only commas/whitespace (`var(--a,,)`, `var(--a, ,)`) — collapses to the canonical empty-fallback form `var(--a,)` (CSS Syntax 3 trims the fallback whitespace; a value-less `<declaration-value>?` is empty). This is **not an output divergence** — both formatters reach the same `var(--a,)` fixed point. The difference is normalization speed: tsv reaches it in one pass; prettier is **non-idempotent**, leaving a stray space on pass 1 (`var(--a, )`) that pass 2 removes. Pathological input (never in real CSS); pinned via the `prettier_intermediate_*` fixture so the audit doesn't flag prettier's intermediate form as novel. The valid empty-fallback round-trip — where tsv and prettier agree in one pass — is the regular fixture [var_empty_fallback](../tests/fixtures/css/values/variables/var_empty_fallback/).

### CSS: Layout

**Greedy fill overflow** (print width) — [comma_separated_greedy_fill](../tests/fixtures/css/comma_separated_greedy_fill_prettier_divergence/): Prettier's `fill()` algorithm allows lines to exceed `printWidth` by 1 char when fill segments exactly consume remaining width and the parent adds trailing punctuation. tsv treats `printWidth` as a hard limit.

> **Related fill boundary divergences**: Several fixtures test variations of Prettier allowing lines to exceed `printWidth`. These share a common root cause—Prettier's `fill()` algorithm boundary conditions:
>
> - CSS: `comma_space_separated_long`, `comma_separated_greedy_fill`
> - Svelte: `inline_element_fill_long`, `inline_component_fill_long`, `fill_after_inline`, `block_multiline_attrs_content_hug`, `multiline_value_inline_long`, `fill_expr_break_boundary_long`
> - TypeScript: `long` (template literals)

### CSS: Comments

**Stable quirk** (except where a per-entry reason is noted). Prettier has stable variants for comment positioning. tsv normalizes consistently.

- At-rule before `{` — [atrule_before_opening_brace](../tests/fixtures/css/tokens/comments/atrule_before_opening_brace_prettier_divergence/)
- At-rule in prelude — [atrule_in_prelude](../tests/fixtures/css/tokens/comments/atrule_in_prelude_prettier_divergence/)
- After colon in values — [in_property_value_after_colon](../tests/fixtures/css/tokens/comments/in_property_value_after_colon_prettier_divergence/)
- Before colon in values — [in_property_value_before_colon](../tests/fixtures/css/tokens/comments/in_property_value_before_colon_prettier_divergence/)
- Before colon, comment contains a colon (scan robustness) — [colon_in_property_comment](../tests/fixtures/css/tokens/comments/colon_in_property_comment_prettier_divergence/)
- @media prelude — [media_list](../tests/fixtures/css/tokens/comments/media_list_prettier_divergence/)
- @media long with `/* */` — Print width (the comment is incidental; the divergence is the over-width unwrapped query) — [media_long](../tests/fixtures/css/tokens/comments/media_long_prettier_divergence/)
- Selector before `{` — [selector_before_opening_brace](../tests/fixtures/css/tokens/comments/selector_before_opening_brace_prettier_divergence/)
- Selector before `{` (≥2) — [selector_before_opening_brace_multiple](../tests/fixtures/css/tokens/comments/selector_before_opening_brace_multiple_prettier_divergence/)
- Selector before `{` (in at-rule) — [selector_before_opening_brace_in_atrule](../tests/fixtures/css/tokens/comments/selector_before_opening_brace_in_atrule_prettier_divergence/)
- Selector list — [selector_list](../tests/fixtures/css/tokens/comments/selector_list_prettier_divergence/)
- Selector list (nested `:is()` / before-comma) — [selector_nested_comment](../tests/fixtures/css/tokens/comments/selector_nested_comment_prettier_divergence/)

### Whitespace: BOM Handling

**BOM stripping.** Prettier preserves byte-order marks. tsv strips them (they serve no purpose in UTF-8).

- Svelte — [bom](../tests/fixtures/svelte/syntax/whitespace/bom_prettier_divergence/)
- CSS — [bom](../tests/fixtures/css/tokens/whitespace/bom_prettier_divergence/)
- TypeScript — [bom](../tests/fixtures/typescript/syntax/whitespace/bom_prettier_divergence/)

### Svelte: Elements

- Menu block element — Spec violation — [menu_block](../tests/fixtures/svelte/elements/menu_block_prettier_divergence/)
- Self-closing non-void — Design choice — [self_closing_nonvoid](../tests/fixtures/svelte/elements/self_closing_nonvoid_prettier_divergence/)
- Fill after inline — Print width — [fill_after_inline](../tests/fixtures/svelte/elements/fill_after_inline_prettier_divergence/)
- Fill boundary — Print width — [inline_element_fill_long](../tests/fixtures/svelte/elements/inline_element_fill_long_prettier_divergence/)
- Fill after breaking attr — Print width — [multiline_value_inline_long](../tests/fixtures/svelte/attributes/multiline_value_inline_long_prettier_divergence/)
- Component fill boundary — Print width — [inline_component_fill_long](../tests/fixtures/svelte/elements/inline_component_fill_long_prettier_divergence/)
- Wide inline child own-line — Print width — [inline_component_wide_long](../tests/fixtures/svelte/elements/inline_component_wide_long_prettier_divergence/) (component), [inline_component_wide_longname_long](../tests/fixtures/svelte/elements/inline_component_wide_longname_long_prettier_divergence/) (long tag name), [inline_component_wide_multi_long](../tests/fixtures/svelte/elements/inline_component_wide_multi_long_prettier_divergence/) (two components), [inline_component_wide_multiattr_long](../tests/fixtures/svelte/elements/inline_component_wide_multiattr_long_prettier_divergence/) (multi-attr, breakable inner), [inline_svelte_element_wide_long](../tests/fixtures/svelte/elements/inline_svelte_element_wide_long_prettier_divergence/) (special element), [inline_element_wide_long](../tests/fixtures/svelte/elements/inline_element_wide_long_prettier_divergence/) (HTML element parity), [inline_element_wide_multiattr_long](../tests/fixtures/svelte/elements/inline_element_wide_multiattr_long_prettier_divergence/) (HTML element multi-attr drop coupling)
- Wide inline content + trailing text — Print width / block-style — [inline_wide_content_trailing_long](../tests/fixtures/svelte/elements/inline_wide_content_trailing_long_prettier_divergence/) (wide content lays out **block-style** with both tags intact; space-authored trailing text hugs the intact `</tag>`; `<strong>` + `<a>`), [inline_wide_content_trailing_newline_long](../tests/fixtures/svelte/elements/inline_wide_content_trailing_newline_long_prettier_divergence/) (newline-authored trailing text keeps its own line), [inline_wide_content_text_sibling_long](../tests/fixtures/svelte/elements/inline_wide_content_text_sibling_long_prettier_divergence/) (non-terminal text before a following element keeps its own line), [inline_nested_child_trailing_space_long](../tests/fixtures/svelte/elements/inline_nested_child_trailing_space_long_prettier_divergence/) (nested wide child + trailing text: dual-stable — newline boundary keeps own line, space boundary hugs `</tag>`; both forms stable under tsv and prettier; converging them is the pending follow-up). See [§Svelte: Inline content block-style](#svelte-inline-content-block-style).
- Inline closing intact — Print width — [inline_closing_intact_long](../tests/fixtures/svelte/elements/inline_closing_intact_long_prettier_divergence/)
- Fill multiple expr — Print width — [fill_multiple_expr_long](../tests/fixtures/svelte/elements/fill_multiple_expr_long_prettier_divergence/)
- Inline content (text) — Design choice — [inline_content_text_wrap](../tests/fixtures/svelte/elements/inline_content_text_wrap_prettier_divergence/), [text_non_breaking_whitespace](../tests/fixtures/svelte/elements/text_non_breaking_whitespace_prettier_divergence/) — an inline element's wrapping text content lays out **block-style** (both tags intact, content on its own indented line, collapsing inline when it fits); prettier pre-breaks the opening tag. See [§Svelte: Inline content block-style](#svelte-inline-content-block-style).
- Inline content (expression) — Design choice — [inline_content_hug_long](../tests/fixtures/svelte/elements/inline_content_hug_long_prettier_divergence/) — breakable-expression content also lays out **block-style** (uniform with text/element content), where prettier dangles. See [§Svelte: Inline content block-style](#svelte-inline-content-block-style).
- Block multiline attrs hug — Print width — [block_multiline_attrs_content_hug](../tests/fixtures/svelte/elements/block_multiline_attrs_content_hug_prettier_divergence/)
- Fill expr break boundary — Print width — [fill_expr_break_boundary_long](../tests/fixtures/svelte/elements/fill_expr_break_boundary_long_prettier_divergence/)
- @debug comments — Content preservation — [debug_comment](../tests/fixtures/svelte/tags/debug/debug_comment_prettier_divergence/)
- svelte:element `this` — Prettier bug — [svelte_element_this_string](../tests/fixtures/svelte/special_elements/svelte_element_this_string_prettier_divergence/)
- svelte:element class ws — Prettier bug — [svelte_element_class_whitespace](../tests/fixtures/svelte/special_elements/svelte_element_class_whitespace_prettier_divergence/)
- Space after block element — Prettier bug — [space_after_block](../tests/fixtures/svelte/elements/space_after_block_prettier_divergence/)

**Fill after inline**: Prettier's fill algorithm allows lines to exceed print width when text follows an inline element closing tag. Prettier produces 111 char lines, tsv breaks at exactly 100 chars.

**Fill boundary**: When fill content exceeds print width, Prettier tolerates the overage while tsv breaks earlier. This is an emergent behavior from prettier-plugin-svelte's doc structure (separate fills per text node with `group([line, element])` wrappers). See also `inline_element_fill_100/` which shows both formatters match at exactly 100 chars.

**Fill after breaking attr**: When an inline element has breaking attributes (e.g., multiline values) and long trailing text follows, Prettier's `handleTextChild` early return for last-child text skips wrapping the previous element, so there's no break point between the closing tag and trailing text. Prettier allows the line to exceed print width (102 chars). tsv's fill correctly breaks trailing words at the print width boundary (100 chars). See also `multiline_value_inline/` which shows both formatters match when trailing text is short.

**Component fill boundary**: Same fill boundary behavior as above but with component elements (`<Comp>text</Comp>`) instead of HTML inline elements. At 101 chars, Prettier keeps everything on one line while tsv breaks the closing `>` of the closing tag to stay within print width. At 100 chars both formatters match.

**Wide inline child own-line**: When an inline child (component or HTML element) is too wide to share a line with the preceding text, Prettier hugs it onto the text line (101+) and breaks it internally — attributes wrap and the closing `>` dangles (the inline content hug). tsv keeps printWidth a hard limit, so the whole child drops to its own line and the preceding word stays hugged on the text line. The break sits at the collapsible space before the child, so it holds regardless of tag-name length (`inline_component_wide_longname_long`), for repeated children in one run (`inline_component_wide_multi_long`), for a multi-attribute component whose own attributes could break (`inline_component_wide_multiattr_long` — it still drops whole, attributes intact), and for special elements (`inline_svelte_element_wide_long`); HTML inline elements produce the identical shape (`inline_element_wide_long`, and `inline_element_wide_multiattr_long` for the multi-attribute counterpart). When a dropped child is followed by trailing text, that text takes its own line too — a dropped child owns its line (`inline_component_wide_multiattr_long` / `inline_element_wide_multiattr_long`, regardless of whether the drop comes from the child's own content overflowing or from the preceding text being too long). These fixtures also pin idempotence — both the compact authored form and Prettier's hugged form must normalize to the own-line form in one pass.

**Wide inline content + trailing text**: The mirror of the above for an element whose own _content_ (not its attributes) overflows, followed by trailing text — `<strong>…90 chars…</strong> tail`. Prettier keeps the content on a single over-width line and lets `>…content…</tag` exceed printWidth (the inline content hug again); tsv wraps the content _inside_ the element, so every line stays ≤100. The trailing text's placement follows the **authored boundary**: a space hugs the intact `</tag>` (`</tag> tail` — `inline_wide_content_trailing_long`, covering both `<strong>` with no attributes and `<a>` with a short one), a newline keeps it on its own line (`inline_wide_content_trailing_newline_long`). This mirrors how a _short_ inline element already keeps `<el>x</el> tail` inline for a space and breaks for a newline — tsv treats the boundary whitespace before trailing text as a meaningful authoring choice rather than normalizing it away. Prettier's tail placement is the same on both forms (space hugs, newline breaks), so the **content wrap is the sole divergence** on each. The `unformatted_ours_*` variants pin tsv's idempotence: differently-authored space forms all normalize to the hugged form in one pass. A non-terminal text run (followed by another inline element) instead keeps its own line regardless of authoring — hugging it is non-convergent (`inline_wide_content_text_sibling_long`). This boundary-respecting choice does not yet apply uniformly: the nested wide child (`inline_nested_child_trailing_space_long`) is now dual-stable (a newline boundary keeps its own line, a space boundary hugs `</tag>`), while the dropped-child case (`inline_component_wide_multiattr_long`) still takes its own line — fully converging these is ongoing inline-content layout work.

**Fill expr break boundary**: When fill content includes a multiline expression (e.g., binary `+` that breaks across lines), subsequent text continues on the continuation line. At the width boundary, Prettier allows the continuation line to reach 101 chars while tsv breaks at 96 to stay under 100. See also `fill_expr_break_continuation_long/` for matching behavior when continuation stays under 100.

**Fill multiple expr**: When fill mode content has multiple expression tags with ternaries, Prettier breaks the opening bracket and packs more onto a single line (101 chars), breaking within the comparison operator (`!==`). tsv keeps the opening bracket hugging content and breaks earlier at the first ternary operator (`?`) to stay within 100 chars (max 79).

**Inline content (expression)**: When inline element content with _breakable_ expressions (ternaries, `&&`/`||`, `+`/`-`) exceeds print width, tsv lays it out **block-style** — both tags intact, the content on its own indented line, where the expression usually fits unbroken (and wraps within printWidth only if still too wide). Prettier instead dangles the tag delimiters and breaks the expression operator-by-operator at the tighter dangled indent. This is uniform with text and element content; see §Svelte: Inline content block-style.

**Menu block element**: prettier-plugin-svelte's `blockElements` list includes `ol` and `ul` but omits `menu`. The HTML spec treats `<menu>` identically to `<ul>` — same `display: block`, same CSS rules (`padding-inline-start: 40px`, `counter-reset: list-item`). The spec explicitly says: _"The `menu` element is simply a semantic alternative to `ul`."_ tsv includes `<menu>` in the block element list (spec-compliant), causing block formatting (expanded content) where Prettier hugs content. This only manifests when compact input is formatted — both formatters preserve the block form if given it directly.

**Block multiline attrs hug**: When whitespace-sensitive elements (`<pre>`, `<textarea>`) have multiline attributes and hugged content that would exceed print width, Prettier keeps `>{content}</tag>` on the attribute line (101+ chars). tsv breaks `>` to its own line (`\n>{content}`) to respect print width while preserving whitespace semantics (no text node added since `>` immediately precedes content).

**svelte:element `this`**: prettier-plugin-svelte 4.x ignores `singleQuote` for a brace-wrapped string literal in `<svelte:element this={…}>`, always emitting double quotes (`this={'hello'}` → `this={"hello"}`), and skips escaping entirely — so `this={'a"b'}` becomes the invalid `this={"a"b"}` and `this={'a\b'}` corrupts to a backspace. tsv delegates the literal to the normal string printer, honoring `singleQuote` and escaping like any other string. The bug is narrow — only a *directly* brace-wrapped string literal triggers it; concatenations, template literals, and other expressions delegate to the JS printer and agree (boundary fixture [svelte_element_this](../tests/fixtures/svelte/special_elements/svelte_element_this/)). One adjacent facet diverges further: a *parenthesized* literal `this={('hello')}` collapses to the plain attribute `this="hello"` under prettier (structural rewrite), while tsv keeps `this={'hello'}` — encoded by the fixture's `unformatted_ours_paren` + `variant_paren_collapse` pair. A fix restoring JS-printer delegation has been prepared for prettier-plugin-svelte; once released and re-pinned, this divergence retires.

**svelte:element class ws**: prettier-plugin-svelte 4.x no longer collapses repeated whitespace in a `class` attribute on `<svelte:element>` (`class="a   b    c"` stays verbatim), while it still collapses on a plain element (`<div class="a   b    c">` → `class="a b c"`) — and so does tsv, everywhere. The `<svelte:element>` attribute path regressed off the normal attribute printer in the same 4.x modern-ast migration as the `this` bug (3.5.2 collapsed here too). tsv collapses uniformly across all elements. Both formatters keep the single-spaced form stable; the divergence shows only when collapsing multi-space input. Retires once a plugin fix releases and tsv's oracle is re-pinned.

**Space after block element**: When a block element (`<div>`, `<p>`, …) is directly followed by content text with a same-line (space, no linebreak) boundary, prettier-plugin-svelte trims the text's leading whitespace (`trimTextNodeLeft`) but still emits the block-child break, stranding a **leading space** on the text line (`<div>block</div> text` → `>⏎ text`). It is **non-idempotent** — a second pass trims the space, converging to the same form tsv produces. tsv takes the same trim but emits no fold/group after the block (the block's own break already supplies the separating line), so no stray space survives and the result is identical in one pass from either authoring. The `unformatted_ours_compact` variant pins tsv's one-pass normalization; `prettier_intermediate_compact` captures prettier's stray-space first pass (which converges to `input` on the next pass).

### Svelte: Inline content block-style

Design choice. tsv lays out an inline element's **wrapping content** block-style — both tags stay intact and the content moves to its own indented line(s), collapsing to `<tag>content</tag>` when it fits, exactly like a block element. Prettier instead **dangles** the tag delimiters (`<tag⏎\t>content</tag⏎>`, pre-breaking the opening tag and dangling the closing `>`). This is uniform across text, expression, and element/component children, with and without attributes (the `>` is **attr-keyed** — it hugs the last attribute when attributes fit and dedents to its own line when they wrap), and including table cells (which are inline-classified). Content that fits stays inline; only overflow breaks to block-style.

Content-boundary whitespace is **render-free under Svelte 5** — start/end-of-content whitespace is trimmed at compile (`<p>foo<span> - bar</span></p>` renders `foo- bar`), so the injected block-style boundaries are render-equivalent (confirmed at corpus scale by `ast_diff --render`). Both formatters keep the block-style form once produced (`prettier(input) == input`); the divergence is that tsv *converges every authoring* to block-style while prettier dangles a compact authoring — so these fixtures carry `unformatted_ours_*` compact variants (tsv normalizes them to the block-style input, prettier does not) alongside `prettier_variant_*` files pinning prettier's stable dangle form (which tsv likewise normalizes to the block-style input). This supersedes the former per-case opening-attach / closing-`>` dangle behavior (see also the updated "Inline content (text)", "Inline content (expression)", and "Wide inline content + trailing text" rows above).

- elements — [inline_before_block_break_prettier_divergence](../tests/fixtures/svelte/elements/inline_before_block_break_prettier_divergence/), [table_cell_hug_long_prettier_divergence](../tests/fixtures/svelte/elements/table_cell_hug_long_prettier_divergence/), [member_expr_break_prettier_divergence](../tests/fixtures/svelte/elements/member_expr_break_prettier_divergence/), [member_expr_break_order_prettier_divergence](../tests/fixtures/svelte/elements/member_expr_break_order_prettier_divergence/), [member_expr_break_order_long_prettier_divergence](../tests/fixtures/svelte/elements/member_expr_break_order_long_prettier_divergence/), [inline_with_if_block_prettier_divergence](../tests/fixtures/svelte/elements/inline_with_if_block_prettier_divergence/), [inline_with_if_block_multiline_prettier_divergence](../tests/fixtures/svelte/elements/inline_with_if_block_multiline_prettier_divergence/), [inline_if_content_block_style_prettier_divergence](../tests/fixtures/svelte/elements/inline_if_content_block_style_prettier_divergence/), [inline_nested_else_if_block_style_prettier_divergence](../tests/fixtures/svelte/elements/inline_nested_else_if_block_style_prettier_divergence/), [inline_nbsp_boundary_long_prettier_divergence](../tests/fixtures/svelte/elements/inline_nbsp_boundary_long_prettier_divergence/), [inline_nested_child_trailing_long_prettier_divergence](../tests/fixtures/svelte/elements/inline_nested_child_trailing_long_prettier_divergence/), [inline_content_unbreakable_prettier_divergence](../tests/fixtures/svelte/elements/inline_content_unbreakable_prettier_divergence/), [inline_nested_wrap_long_prettier_divergence](../tests/fixtures/svelte/elements/inline_nested_wrap_long_prettier_divergence/) (nested inline elements both wrap block-style; prettier double-pyramids)
- components — [compact_block_children_prettier_divergence](../tests/fixtures/svelte/components/compact_block_children_prettier_divergence/), [compact_multiple_blocks_prettier_divergence](../tests/fixtures/svelte/components/compact_multiple_blocks_prettier_divergence/), [inline_parent_blocks_prettier_divergence](../tests/fixtures/svelte/components/inline_parent_blocks_prettier_divergence/), [text_child_long_prettier_divergence](../tests/fixtures/svelte/components/text_child_long_prettier_divergence/) (component body long inline text wraps block-style; prettier dangles)
- blocks — [nested_expanding_prettier_divergence](../tests/fixtures/svelte/blocks/await/nested_expanding_prettier_divergence/), [inline_element_boundary_long_prettier_divergence](../tests/fixtures/svelte/blocks/await/inline_element_boundary_long_prettier_divergence/), [snippet/body_inline_content_prettier_divergence](../tests/fixtures/svelte/blocks/snippet/body_inline_content_prettier_divergence/) (snippet body inline content lays out block-style; prettier dangles)
- attributes — [multiline_value_inline_prettier_divergence](../tests/fixtures/svelte/attributes/multiline_value_inline_prettier_divergence/), [directive_expr_logical_long_prettier_divergence](../tests/fixtures/svelte/attributes/directive_expr_logical_long_prettier_divergence/)
- directives — [on/long_prettier_divergence](../tests/fixtures/svelte/directives/on/long_prettier_divergence/)

### Svelte: Attributes

**Trailing comments in `{...}`** (content preservation) — [expr_trailing](../tests/fixtures/svelte/syntax/comments/expr_trailing_prettier_divergence/) (block comments, inline); [expr_trailing_line](../tests/fixtures/svelte/syntax/comments/expr_trailing_line_prettier_divergence/) (line comments — `}` kept on its own line so the `//` doesn't swallow it).

**Leading/interior comments in a `bind:` function-binding sequence** (content preservation) — `bind:value={getter, setter}` carries getter/setter expressions as a bare (parens-stripped) sequence; tsv preserves a comment at the author's position where prettier kept it but tsv used to drop it. A leading line or multi-line block comment, mid (between getter/setter) block, and mid line comment all match prettier (regular fixture [function_comment](../tests/fixtures/svelte/directives/bind/function_comment/)). A **single-line block** comment *leading* the sequence is the one divergence — [function_comment_inline_block](../tests/fixtures/svelte/directives/bind/function_comment_inline_block_prettier_divergence/): prettier re-parenthesizes it (`{/* c */ (a, b)}`) then drops the comment on the next pass (non-idempotent), so tsv keeps the sequence bare and the comment in place. Trailing comments after the last operand are dropped by both. See [Comment Position Philosophy](#comment-position-philosophy).

**Same-line `//` comment placement in the attribute list** (comment preservation) — [comment_same_line](../tests/fixtures/svelte/attributes/comment_same_line_prettier_divergence/): a line comment the author put on the same line as the tag name (`<div // foo`) or trailing an attribute that has more attributes after it (`a="1" // mid`) stays trailing that token; prettier relocates it to its own line. A `//` trailing the *last* attribute (before `>`/`/>`) already stays inline in both formatters, so it is not a divergence — [comment_trailing_same_line](../tests/fixtures/svelte/attributes/comment_trailing_same_line/). Block comments and own-line comments are preserved as-written by both. See [Comment Position Philosophy](#comment-position-philosophy).

**Unofficial directive modifiers** (content preservation) — a directive type *without* official modifier support — `use:` / `bind:` / `class:` / `animate:` / `let:` — carrying a `|modifier` (`use:action|mod`). Svelte's permissive parser records the trailing text in `modifiers`, and tsv's parser AST matches it exactly (so this is **not** a `_svelte_divergence`); but prettier-plugin-svelte silently *drops* the text on format, while tsv preserves it verbatim — [modifier_preservation](../tests/fixtures/svelte/directives/modifier_preservation_prettier_divergence/). A `|modifier` is semantics-bearing source, not whitespace, so deleting it is content loss. The three officially-supporting types (`on:` / `transition:` / `style:`) keep their modifiers in both formatters; tsv emits and prints modifiers uniformly across all eight directive types. See [conformance_svelte.md §Directive Modifiers](./conformance_svelte.md#directive-modifiers).

### Svelte: Blocks

Standalone head wrap + dangle + body-expand:

- `{#each}` — [each_long](../tests/fixtures/svelte/blocks/each/long_prettier_divergence/)
- `{#await}` — [await_long](../tests/fixtures/svelte/blocks/await/long_prettier_divergence/)
- `{#key}` — [key_long](../tests/fixtures/svelte/blocks/key/long_prettier_divergence/)
- `{#if}` (binary / member chain / call / `{:else if}`) — [if_long](../tests/fixtures/svelte/blocks/if/long_prettier_divergence/)
- `{#if}` last block quirk — [last_block](../tests/fixtures/svelte/blocks/if/last_block_prettier_divergence/)

Same layout inside an inline element (head wraps + body expands, element hugs the outer boundary):

- `{#if}` — [if/inline_element_long](../tests/fixtures/svelte/blocks/if/inline_element_long_prettier_divergence/)
- `{#each}` — [each/inline_element_long](../tests/fixtures/svelte/blocks/each/inline_element_long_prettier_divergence/)
- `{#key}` — [key/inline_element_long](../tests/fixtures/svelte/blocks/key/inline_element_long_prettier_divergence/)
- `{#await}` — [await/inline_element_long](../tests/fixtures/svelte/blocks/await/inline_element_long_prettier_divergence/)
- `{#snippet}` (params inline + body expand vs prettier's param-wrap) — [snippet/inline_element_long](../tests/fixtures/svelte/blocks/snippet/inline_element_long_prettier_divergence/)
- `{#snippet}` standalone, **flush body** (tsv keeps params inline + body on its own line; prettier wraps the params just to keep the body hugging `)}`) — [snippet/flush_body_param_wrap_prettier_divergence](../tests/fixtures/svelte/blocks/snippet/flush_body_param_wrap_prettier_divergence/)

**Print width.** Prettier doesn't apply width-based wrapping to block expressions at all:

- **Method chains** in `{#each}`, `{#await}`, `{#if}`, `{#key}`, etc. don't account for the tag prefix width, resulting in 140+ char lines. tsv passes context offset for proper wrapping.
- **Logical expressions** (`&&`, `||`) in block conditions are never wrapped internally by Prettier, even when exceeding 100 chars. (In `<script>`, assignments provide a break point so this isn't an issue.) tsv wraps them with proper indentation.
- **Function calls** in block expressions don't wrap their arguments. tsv wraps function arguments when they exceed print width.

**Head `}` dangle + clause hug.** When a block head wraps, tsv drops the closing `}` (and any `as item` / `then value` clause) to its own line at the tag's base indent — `{#if a &&⏎\t…⏎}`, `{#each …⏎as item}` — consistent with tsv's JS `if (⏎…⏎) {` and its broken-element `>` (`bracketSameLine: false`). The one shape that hugs is a single call/`new` whose arguments wrapped: its `)` already dedents to base, so the clause + `}` continue on it (`) as item}`, `)}`, `) then r}`). Prettier never wraps a block head, so it never faces this.

**Body-expand (whole construct goes multiline).** When the head wraps — or the inline construct simply exceeds printWidth — tsv expands the **entire** block: the body, every `{:then}` / `{:catch}` / `{:else}` / `{:else if}` section/branch, and the `{/tag}` close each drop to their own indented lines, uniformly across `{#if}` / `{#each}` / `{#await}` / `{#key}` / `{#snippet}`. This holds **inside inline elements/components too** (`<span>{#if …}…{/if}</span>`) — block-body boundary whitespace is render-non-significant there (verified against the Svelte compiler), so it is safe; the only gate is `<pre>` / `white-space:pre`, where the drop is suppressed and the body stays on the line (both formatters agree — [elements/pre_block_body_long](../tests/fixtures/svelte/elements/pre_block_body_long/)). Inside `<pre>` a nested element's wrapped attributes and close token indent off its nesting depth — one level per enclosing container, the same model as prettier ([elements/pre_nested_attr_indent](../tests/fixtures/svelte/elements/pre_nested_attr_indent/)) — and the source close form is preserved (self-closing `/>` vs explicit `></tag>`), for components and HTML inline elements alike. One residual divergence remains there: an empty self-closing element whose attributes fit within printWidth (counting the preserved-text prefix on the line) keeps them on one line with only `/>` dropping, where prettier full-breaks them regardless — [elements/pre_block_empty_element](../tests/fixtures/svelte/elements/pre_block_empty_element_prettier_divergence/). Prettier keeps the construct inline past printWidth (only the enclosing element wraps).

**Middle zone (head fits alone, head + body doesn't).** tsv decouples the head-wrap decision (head-alone width) from the body-expand decision (head + body width): when the head fits on its own line but the whole construct overflows, the head stays flat and only the body expands. This is chosen in one pass (no wrap-then-unwrap across two formats), so every layout is an idempotent fixed point.

**Uniform body drop (no breakable-element hug).** When an inline-authored block body overflows, tsv drops it to its own indented line **uniformly — for every body shape**: text, expression tags (`{x}`), void/empty elements (`<Spinner />`, `<Comp></Comp>`), and elements/components **with attributes or children** alike. Prettier instead leaves the body hugging the `}` and breaks it *internally* (an element wraps its attributes / closing `>`; text just overflows) — prettier's block-body layout is driven by authored boundary whitespace, never by width. tsv's uniform drop keeps the layout idempotent (a one-pass `conditional_group`, no special case keyed on whether the body can break internally) and consistent across all body shapes and contexts (block level, inside inline elements/components, in any section/branch). The earlier breakable-element hug was removed: as a non-first body node (behind leading text, a comment, a void sibling, or an atomic `{:else}` branch) it over-wrapped the head and was non-idempotent across two passes. Cataloged at [if/element_body_long](../tests/fixtures/svelte/blocks/if/element_body_long_prettier_divergence/) and [each/element_body_long](../tests/fixtures/svelte/blocks/each/element_body_long_prettier_divergence/) (block level, at the 100/101 boundary), [await/element_body_long](../tests/fixtures/svelte/blocks/await/element_body_long_prettier_divergence/) and [snippet/element_body_long](../tests/fixtures/svelte/blocks/snippet/element_body_long_prettier_divergence/) (inside `<Container>`), [await/full_form_element_body_long](../tests/fixtures/svelte/blocks/await/full_form_element_body_long_prettier_divergence/) (the full `{:then}`/`{:catch}` form, each section body drops), [key/void_element_body_long](../tests/fixtures/svelte/blocks/key/void_element_body_long_prettier_divergence/) (void body + an attributed body that now drops the same way), [elements/inline_if_sibling_fill_long](../tests/fixtures/svelte/elements/inline_if_sibling_fill_long_prettier_divergence/) (body drop next to an inline sibling), [elements/inline_component_else_body_long](../tests/fixtures/svelte/elements/inline_component_else_body_long_prettier_divergence/) (breakable element in `{:else}`, behind an atomic consequent), [if/element_body_leading_node_long](../tests/fixtures/svelte/blocks/if/element_body_leading_node_long_prettier_divergence/) (non-first body node — leading text, a comment, or a void element before the breakable element), and [if/element_body_deep_nested](../tests/fixtures/svelte/blocks/if/element_body_deep_nested_prettier_divergence/) (deep nesting — the dropped element re-wraps at the body indent). Paramless `{#snippet}` bodies drop the same way once the whole construct still overflows after the element boundary wraps — [snippet/inline_element_long](../tests/fixtures/svelte/blocks/snippet/inline_element_long_prettier_divergence/). The drop reads correctly embedded in realistic nested context with inline siblings (incl. the breadcrumb shape that motivated removing the hug) — [elements/block_body_drop_nested_siblings](../tests/fixtures/svelte/elements/block_body_drop_nested_siblings_prettier_divergence/).

**Sibling `>` dangle (axis 3).** When an inline element is immediately followed — no whitespace — by a block that **renders multiline**, tsv dangles that element's **closing `>`** onto the block-head line (`<a href={root}>Home</a⏎>{#each …}`). This applies to **all five block heads** — `{#if}` / `{#each}` / `{#key}` / `{#await}` / `{#snippet}`. It generalizes the rule tsv already applies from the enclosing-element side: the `>` token immediately preceding a block's `{#…}` dangles, whether it's the *opening* `>` of a parent inline element holding a sole-content block (`<span⏎>{#if …}`) or the *closing* `>` of a preceding inline sibling. The `>` moves only *inside* the closing tag (`</a⏎>`), so the boundary whitespace is tag-internal and the output parses to a byte-identical AST — render-safe (verified against the Svelte compiler), the same property as the element boundary-`>` wrap. It is **strict / multiline-only**: a short block that stays inline keeps the `>` hugged (`<span>text</span>{#if c}text{/if}`), and the decision keys on whether the block actually renders multiline — not on whether its body is authored inline or on its own line — so the layout is a fixed point on its own output. It is **leading-side only**: the closing side (`{/if}<span>`) keeps flowing. Text/expression siblings (`wide text {#if…}`, `{x}{#if…}`) keep hugging — there is no `>` to dangle and the boundary whitespace is render-fixed, an asymmetry that is forced, not a tsv inconsistency. Prettier never expands the block at all, so it always keeps the boundary hugged. **`{#await}` / `{#snippet}` parity:** unlike `{#if}` / `{#each}` / `{#key}`, await/snippet don't force their block parent multiline on their own (a lone `<div>{#await p}x{/await}</div>` stays inline, matching prettier) — they go multiline only once they follow a **breakable** sibling: an inline element / component / expression-or-`{@html}`-or-`{@render}` tag (the `has_preceding_breakable` set, shared with the body-drop). There the parent breaks so the dangle, the block-on-own-line separation for a block sibling, and the body-drop all resolve in one pass (their body-drop keys on `can_wrap`, the same gate the other heads use). After a **non-breakable** sibling — plain text or a comment — they stay **inline**, matching prettier; the force is additionally gated on `kind.is_block()`, so an inline element or component parent never breaks for *any* sibling. This is a distinct axis from the `>` dangle above: a breakable sibling breaks the *parent* even when the short block keeps its own `>` hugged, and that break is itself a divergence (prettier keeps the short inline-authored construct inline). Cataloged at [elements/inline_sibling_gt_dangle](../tests/fixtures/svelte/elements/inline_sibling_gt_dangle_prettier_divergence/) (the dangle for all five heads + a no-dangle control), [elements/await_snippet_breakable_sibling](../tests/fixtures/svelte/elements/await_snippet_breakable_sibling_prettier_divergence/) (the short-case parent break after a breakable sibling), [elements/await_snippet_nonbreakable_sibling_inline](../tests/fixtures/svelte/elements/await_snippet_nonbreakable_sibling_inline/) (the non-breakable text/comment counterpart that stays inline, matching prettier), [components/await_snippet_sibling_inline](../tests/fixtures/svelte/components/await_snippet_sibling_inline/) (a component parent stays inline for any sibling, incl. `{@const}` / `{@debug}`), [blocks/await/preceding_sibling_body_long](../tests/fixtures/svelte/blocks/await/preceding_sibling_body_long_prettier_divergence/) (await body-drop after a non-danglable expression-tag sibling), [elements/block_body_drop_nested_siblings](../tests/fixtures/svelte/elements/block_body_drop_nested_siblings_prettier_divergence/) (the breadcrumb `</a>` dangle in realistic context), and [elements/inline_if_sibling_fill_long](../tests/fixtures/svelte/elements/inline_if_sibling_fill_long_prettier_divergence/) (the `</span>` dangle in a fill).

**Last block not expanded**: Prettier expands `{#if a} content {/if}` (symmetric spaces) to multiline, but has a quirk: the last block in a file stays inline. A single block appears preserved only because it's last. tsv expands consistently regardless of position.

### Svelte: destructuring literal normalization

**Design choice.** tsv routes the binding patterns of `{#each … as}`, `{#await … then}`, `{:then}`, and `{:catch}` through its TypeScript printer, so **literal default values** normalize to tsv's canonical form — string literals to single quotes (`{ a = "x" }` → `{ a = 'x' }`, with escape-minimizing keeping double quotes when single would need escaping, so `"a'b"` stays double) and numeric literals to canonical shape (lowercase hex/exponent, leading/trailing-zero rules: `0xFF` → `0xff`, `1.50` → `1.5`, `.5` → `0.5`, `1E10` → `1e10`, `0xFFn` → `0xffn`). This is the same normalization `{@const}` (and every other literal tsv emits) already applies. prettier-plugin-svelte instead prints these patterns from raw source, preserving the author's quote style and numeric token verbatim — in these binding positions it ignores `singleQuote` and numeric normalization. tsv normalizes uniformly so a destructuring default reads the same wherever it appears. Booleans, `null`, and regex literals are already canonical in both formatters and are unaffected.

- `{#each as { x = … }}` strings + numbers — [destructure_literal_default](../tests/fixtures/svelte/blocks/each/destructure_literal_default_prettier_divergence/)
- `{#await then}` / `{:then}` / `{:catch}` strings + numbers — [destructure_literal_default](../tests/fixtures/svelte/blocks/await/destructure_literal_default_prettier_divergence/)

### Svelte: destructuring rename-with-default key drop

**Prettier bug.** A renamed (non-shorthand) destructuring property that carries a **default value** loses its source key in prettier-plugin-svelte's binding-pattern printer for `{#each … as}`, `{#await … then}`, `{:then}`, and `{:catch}`. `{ a: b = 1 }` (read property `a`, bind to `b`) prints as `{ b = 1 }` (read property `b`) — a **semantic change**, since the output reads a different source property. The bug is specific to a non-shorthand property whose value is an `AssignmentPattern`: plain renames without a default (`{ a: b }`) print correctly, only the defaulted property in a list loses its key (`{ a: b = 1, c: d }` → `{ b = 1, c: d }`), and a nested pattern with a default drops its key the same way (`{ a: { b } = c }` → `{ { b } = c }`). prettier's wrong output is itself stable. tsv prints these patterns through its TypeScript printer, which preserves the key in every case.

- `{#each as}` rename + default, sibling, nested — [destructure_rename_default](../tests/fixtures/svelte/blocks/each/destructure_rename_default_prettier_divergence/)
- `{#await then}` / `{:then}` / `{:catch}` rename + default — [destructure_rename_default](../tests/fixtures/svelte/blocks/await/destructure_rename_default_prettier_divergence/)

### Svelte: destructuring binding-pattern comments

**Content preservation.** A comment placed **inside** a destructuring binding pattern of `{#each … as}`, `{#await … then}`, `{:then}`, or `{:catch}` is preserved where the author wrote it; prettier-plugin-svelte prints these patterns from a comment-blind path and silently drops it. tsv routes the pattern through a comment-aware printer (the same canonical positions it preserves for a regular TypeScript destructure, e.g. `const { a = /* c */ 1 } = x`), so a destructuring comment reads the same wherever it appears. Covered (block comments, pattern stays inline): object default value (`{ a = /* c */ 1 }`), leading after `{` (`{ /* c */ a }`), trailing before `}` (`{ a /* c */ }`), rename `key:`→value gap (`{ a: /* c */ b }`), array element (`[a /* c */]`), rest binding (`[.../* c */ rest]`), and nested patterns. Per [Comment Position Philosophy](#comment-position-philosophy), user intent is preserved when prettier moves or drops comments. (An interior comment in the `{#await … then}` *shorthand* pattern was additionally mis-relocated to trail the awaited expression — the expression's trailing-comment range now stops at the pattern.) These are `_svelte_prettier_divergence` fixtures: acorn attaches the comment to the pattern node (`leadingComments`/`trailingComments`) where tsv's detached model does not — see [conformance_svelte.md §Comment Attachment Differences](./conformance_svelte.md#comment-attachment-differences).

- `{#each as { … }}` object/array/rest/nested positions — [destructure_comment](../tests/fixtures/svelte/blocks/each/destructure_comment_svelte_prettier_divergence/)
- `{#await then}` / `{:then}` / `{:catch}` — [destructure_comment](../tests/fixtures/svelte/blocks/await/destructure_comment_svelte_prettier_divergence/)

### Svelte: empty destructuring brace spacing

**Design choice.** Non-empty object-destructure patterns in `{#each … as}`, `{#await … then}`, `{:then}`, and `{:catch}` binding positions space their braces in both formatters (`{a}` → `{ a }`), matching prettier-plugin-svelte under `bracketSpacing`. The lone remaining divergence is the **empty** pattern: tsv keeps tight braces (`{}`), prettier-plugin-svelte inserts a space (`{ }`). tsv's empty object braces stay tight everywhere — `bracketSpacing` only spaces braces around content, and an empty pattern has none — so this binding position follows the same universal empty-braces rule as an empty object literal (`{}`) and a TypeScript empty destructure (`const {} = x`, where both formatters already agree on `{}`).

- `{#each as {}}` empty pattern — [destructure_empty](../tests/fixtures/svelte/blocks/each/destructure_empty_prettier_divergence/)

### TypeScript

- Empty statement blank lines — Design choice — [empty_standalone](../tests/fixtures/typescript/statements/empty_standalone_prettier_divergence/)
- Return type generic union — Print width — [return_type_generic_union_long](../tests/fixtures/typescript/declarations/function/return_type_generic_union_long_prettier_divergence/)
- Single specifier import — Print width — [single_specifier_long](../tests/fixtures/typescript/modules/imports/single_specifier_long_prettier_divergence/)
- Module path calls — Print width — [path_calls_long](../tests/fixtures/typescript/modules/imports/path_calls_long_prettier_divergence/)
- Instantiation expression parens — Semantic preservation — [instantiation_parens](../tests/fixtures/typescript/typescript_specific/assertions/instantiation_parens_prettier_divergence/)
- Optional-chain base member chain — Semantic preservation — [optional_paren_member_chain](../tests/fixtures/typescript/expressions/chain/optional_paren_member_chain_prettier_divergence/)
- Optional-chain non-null base member chain — Semantic preservation — [optional_paren_non_null_member_chain](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_member_chain_prettier_divergence/)
- Optional-chain non-null new callee — Prettier bug — [optional_paren_non_null_new_callee](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_new_callee_prettier_divergence/)
- Non-null parenthesized base — Design choice — [non_null_paren_base_long](../tests/fixtures/typescript/expressions/member/non_null_paren_base_long_prettier_divergence/)
- Constrained infer extends-operand parens — Prettier bug — [constrained_extends_parens](../tests/fixtures/typescript/types/infer/constrained_extends_parens_prettier_divergence/)
- Arrow type param trailing comma — Design choice — [single_type_param](../tests/fixtures/typescript/expressions/arrow/generic/single_type_param_prettier_divergence/)

**Instantiation expression parens**: Prettier strips parentheses from ternary and binary expressions in `TSInstantiationExpression` (`(x ? y : z)<T>` → `x ? y : z<T>`), changing semantics. Without parens, `<T>` only applies to the last operand. tsv preserves parens to maintain the original meaning. Both formatters agree on preserving parens for assignment expressions (`(x = y)<T>`).

**Optional-chain base member chain**: When a parenthesized optional chain is the base of a **member chain** — one that routes through Prettier's `member-chain.js` printer because it has a bare member access (`(a?.b).c.d()`, not `(a?.b).c()`) — Prettier drops the parens (`a?.b.c.d()`), folding the trailing access into the optional chain and changing its short-circuit boundary. `(a?.b).c.d()` reads `.c` on the result of `a?.b` (throws if `a` is null); `a?.b.c.d()` short-circuits the `.c.d()` tail to `undefined`. The forms parse to different ASTs, so this is a semantic change. It's a bug in `member-chain.js`: Prettier's own `parentheses/chain-expression.js` (`shouldAddParenthesesToChainExpression`) keeps the parens for a `ChainExpression` that is the `object` of a non-optional `MemberExpression`, but the chain printer flattens without honoring it. tsv keeps the parens. The non-member-chain forms (`(a?.b).c`, `(a?.b).c()`, `(a?.b)()`, `(a?.b)[c]`) keep their parens in both formatters — see [optional_paren_boundary](../tests/fixtures/typescript/expressions/chain/optional_paren_boundary/). The same bug recurs with a **non-null assertion** on the base: `(a?.b)!.c()` (and `(a?.b)!.c.d()`) — the `!` adds a node that pushes the chain past the `member-chain.js` threshold, so Prettier drops the parens (`a?.b!.c()`) while the member-only forms `(a?.b)!.c` / `(a?.b)!.c.d` and the single member+call `(a?.b).c()` (no `!`) stay below it and match. tsv keeps the parens in every case — see [optional_paren_non_null_member_chain](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_member_chain_prettier_divergence/). The non-null boundary forms `(a?.b)!.c`, `(a?.b)!()`, `(a?.b)![c]`, `(a?.b)!.c.d` (no call in the tail) keep their parens in both formatters — see [optional_paren_non_null_boundary](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_boundary/).

**Optional-chain non-null new callee**: A non-null assertion sealing a parenthesized optional chain used as a `new` callee — `new (a?.b)!()` / `new (a?.())!()` — keeps the parens. The `!` is type-only, so tsv normalizes to the `!`-outside form (matching the boundary sibling `(a?.b)!.c`). Prettier strips the parens off **both** forms, and both results are themselves **syntax errors** (an optional chain can't be a `new` callee), so Prettier's own output fails to re-parse: the member base becomes `new a?.b!()` and the call base `new a?.()!()`. (Under prettier-plugin-svelte 3.5.2 the call base instead stayed valid as `new (a?.()!)()`, the `!` merely relocated inside the parens; 4.x strips the parens here too.) tsv keeps the semantically-required parens in the canonical `!`-outside form in both cases. The non-`!` new-callee forms (`new (a?.b)()`, `new (a?.b())()`, `new (a?.())()`) and the tagged-template tag (with or without `!`) match Prettier — see [optional_paren_new_tagged_boundary](../tests/fixtures/typescript/expressions/chain/optional_paren_new_tagged_boundary/) and [optional_paren_non_null_tag_boundary](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_tag_boundary/).

**Constrained infer extends-operand parens**: An `infer X extends C` only ever appears in a conditional type's extends-type, so a trailing token always follows the constraint. When the constraint — or a nested arrow's return — abuts the enclosing `? :`, the parens TypeScript requires are the only thing keeping the parse unambiguous, and Prettier strips them, emitting output that **fails to re-parse** (acorn-typescript rejects it). Two sites: a constrained infer behind a _nested_ arrow return (`M extends (() => () => infer U extends string) ? …` — Prettier's rule only inspects the immediate return type) and a _conditional-type_ infer constraint (`X extends infer U extends (A extends B ? C : D) ? …`). tsv keeps the parens in both, staying valid. The single-arrow form (`M extends (() => infer U extends string) ? …`) is preserved by both formatters (Prettier's single-level `needs-parentheses` rule covers it — see [constrained_extends_parens](../tests/fixtures/typescript/types/infer/constrained_extends_parens/), where tsv matches). A bare `<T extends (A extends B ? C : D)>` type-parameter declaration is unaffected: the `>` terminates it, so Prettier strips and tsv matches.

**Single specifier import**: Prettier intentionally keeps single-specifier imports on one line even when they exceed print width ([prettier/prettier#1954](https://github.com/prettier/prettier/issues/1954#issuecomment-306067705)). tsv wraps at print width for consistency and to respect the configured line length limit.

**Module path calls**: Prettier special-cases `require`/`import` identifiers:

- `require(string)`: Prettier keeps on one line regardless of length; tsv wraps at print width
- `require.resolve.paths(string)`: Prettier breaks at `.paths` chain; tsv expands call arguments
- `import.meta.resolve(string)`: Prettier breaks at `.resolve` chain; tsv expands call arguments

tsv treats these like any other function call—no special-casing for module path identifiers. This is consistent with tsv's handling of single-specifier imports: respect print width uniformly.

**Non-null parenthesized base**: For a non-null assertion on a parenthesized base whose inner call breaks its arguments (`(await call(...))!.member`), Prettier hugs the inner call (`(await call(\n...\n))!.member`) — yet it _hangs_ the outer parens for the same base without the `!` (`(await call(...)).member`, see [paren_base_trailing_long](../tests/fixtures/typescript/expressions/member/paren_base_trailing_long/), where tsv matches Prettier). tsv lays out the parenthesized base the same way regardless of a trailing non-null assertion, keeping the two forms visually consistent. Content is identical (ASTs match); only the parenthesized-base layout differs.

**Return type generic union**: Prettier has special handling for `null` and `void` in union types within generic return types. When the second union member is `null` or `void`: (1) function declarations and class methods allow lines to exceed print width instead of breaking inside `<>`, (2) arrow functions break the assignment (`const fn =`) instead of breaking inside the return type. tsv breaks consistently inside the return type generic at the print width boundary regardless of type keyword.

**Arrow type param trailing comma**: For a generic arrow with a **single type param that has no constraint** (`<T>`, default-only `<T = string>`, or `const`-modified `<const T>`), Prettier forces a trailing comma — `<T,>` — via `shouldForceTrailingComma` (`language-js/print/type-parameters.js`). It does so to keep the output valid as TSX, where a bare `<T>` is ambiguous with a JSX element; the guard fires whenever the file is not known to end in `.ts`, which is always the case for a Svelte `<script>` body (prettier-plugin-svelte hands it to prettier without a `.ts` filepath). tsv has no JSX — it never emits TSX, and Svelte's own parser accepts bare `<T>` in every TS position (`<script>`, template `{...}`, `{@const}`) — so the disambiguation is vestigial and tsv emits the bare canonical form. Multi-param (`<T, U>`), constrained (`<T extends X>`), and empty (`<>`) type params are unaffected; prettier never forces the comma for those and tsv matches. The accepted tradeoff: in a mixed-tool repo prettier rewrites `<T>` back to `<T,>`, so the two ping-pong on this construct (reviewed and accepted — bare `<T>` is correct for a non-JSX formatter). Fixtures: [single_type_param](../tests/fixtures/typescript/expressions/arrow/generic/single_type_param_prettier_divergence/), [const_type_param_arrow](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_arrow_prettier_divergence/), and — stacked with the acorn-typescript async param-drop parser bug — [async_generic/stacked](../tests/fixtures/typescript/expressions/arrow/async_generic/stacked_svelte_prettier_divergence/), [async_generic/forms](../tests/fixtures/typescript/expressions/arrow/async_generic/forms_svelte_prettier_divergence/) (optional-param, object-`as`-body, and a type-vs-value-position contrast that pins the comma to value position) and [curried_typed_callback](../tests/fixtures/typescript/expressions/arrow/curried_typed_callback_svelte_prettier_divergence/). The comment-relocation fixture [arrow_type_params_paren_comment](../tests/fixtures/typescript/declarations/function/arrow_type_params_paren_comment_prettier_divergence/) also exercises it.

#### Import-phase proposals

The Stage-3 **source-phase imports** and **import defer** proposals (`import source x
from 'mod'` / `import.source('mod')`, `import defer * as ns from 'mod'` /
`import.defer('mod')`) are a tsv-native parser divergence — acorn rejects them, so
they are **not** in the "Prettier rejects valid input" set above (that set is keyed
on acorn *accepting* the input). Prettier diverges two ways:

- **`import defer` — phase dropped (information loss).** Prettier formats `import
  defer * as ns from 'mod'` to `import * as ns from 'mod'`, silently deleting the
  `defer` phase keyword and changing the import's semantics. tsv preserves it.
- **`import source` — printer throws.** Prettier's `typescript` parser reads
  `source` as a binding name and throws (`'=' expected`). tsv parses and keeps the
  statement stable.

The dynamic `import.source(…)` / `import.defer(…)` forms have no divergence —
prettier formats them identically to tsv. None of these can be fixtures (acorn,
the fixture parse oracle, rejects the syntax; prettier, the format oracle, drops or
throws), so the printer's round-trips are covered by `tests/import_phase.rs` and
the parser by the test262 suite. The `import source` throw is also live-pinned in
`tests/prettier_error_bugs.rs`; the `import defer` phase-drop is documented-only
(a live "prettier succeeds with wrong output" check would gate the suite on a
sidecar call under load). See
[conformance_svelte.md §Import-phase proposals](./conformance_svelte.md#import-phase-proposals)
and [conformance_test262.md](./conformance_test262.md). **Upstream candidate**:
prettier import-phase support — promote to fixtures once it lands.

### Prettier rejects valid input

These inputs are **valid** by tsv's parse oracle (Svelte / acorn-typescript) and our formatter keeps them stable, but prettier's `typescript` parser/printer **throws** on them — so there is no `output_prettier.*` oracle. Each fixture carries a `prettier_rejects.txt` marker pinning the exact error; rule F6 live-verifies that prettier still rejects the input (failing loudly if the bug is fixed upstream or the error morphs). All three reproduce in plain prettier (`parser: 'typescript'`, zero Svelte) and are fine under `babel-ts`; the 4.x prettier-plugin-svelte bump surfaced them because the plugin switched `lang="ts"` formatting from `babel-ts` to the real `typescript` parser.

- Optional chain to private field (`x?.#a`) — `An optional chain cannot contain private identifiers.` — [private_fields_optional_chain](../tests/fixtures/typescript/declarations/class/private_fields_optional_chain_prettier_divergence/)
- Parenthesized optional-chain decorator callee (`@((a?.b)())`) — `Cannot read properties of undefined (reading 'type')` — [parenthesized_optional_chain](../tests/fixtures/typescript/typescript_specific/decorators/parenthesized_optional_chain_prettier_divergence/)
- Line comment before import-attributes `with` — `'(' expected.` — [with_keyword_comment_line](../tests/fixtures/typescript/modules/imports/with_keyword_comment_line_prettier_divergence/)

**Optional chain to private field**: `x?.#a` is valid modern JS (ecma262 `OptionalChain : ?. PrivateIdentifier`, from the private-fields-in-`in` era). typescript-estree rejects it; tsv keeps it stable. The comprehensive (prettier-formattable) private-field cases live in [private_fields](../tests/fixtures/typescript/declarations/class/private_fields/).

**Parenthesized optional-chain decorator callee**: a parenthesized optional chain that is then _continued_ (called or member-accessed) inside a decorator (`@((a?.b)())`, `@((a?.b).c())`) crashes prettier's `estree` printer. `@(a?.b())`, `@((a?.b))`, and the non-optional `@((a.b)())` are all fine. The non-crashing parenthesized-decorator cases live in [parenthesized](../tests/fixtures/typescript/typescript_specific/decorators/parenthesized/).

**Line comment before `with`**: a line comment between an import's source and its `with` attributes keyword (`import b from './b' // c⏎with {…}`) makes typescript-estree throw `'(' expected.`. The block-comment forms (source→`with` and `with`→`{`) and the line comment _after_ `with` all format — Prettier relocates/floats them — and live in [with_keyword_comment](../tests/fixtures/typescript/modules/imports/with_keyword_comment_prettier_divergence/).

### Tabs-Only Alignment

These fixtures exercise the [Tabs-Only Indentation Philosophy](#tabs-only-indentation-philosophy): Prettier's `align(2, …)` for broken union members emits `tabs + 2 spaces` under `--use-tabs`, while tsv rounds the 2-column offset up to a whole tab everywhere.

- Union object member — [union_object_member](../tests/fixtures/typescript/types/union_object_member_prettier_divergence/)
- Union hugged object — [union_hug_object](../tests/fixtures/typescript/types/union_hug_object_prettier_divergence/)
- Union parenthesized object — [union_parens_object](../tests/fixtures/typescript/types/union_parens_object_prettier_divergence/)
- Union intersection trailing object — [union_intersection_object_long](../tests/fixtures/typescript/types/union_intersection_object_long_prettier_divergence/)
- Union member nested generic — [nested_generic_member_long](../tests/fixtures/typescript/types/nested_generic_member_long_prettier_divergence/)
- Union member function type — [union_fn_type_member_long](../tests/fixtures/typescript/types/union_fn_type_member_long_prettier_divergence/)
- Union member break + line comment — [union_member_long_line_comment](../tests/fixtures/typescript/types/comments/union_member_long_line_comment_prettier_divergence/)
- Union paren-union member — [union_paren_union_member_long](../tests/fixtures/typescript/types/union_paren_union_member_long_prettier_divergence/)
- Union paren member + line comment — [union_paren_member_long_line_comment](../tests/fixtures/typescript/types/comments/union_paren_member_long_line_comment_prettier_divergence/)

### TypeScript: Template Literals

tsv formats template interpolation `${...}` using two strategies based on expression type:

- **Qualifying types** (Identifier, MemberExpression, ConditionalExpression, BinaryExpression, SequenceExpression, TSAsExpression, TSSatisfiesExpression, etc.): softline wrapping at `${`/`}` boundaries — the group breaks when the line exceeds print width. This provides width enforcement for expressions that have no internal break points.
- **Non-qualifying types** (CallExpression, chains, ArrowFunction, etc.): no softlines at `${`/`}` — expression breaks internally while `${`/`}` stays hugged. Matches Prettier's approach where these types keep their doc structure.

Prettier uses atomic text (no doc structure) when the expression has no structural newlines, which means internal breaks can't happen. tsv preserves doc structure, so non-qualifying expressions can still break internally when they exceed width. This divergence appears when non-qualifying expressions exceed width and would need internal breaks.

- 100/101 char boundary — [long](../tests/fixtures/typescript/expressions/literals/template/long_prettier_divergence/)
- Long expression — [interpolation_expression_long](../tests/fixtures/typescript/expressions/literals/template/interpolation_expression_long_prettier_divergence/)
- Multiline indent — [interpolation_multiline_indent_long](../tests/fixtures/typescript/expressions/literals/template/interpolation_multiline_indent_long_prettier_divergence/)
- Nested template — [interpolation_nested_template](../tests/fixtures/typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence/)
- Template literal type — [template_literal_type_long](../tests/fixtures/typescript/types/template_literal_type_long_prettier_divergence/)
- Template literal type (multibyte width) — [template_literal_type_multibyte_long](../tests/fixtures/typescript/types/template_literal_type_multibyte_long_prettier_divergence/)
- Type with conditional — [template_literal_type_conditional_long](../tests/fixtures/typescript/types/template_literal_type_conditional_long_prettier_divergence/)
- Ternary consequent — [template_consequent_long](../tests/fixtures/typescript/expressions/ternary/template_consequent_long_prettier_divergence/)
- Binary operand — [template_operand_long](../tests/fixtures/typescript/expressions/logical/template_operand_long_prettier_divergence/)

**Expression atomization**: Prettier pre-renders each template expression at `printWidth: Infinity` (`template-literal.js:212-226`). If the rendered result is single-line (no newlines), prettier replaces the expression doc with an atomic string — making it impossible to break, even when the line exceeds print width. Only expressions that naturally produce multi-line output get softline wrapping. tsv always preserves doc structure, so qualifying expressions can break at `${`/`}` boundaries when the line exceeds width. This is the primary source of template literal divergences: simple expressions like `${prop}` or `${obj.field}` stay inline in prettier (atomic) but may break in tsv (softline group).

**Multiline indent**: For code-generation templates with indented content, tsv applies Prettier's `addAlignmentToDoc` for indent calculation (using ceiling division to match prettier's useTabs rounding). Non-qualifying types (chains, calls) hug at `${}` with internal breaks, while qualifying types break at `${`/`}` when exceeding width.

**Nested template**: When a template expression contains an array literal wrapping a long inner template, tsv breaks the array to respect print width while Prettier keeps it inline.

### TypeScript: Comments

Prettier relocates certain comments during formatting. tsv preserves comments where the user placed them. This is the single largest category of divergence. See [Comment Position Philosophy](#comment-position-philosophy) for the design principles.

#### Comment relocation

Prettier moves comments between syntactic boundaries into adjacent blocks, parens, or other positions. tsv preserves them where the user placed them.

- Conditional type after `:` → Trailing on true branch — [comment_after_colon](../tests/fixtures/typescript/types/conditional/comment_after_colon_prettier_divergence/)
- Ternary operand to operator (≥2 line comments; test→`?`, consequent→`:`) → Every comment after the first relocated across the operator (`cond // c1⏎ ? // c2`); tsv keeps each before the operator on its own line — [consecutive_operand_comment](../tests/fixtures/typescript/expressions/ternary/consecutive_operand_comment_prettier_divergence/)
- Switch empty body → Discriminant parens — [empty_comment](../tests/fixtures/typescript/statements/switch/empty_comment_prettier_divergence/)
- Switch case before `{` → After opening brace — [case_block_comment](../tests/fixtures/typescript/statements/switch/case_block_comment_prettier_divergence/)
- Switch discriminant trailing → Switch body — [discriminant_trailing_comment](../tests/fixtures/typescript/statements/switch/discriminant_trailing_comment_prettier_divergence/)
- For after `)` → Inline with update clause — [trailing_comment](../tests/fixtures/typescript/statements/for/trailing_comment_prettier_divergence/)
- For empty clauses → Outside parentheses (broken) — [empty_clauses_comment](../tests/fixtures/typescript/statements/for/empty_clauses_comment_prettier_divergence/)
- For non-empty header after `)` (line) → First comment relocated into the parens (trailing the last clause) — [header_body_comment](../tests/fixtures/typescript/statements/for/header_body_comment_prettier_divergence/)
- For-of loop header → Outside loop header — [of_line_comment](../tests/fixtures/typescript/statements/for/of_line_comment_prettier_divergence/)
- For-in/of own-line comment → Before statement or after `)` — [in_of_own_line_comment](../tests/fixtures/typescript/statements/for/in_of_own_line_comment_prettier_divergence/)
- For-in/of pre-paren comment / `for await` (breaking layout) → Pre-paren comment inside the parens, left-trailing line comment after `)` — [in_of_break_pre_paren_comment](../tests/fixtures/typescript/statements/for/in_of_break_pre_paren_comment_prettier_divergence/)
- Do-while after `(` → After semicolon — [open_paren_comment](../tests/fixtures/typescript/statements/do_while/open_paren_comment_prettier_divergence/)
- Do-while between `)` and `;` → Inside the condition parens — [close_paren_comment](../tests/fixtures/typescript/statements/do_while/close_paren_comment_prettier_divergence/)
- If/while/switch keyword before `(` → Inside the condition parens — [keyword_paren_comment](../tests/fixtures/typescript/statements/if/keyword_paren_comment_prettier_divergence/)
- Between `}` and catch/finally → Into subsequent block body — [catch_between_comment](../tests/fixtures/typescript/statements/try/catch_between_comment_prettier_divergence/)
- Try/catch/finally before `{` → Into block body or catch parens — [line_comment_absorbed](../tests/fixtures/typescript/statements/try/line_comment_absorbed_prettier_divergence/)
- Label after `:` → Before entire labeled statement — [comment](../tests/fixtures/typescript/statements/labeled/comment_prettier_divergence/)
- Between `}` and `else` → Into else block body — [else_block_own_line_comment](../tests/fixtures/typescript/statements/if/else_block_own_line_comment_prettier_divergence/), [else_leading_block_comment](../tests/fixtures/typescript/statements/if/else_leading_block_comment_prettier_divergence/)
- While before `{` (line) → Into block body — [line_before_body_comment](../tests/fixtures/typescript/statements/while/line_before_body_comment_prettier_divergence/)
- While before `{}` (block/line) → Into block body (expands block) — [absorbed_body_comment](../tests/fixtures/typescript/statements/while/absorbed_body_comment_prettier_divergence/)
- Do-while between `}` and `while` → Into while condition — [line_before_while_comment](../tests/fixtures/typescript/statements/do_while/line_before_while_comment_prettier_divergence/), [while_leading_block_comment](../tests/fixtures/typescript/statements/do_while/while_leading_block_comment_prettier_divergence/)
- Trailing member chain → After `=` — [trailing_member_comment](../tests/fixtures/typescript/expressions/calls/chained/trailing_member_comment_prettier_divergence/)
- Member-only chain interior line comment (no calls) → Hoisted before the expression / trailed on the statement (merging consecutive); tsv breaks the chain and keeps each comment in place — [member_only_interior_line_comment](../tests/fixtures/typescript/expressions/calls/chained/member_only_interior_line_comment_prettier_divergence/)
- Block comment in computed `[]` → Before member chain (hoisted) — [block_comment_computed_member_long](../tests/fixtures/typescript/syntax/comments/block_comment_computed_member_long_prettier_divergence/)
- Switch case colon comment → Before colon or into body — [case_colon_comment](../tests/fixtures/typescript/statements/switch/case_colon_comment_prettier_divergence/)
- Switch case/`default` colon, comment contains a colon (scan robustness) → `case` preserved (plain); `default` into the body — [case_colon_in_comment](../tests/fixtures/typescript/statements/switch/case_colon_in_comment_prettier_divergence/)
- Class property definite `!` → Before `!` modifier — [property_definite_comment](../tests/fixtures/typescript/statements/class/property_definite_comment_prettier_divergence/)
- Class property modifier → Before `?`/`!` modifier — [property_modifier_comment](../tests/fixtures/typescript/statements/class/property_modifier_comment_prettier_divergence/)
- Between member modifiers → After the last modifier — [modifier_pair_comment](../tests/fixtures/typescript/declarations/class/modifier_pair_comment_prettier_divergence/)
- Generator method `async`→`*` (class + object shorthand) → After the `*`, before the name (`async */* c */ m()`) — [async_star_comment (class)](../tests/fixtures/typescript/statements/class/async_star_comment_prettier_divergence/), [(object)](../tests/fixtures/typescript/expressions/objects/async_star_comment_prettier_divergence/)
- Interface member after `?` → Before `?` or inside parens — [modifier_after_comment](../tests/fixtures/typescript/types/type_members/modifier_after_comment_prettier_divergence/)
- Type-literal member after `?` → Before `?` or inside parens — [optional_marker_comment](../tests/fixtures/typescript/types/type_literal/optional_marker_comment_svelte_prettier_divergence/)
- Member after `?`, no annotation (iface + type-literal) → Before `?` (`a /* c */?;`) — [property_signature_no_annotation_optional_comment](../tests/fixtures/typescript/types/type_members/property_signature_no_annotation_optional_comment_prettier_divergence/)
- Class method after `?` → Before `?` modifier — [optional_marker_comment](../tests/fixtures/typescript/declarations/class/optional_marker_comment_prettier_divergence/)
- Optional `?` to `:` line comment (all contexts) → Trailing the member `;` — [optional_marker_line_comment](../tests/fixtures/typescript/syntax/comments/optional_marker_line_comment_prettier_divergence/)
- Member key to `:` line comment (non-optional) → Trailing the member `;` — [key_colon_line_comment](../tests/fixtures/typescript/syntax/comments/key_colon_line_comment_prettier_divergence/)
- Member key to optional `?` line comment (iface/type-literal/class/method) → Trailing the member `;`; tsv keeps the comment after the key and drops `?` + the rest of the member to a continuation line — [key_optional_marker_line_comment](../tests/fixtures/typescript/syntax/comments/key_optional_marker_line_comment_prettier_divergence/)
- Enum member name to `=` line comment → After the value (`A = 1 // c`); tsv keeps it after the name + continuation indent. With a second trailing comment prettier merges both onto one line (info loss); tsv keeps them distinct — [member_before_eq_line_comment](../tests/fixtures/typescript/declarations/enum/member_before_eq_line_comment_prettier_divergence/)
- Class property name to `=` line comment → Trailing the member `;` after the value (`a = 1; // c`); tsv keeps it in place + continuation indent. Two comments → prettier merges (info loss), tsv keeps distinct — [property_before_eq_line_comment](../tests/fixtures/typescript/declarations/class/property_before_eq_line_comment_prettier_divergence/)
- Variable binding to `=` line comment → Trailing the statement `;` after the value (`const a = 1; // c`); tsv keeps it in place + continuation indent. Two comments → prettier merges (info loss), tsv keeps distinct — [declarator_before_eq_line_comment](../tests/fixtures/typescript/declarations/variable/declarator_before_eq_line_comment_prettier_divergence/)
- Object property key to `:` line comment → Hoisted to its own line before the key (`// c⏎a: 1`); tsv keeps it after the key and drops `: value` to a continuation line — [property_key_colon_line_comment](../tests/fixtures/typescript/expressions/objects/property_key_colon_line_comment_prettier_divergence/)
- Variable definite `!` → After `!` modifier — [definite_comment](../tests/fixtures/typescript/declarations/variable/definite_comment_prettier_divergence/)
- Function param optional `?` → After `?` modifier — [param_optional_comment](../tests/fixtures/typescript/declarations/function/param_optional_comment_prettier_divergence/)
- Computed key after `]` (object) → Inside brackets `[x /* c */]` — [computed_key_bracket_colon_comment](../tests/fixtures/typescript/expressions/objects/computed_key_bracket_colon_comment_prettier_divergence/)
- Computed key after `]` (class) → Inside brackets `[x /* c */]` — [computed_key_bracket_comment](../tests/fixtures/typescript/statements/class/computed_key_bracket_comment_prettier_divergence/)
- Computed key after `]` (iface) → Inside brackets (set: into params) — [computed_key_bracket_comment](../tests/fixtures/typescript/types/type_members/computed_key_bracket_comment_prettier_divergence/), [paren_in_comment](../tests/fixtures/typescript/types/type_members/computed_key_bracket_paren_in_comment_prettier_divergence/)
- Computed key after `]`, no annotation (iface) → Inside brackets `[k /* c */]` — [computed_key_no_annotation_comment](../tests/fixtures/typescript/types/type_members/computed_key_no_annotation_comment_prettier_divergence/)
- Computed key after `]` (destr) → Inside brackets `[x /* c */]` — [computed_key_bracket_comment](../tests/fixtures/typescript/expressions/destructuring/computed_key_bracket_comment_prettier_divergence/)
- `readonly` keyword to `[` (index sig) → Inside brackets before the key `[/* c */ k` — [index_signature_readonly_comment](../tests/fixtures/typescript/types/type_members/index_signature_readonly_comment_prettier_divergence/)
- Accessor keyword (`get`/`set`) to `[` (computed key) → Inside brackets before the key `[/* c */ a` — [accessor_keyword_bracket_comment](../tests/fixtures/typescript/expressions/objects/accessor_keyword_bracket_comment_prettier_divergence/)
- Generator `*` to `[` (computed key, incl. `async *`) → Inside brackets before the key `[/* c */ a` — [generator_star_bracket_comment](../tests/fixtures/typescript/expressions/objects/generator_star_bracket_comment_prettier_divergence/)
- Type params to `(` (signatures) → Inside parens as leading on param — [signature_paren_in_comment](../tests/fixtures/typescript/types/type_members/signature_paren_in_comment_prettier_divergence/)
- `new` to `(` (construct signatures) → Inside parens as leading on param (after `)` when empty) — [construct_signature_paren_in_comment](../tests/fixtures/typescript/types/interfaces/construct_signature_paren_in_comment_prettier_divergence/)
- `new` to `(` (constructor types, incl. `abstract`) → Inside parens as leading on param (after `)` when empty) — [constructor_paren_comment](../tests/fixtures/typescript/types/function_type/constructor_paren_comment_prettier_divergence/)
- Type params to `(` (func types) → Inside parens as leading on param — [paren_in_comment](../tests/fixtures/typescript/types/function_type/paren_in_comment_prettier_divergence/)
- Type params to `(` (declare fn) → Inside parens as leading on param — [declare_paren_in_comment](../tests/fixtures/typescript/declarations/function/declare_paren_in_comment_prettier_divergence/)
- Type params to `(` (arrows) → Inside parens as leading on param — [arrow_type_params_paren_comment](../tests/fixtures/typescript/declarations/function/arrow_type_params_paren_comment_prettier_divergence/)
- Type params to `(` (overloads) → Inside parens as leading on param — [overload_type_params_paren_comment](../tests/fixtures/typescript/declarations/function/overload_type_params_paren_comment_prettier_divergence/)
- Type params to `(` (iface/type) → Inside parens as leading on param — [method_type_params_paren_comment](../tests/fixtures/typescript/types/type_members/method_type_params_paren_comment_prettier_divergence/)
- Anon func keyword to `(` → After `)` or inside parens — [expr_anon_keyword_comment](../tests/fixtures/typescript/declarations/function/expr_anon_keyword_comment_prettier_divergence/)
- Anon func keyword to `(` (line) → After `)` or inside parens — [expr_anon_line_comment](../tests/fixtures/typescript/declarations/function/expr_anon_line_comment_prettier_divergence/)
- Anon class keyword to `{` (line) → Into class body — [expr_anon_line_comment](../tests/fixtures/typescript/declarations/class/expr_anon_line_comment_prettier_divergence/)
- Constructor type `new` to `(` → After `)`, before param, or place — [constructor_type_new_comment](../tests/fixtures/typescript/types/constructor_type_new_comment_prettier_divergence/)
- Constructor type `abstract` to `new` → After `new` (mirrors the `new`-to-params relocation; was dropped — content loss) — [constructor_type_abstract_comment](../tests/fixtures/typescript/types/constructor_type_abstract_comment_prettier_divergence/)
- Name to type params (line) → End of declaration line — [name_type_params_line_comment](../tests/fixtures/typescript/declarations/class/name_type_params_line_comment_prettier_divergence/)
- Method name to type params (line) → End of method line — [method_name_type_params_line_comment](../tests/fixtures/typescript/declarations/class/method_name_type_params_line_comment_prettier_divergence/)
- Heritage last item before `{` → Into class/interface body — [heritage_last_item_line_comment](../tests/fixtures/typescript/declarations/class/heritage/heritage_last_item_line_comment_prettier_divergence/)
- Arrow body stripped parens → Into arrow params or trailing — [body_paren_comment](../tests/fixtures/typescript/expressions/arrows/body_paren_comment_prettier_divergence/)
- Sequence last-operand trailing edge (stmt) → After the `;` (before it, in tsv) — [operand_edge_comment_stmt](../tests/fixtures/typescript/expressions/sequence/operand_edge_comment_stmt_prettier_divergence/)
- Between keyword and `(` → Inside parens — [keyword_paren_comment](../tests/fixtures/typescript/syntax/comments/keyword_paren_comment_prettier_divergence/)
- `for await` keyword gaps → Inside parens — [for_await_keyword_comment](../tests/fixtures/typescript/statements/for/for_await_keyword_comment_prettier_divergence/)
- Between `)` and `{` (switch) → Inside condition parens — [condition_absorbed_comment](../tests/fixtures/typescript/syntax/comments/condition_absorbed_comment_prettier_divergence/)
- Between `)` and `{` (switch), comment contains a `{` (scan robustness) → Inside condition parens — [condition_absorbed_brace_in_comment](../tests/fixtures/typescript/syntax/comments/condition_absorbed_brace_in_comment_prettier_divergence/)
- Before `;` in declarations → After `;` — [around_semicolons](../tests/fixtures/typescript/syntax/comments/around_semicolons_prettier_divergence/)
- Abstract method return type to `;` → After `;` (abstract methods only; declare method/property and abstract property keep it before) — [method_trailing_semicolon_comment](../tests/fixtures/typescript/declarations/class/method_trailing_semicolon_comment_svelte_prettier_divergence/)
- Between modifier keywords → Before declaration name — [declaration_keyword_name](../tests/fixtures/typescript/syntax/comments/declaration_keyword_name_prettier_divergence/)
- Between `async` and `function` → Before function name — [comments_between_keywords](../tests/fixtures/typescript/declarations/function/async/comments_between_keywords_prettier_divergence/)
- Import keyword to empty `{}` → After `from` — [empty_keyword_comment](../tests/fixtures/typescript/modules/imports/empty_keyword_comment_prettier_divergence/)
- Export keyword to empty `{}` → After `from` — [empty_keyword_comment](../tests/fixtures/typescript/modules/exports/empty_keyword_comment_prettier_divergence/)
- Import `type` keyword to empty `{}` → After `from` — [empty_type_keyword_comment](../tests/fixtures/typescript/modules/imports/empty_type_keyword_comment_prettier_divergence/)
- Export `type` keyword to empty `{}` → After `from` — [empty_type_keyword_comment](../tests/fixtures/typescript/modules/exports/empty_type_keyword_comment_prettier_divergence/)
- Import `type` keyword to specifiers → Into the specifier braces — [type_keyword_comment](../tests/fixtures/typescript/modules/imports/type_keyword_comment_prettier_divergence/)
- Export `type` keyword to specifiers → Into the specifier braces — [type_keyword_comment](../tests/fixtures/typescript/modules/exports/type_keyword_comment_prettier_divergence/)
- Default import header comments → Binding side of `type` — [default_keyword_comment](../tests/fixtures/typescript/modules/imports/default_keyword_comment_prettier_divergence/)
- Namespace import header comments → Binding side of `type` — [namespace_keyword_comment](../tests/fixtures/typescript/modules/imports/namespace_keyword_comment_prettier_divergence/)
- Export-all header comments → After `from` — [all_keyword_comment](../tests/fixtures/typescript/modules/exports/all_keyword_comment_prettier_divergence/)
- Export-all namespace `*` to `as` → After `as` (before binding) — [all_namespace_keyword_comment](../tests/fixtures/typescript/modules/exports/all_namespace_keyword_comment_prettier_divergence/)
- Export `}` to `;` (no `from`) → Inside the specifier braces — [close_brace_comment](../tests/fixtures/typescript/modules/exports/close_brace_comment_prettier_divergence/)
- Import binding to `from` (line) → After `;` — [from_comment](../tests/fixtures/typescript/modules/imports/from_comment_prettier_divergence/)
- Import specifiers to `from` → Into the specifier braces — [from_comment](../tests/fixtures/typescript/modules/imports/from_comment_prettier_divergence/)
- Export specifiers to `from` → Into the specifier braces — [from_comment](../tests/fixtures/typescript/modules/exports/from_comment_prettier_divergence/)
- Import source to `with` (line) → After `;` — [with_keyword_comment](../tests/fixtures/typescript/modules/imports/with_keyword_comment_prettier_divergence/)
- Import `with` to attributes `{` → Before `with` (block) / `;` (line) — [with_keyword_comment](../tests/fixtures/typescript/modules/imports/with_keyword_comment_prettier_divergence/)
- Re-export attributes header (`with`→`{`, after `}`) → Before `with` (block) / into braces — [exports/attributes_comment](../tests/fixtures/typescript/modules/exports/attributes_comment_prettier_divergence/)
- Empty `with {}` comment (`with`→`{`, inside `{}`, after `}`) → Before `with` — [attributes_empty_comment](../tests/fixtures/typescript/modules/imports/attributes_empty_comment_prettier_divergence/)
- Import source to `;` (line) → After `;` — [source_trailing_comment](../tests/fixtures/typescript/modules/imports/source_trailing_comment_prettier_divergence/)
- Re-export source to `;` (line) → After `;` — [all_source_trailing_comment](../tests/fixtures/typescript/modules/exports/all_source_trailing_comment_prettier_divergence/)
- Import-equals ref to `;` (line) → After `;` — [equals_trailing_comment](../tests/fixtures/typescript/modules/imports/equals_trailing_comment_prettier_divergence/)
- Import keyword/`from`→source (line) → In place flat (bare/empty), into braces (named), after `;` (default); tsv indents — [source_line_comment](../tests/fixtures/typescript/modules/imports/source_line_comment_prettier_divergence/)
- Re-export `from`→source (line) → In place flat (empty/export-all), into braces (named); tsv indents — [source_line_comment](../tests/fixtures/typescript/modules/exports/source_line_comment_prettier_divergence/)
- No-`from` empty export keyword→`{}` (line) → In place flat; tsv indents — [empty_no_from_line_comment](../tests/fixtures/typescript/modules/exports/empty_no_from_line_comment_prettier_divergence/)
- `export`/`export default`→declaration (line) → In place flat; tsv indents — [export_declaration_line_comment](../tests/fixtures/typescript/syntax/comments/export_declaration_line_comment_prettier_divergence/)
- Declaration keyword→name (line; `function`/`class`/`enum`/`declare function`) → In place flat; tsv indents — [keyword_name_line_comment](../tests/fixtures/typescript/syntax/comments/keyword_name_line_comment_prettier_divergence/)
- Between `else` and empty `;` → Before `else` keyword — [else_empty_line_comment](../tests/fixtures/typescript/statements/if/else_empty_line_comment_prettier_divergence/)
- Between `else` and non-block body → Before `else` keyword — [else_line_comment_nonblock](../tests/fixtures/typescript/statements/if/else_line_comment_nonblock_prettier_divergence/)
- Union infix `\|` line comment → Trailing on previous member — [union_infix_pipe_line_comment](../tests/fixtures/typescript/types/comments/union_infix_pipe_line_comment_prettier_divergence/)
- Retained paren union member comment → Outside the parens (after `)`/`(`) — [union_intersection_retained_paren_comment](../tests/fixtures/typescript/types/union_intersection_retained_paren_comment_prettier_divergence/)
- Retained paren union member line cmt → Outside parens, member kept inline — [union_intersection_retained_paren_line_comment](../tests/fixtures/typescript/types/union_intersection_retained_paren_line_comment_prettier_divergence/)
- Retained paren union leading line cmt → Before the 1st member (out of parens) — [union_intersection_retained_paren_leading_line_comment](../tests/fixtures/typescript/types/union_intersection_retained_paren_leading_line_comment_prettier_divergence/)
- Retained paren intersection member cmt → Outside the parens (after `)`/`(`) — [retained_paren_intersection_member_comment](../tests/fixtures/typescript/types/retained_paren_intersection_member_comment_prettier_divergence/)
- Type alias head to `=` (line) → After `=` (right of operator) — [type_alias_line_pre_equals](../tests/fixtures/typescript/types/comments/type_alias_line_pre_equals_prettier_divergence/)
- Type param keyword to value (own-line) → Up onto the keyword line — [type_param_keyword_own_line_comment](../tests/fixtures/typescript/types/comments/type_param_keyword_own_line_comment_prettier_divergence/)
- Function param default to value (line) → Floated out to trail the whole parameter (after the value) — [param_default_line_comment](../tests/fixtures/typescript/declarations/function/param_default_line_comment_prettier_divergence/)
- `as`/`satisfies` keyword to type (line) → Statement-trailing (after `;`) — [as_satisfies_value_line_comment](../tests/fixtures/typescript/expressions/as_satisfies_value_line_comment_prettier_divergence/)
- Angle-bracket type assertion, trailing `<` / trailing `>` (line) → trailing-`<` onto its own line; trailing-`>` into the cast, trailing the type (`<string> // c` → `<⏎string // c⏎>z`, tsv keeps it after `>` leading the expression one indent in). Own-line-after-`<` and trailing-the-type break the cast in both formatters and match — [type_assertion_line_comment](../tests/fixtures/typescript/types/type_assertion_line_comment_svelte_prettier_divergence/)
- Angle-bracket type assertion, own-line before `>` after a trailing-type comment (line) → No fixed point (prettier oscillates the comment across `>`); tsv keeps it on its own line before `>` inside the cast — [type_assertion_close_own_line_comment](../tests/fixtures/typescript/types/type_assertion_close_own_line_comment_svelte_prettier_divergence/)
- Angle-bracket type assertion, own-line after `>` before the expression (line) → Into the cast trailing the type, over two passes (pass 1 glues it onto `>`, pass 2 moves it inside — F4 audit signature); tsv keeps it on its own line leading the expression — [type_assertion_expr_own_line_comment](../tests/fixtures/typescript/types/type_assertion_expr_own_line_comment_svelte_prettier_divergence/)
- Angle-bracket type assertion robustness (object operand after `>`; block+line trailing `<`) → operand-after-`>` comment relocated into the cast, trailing-`<` block+line moved to their own line; tsv preserves each. A generic cast type with a nested `>` matches prettier (the close `>` is found past the type's own) — [type_assertion_line_comment_robustness](../tests/fixtures/typescript/types/type_assertion_line_comment_robustness_svelte_prettier_divergence/)
- Heritage keyword to type (line) → Up before the keyword — [extends_keyword_line_comment](../tests/fixtures/typescript/class/extends_keyword_line_comment_prettier_divergence/)
- Conditional `extends` to check (line) → Trailing the extends-type — [check_extends_line_comment](../tests/fixtures/typescript/types/conditional/check_extends_line_comment_prettier_divergence/)
- Mapped `:` to value (line) → Trailing the member `;` — [mapped_value_line_comment](../tests/fixtures/typescript/types/mapped_value_line_comment_prettier_divergence/)
- Type predicate `is` to type (line) → Trailing the body `{` — [predicate_is_line_comment](../tests/fixtures/typescript/types/predicate_is_line_comment_prettier_divergence/)
- Call arg after-comma block + same-line line comment → Block relocated before the comma (tsv keeps it on the comma line) — [plain](../tests/fixtures/typescript/expressions/calls/nonlast_arg_after_comma_block_then_line_prettier_divergence/), [new](../tests/fixtures/typescript/expressions/calls/new_nonlast_arg_after_comma_block_then_line_prettier_divergence/), [joined](../tests/fixtures/typescript/expressions/calls/multiline_arg_nonlast_after_comma_block_then_line_prettier_divergence/), [chain](../tests/fixtures/typescript/expressions/calls/chained/nonlast_arg_after_comma_block_then_line_prettier_divergence/), [import](../tests/fixtures/typescript/expressions/calls/import_inter_arg_block_then_line_prettier_divergence/)
- Call arg after-comma block, **stranded** (newline before the next arg) → Block relocated before the comma (tsv respects the newline and keeps the stranded block on the comma line). A block instead **hugging** the next arg leads it (`C`) and both formatters agree (plain match) — [stranded](../tests/fixtures/typescript/expressions/calls/nonlast_arg_after_comma_block_stranded_prettier_divergence/), [import](../tests/fixtures/typescript/expressions/calls/import_inter_arg_stranded_prettier_divergence/) (dynamic `import()` source→options gap; hugging match at [import_inter_arg_comment](../tests/fixtures/typescript/expressions/calls/import_inter_arg_comment/))
- Call open paren `(` trailing → Onto its own line — [open_paren_comment](../tests/fixtures/typescript/expressions/calls/open_paren_comment_prettier_divergence/), [chain](../tests/fixtures/typescript/expressions/calls/chain_open_paren_comment_prettier_divergence/), [new](../tests/fixtures/typescript/expressions/calls/new_open_paren_comment_prettier_divergence/)
- Object literal `{` trailing → Onto its own line — [open_brace_comment](../tests/fixtures/typescript/expressions/objects/open_brace_comment_prettier_divergence/)
- Array literal `[` trailing → Onto its own line — [open_bracket_comment](../tests/fixtures/typescript/expressions/arrays/open_bracket_comment_prettier_divergence/)
- Block body `{` trailing → Onto its own line — [block_open_brace_comment](../tests/fixtures/typescript/statements/block_open_brace_comment_prettier_divergence/)
- Type-parameter `<` trailing → Onto its own line — [open_angle_comment](../tests/fixtures/typescript/types/type_params/open_angle_comment_prettier_divergence/)
- Function/constructor-type `(` trailing → Onto its own line — [open_paren_comment](../tests/fixtures/typescript/types/function_type/open_paren_comment_prettier_divergence/)
- Fn/ctor-type empty-params `(` trailing → After `)` (out of empty parens) — [empty_param_line_comment](../tests/fixtures/typescript/types/function_type/empty_param_line_comment_svelte_prettier_divergence/)
- Fn/ctor-type pre-arrow `)` trailing (params) → Onto the last param (`a: T // c`) — [pre_arrow_param_line_comment](../tests/fixtures/typescript/types/function_type/pre_arrow_param_line_comment_prettier_divergence/)
- Call/construct signature `(` trailing → Onto its own line (method keeps) — [signature_params_leading_line_comment](../tests/fixtures/typescript/types/comments/signature_params_leading_line_comment_prettier_divergence/)
- Object destructuring `{` trailing → Onto its own line — [object_open_brace_comment](../tests/fixtures/typescript/expressions/destructuring/object_open_brace_comment_prettier_divergence/)
- Array destructuring `[` trailing → Onto its own line — [array_open_bracket_comment](../tests/fixtures/typescript/expressions/destructuring/array_open_bracket_comment_prettier_divergence/)
- Namespace/module body `{` trailing → Onto its own line — [open_brace_comment](../tests/fixtures/typescript/declarations/namespace/open_brace_comment_prettier_divergence/)
- Class/interface/enum body `{` trailing → Onto its own line — [class](../tests/fixtures/typescript/statements/class/open_brace_comment_prettier_divergence/), [interface](../tests/fixtures/typescript/statements/interface/open_brace_comment_prettier_divergence/), [enum](../tests/fixtures/typescript/declarations/enum/open_brace_comment_prettier_divergence/)
- Type literal body `{` trailing → Onto its own line — [type_literal_open_brace_comment](../tests/fixtures/typescript/types/type_literal_open_brace_comment_svelte_prettier_divergence/)
- Import/export specifier `{` trailing → Onto its own line — [imports](../tests/fixtures/typescript/modules/imports/open_brace_comment_prettier_divergence/), [exports](../tests/fixtures/typescript/modules/exports/open_brace_comment_prettier_divergence/)
- Tuple type `[` trailing → Onto its own line — [open_bracket_comment](../tests/fixtures/typescript/types/tuple/open_bracket_comment_prettier_divergence/)
- Index signature `[` trailing (line) → Onto its own line (key's leading comment) — [index_signature_open_bracket_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_open_bracket_line_comment_svelte_prettier_divergence/)
- Index signature before `]` (own-line line) → After `]` (value `:` to next line) — [index_signature_close_bracket_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_close_bracket_line_comment_prettier_divergence/)
- Index signature `]`→value-`:` (line) → Into brackets, trailing key type — [index_signature_bracket_colon_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_bracket_colon_line_comment_prettier_divergence/)
- Index signature `]`→value-`:` (≥2 line comments) → No fixed point (prettier oscillates); tsv keeps each on its own line — [index_signature_bracket_colon_multi_comment](../tests/fixtures/typescript/types/type_members/index_signature_bracket_colon_multi_comment_prettier_divergence/)
- Index signature value-`:`→type under a `]`→`:` comment (line) → Type kept flush (tsv indents continuation) — [index_signature_bracket_colon_value_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_bracket_colon_value_line_comment_prettier_divergence/)
- Index signature in-bracket line comments, **class** context (four positions) → `[`/`:`→type/own-line-before-`]` relocate or stay flush, key→`:` matches — [index_signature_bracket_line_comment_positions](../tests/fixtures/typescript/declarations/class/index_signature_bracket_line_comment_positions_svelte_prettier_divergence/)
- Type-argument `<` trailing (multi) → Onto its own line — [type_argument_open_angle_comment](../tests/fixtures/typescript/types/type_argument_open_angle_comment_prettier_divergence/)
- Call/`new`-expr type-arg `<` trailing (multi) → Onto its own line — [type_args_open_angle_comment](../tests/fixtures/typescript/expressions/calls/type_args_open_angle_comment_prettier_divergence/)
- Computed-key `[` trailing (line) → Out of the brackets to the member's leading line — [computed_key_open_bracket_line_comment](../tests/fixtures/typescript/expressions/objects/computed_key_open_bracket_line_comment_prettier_divergence/)
- Computed-key `[` trailing (line, class) → Kept on `[` line, key glued flush — [computed_key_open_bracket_line_comment](../tests/fixtures/typescript/statements/class/computed_key_open_bracket_line_comment_svelte_prettier_divergence/)
- Computed-key key→`]` (line) → Out of the brackets (same-line → member's leading line, own-line → past `]:` onto the value) — [computed_key_close_bracket_line_comment](../tests/fixtures/typescript/expressions/objects/computed_key_close_bracket_line_comment_prettier_divergence/)

**Prefix type-operator operand hang** (layout, not a relocation): `type A = keyof // c\n\t\tB`. Both formatters keep the comment after the operator _and_ the operator on the `=` line — the comment is **not** relocated. They differ only on the operand's indent: tsv hangs it one level under the operator (the uniform keyword→value layout `append_keyword_value_line_comments`, shared with the type keyword→type sites in the table above), while Prettier leaves it flush at the operator's level. A long _comment-free_ `keyof`/`typeof` still breaks after `=` in both formatters; the comment is what keeps the operator on the `=` line. Content-preserved and idempotent ([type_operator_keyword_line_comment](../tests/fixtures/typescript/types/type_operator_keyword_line_comment_prettier_divergence/)).

**Else line comment with non-block body**: `} else // c\nexpr;` → Prettier relocates the comment before `else` (`} // c\nelse expr;`). tsv preserves the comment after `else` with the body indented (`} else // c\n\texpr;`). Affects block consequent, comments-path, and non-block consequent cases. Both positions are dual-stable.

**Import/export keyword-to-braces comments**: `import /* c */ {} from 'a'` → Prettier relocates comments between the `import`/`export` keyword and empty specifier braces to after `from`: `import {} from /* c */ 'a'`. tsv preserves the comment between keyword and braces. The same holds for an empty type-only import or re-export around the `type` keyword — both the keyword→`type` gap (`import /* c */ type {} from 'a'`, `export /* c */ type {} from 'a'`) and the `type`→`{}` gap (`import type /* c */ {} from 'a'`) are preserved in place while Prettier relocates them after `from`. With **named specifiers** the same two gaps (`import /* c */ type { A } from 'a'`, `import type /* c */ { A } from 'a'`, and the export forms) are likewise preserved in place; here Prettier relocates the comment _into_ the specifier braces as the first specifier's leading comment (`import type { /* c */ A } from 'a'`, a line comment also expanding the braces). Both positions are dual-stable in our formatter. A line comment in any of these gaps — including the no-`from` `export // c⏎{}` — indents the continuation one level (the uniform module-header rule below); for the no-`from` form Prettier keeps the comment in place and flat, so tsv's indent is an indent-only divergence.

**Default / namespace import + export-all header comments**: the same preserve-in-place rule covers the remaining module-header shapes. In a **default** (`import /* c */ Foo from 'a'`) or **namespace** (`import /* c */ * as ns from 'a'`) import, tsv keeps each header comment where the user wrote it; Prettier keeps a comment already adjacent to the binding but relocates a comment between `import` and `type` to the binding side of `type` (`import type /* c */ Foo`). In an **export-all** (`export /* c */ * from 'a'`, including `export /* c */ type * from 'a'`) Prettier relocates _every_ header comment — around `export`, `type`, and `*` — to after `from`, before the source; tsv preserves them in place. A comment between `*` and `as` in a namespace binding (`import * /* c */ as ns`, `export * /* c */ as ns from 'a'`) is likewise preserved in place — Prettier relocates it to after `as`, before the binding. Both positions are dual-stable in our formatter. Per the uniform module-header rule below, a line comment in **every** one of these gaps indents the continuation one level — the keyword→default/namespace binding, `type`→default/namespace binding, `type`→namespace-`*`, export-all `export`/`type`→`*`, `*`→`from`, `*`→`as`, and `as`→binding gaps alike. Where Prettier relocates (export-all, `*`→`as`) tsv is free; where Prettier keeps the comment in place and flat (keyword/`type`→binding, `type`→namespace-`*`, `as`→binding) tsv's indent is a deliberate indent-only divergence.

**Import/export binding-or-specifiers to `from`**: A comment in the gap between an import's binding/specifiers (or a re-export's specifiers) and the `from` keyword is preserved where the user placed it; Prettier's relocation depends on the binding shape. For a **default** or **namespace** binding (no braces) a same-line block comment stays in place in both formatters (`import Foo /* c */ from 'a'` — dual-stable), but Prettier floats a line comment past the `;` to a statement-trailing position (`import Foo from 'a'; // c`, the before-semicolon/float-out rule) while tsv keeps it before `from`, indenting the `from …` continuation one level onto its own line. For **named specifiers** (`import { a } /* c */ from 'a'`, `export { a } /* c */ from 'a'`) Prettier relocates the comment _into_ the braces as the last specifier's trailing comment — a block comment inline (`{ a /* c */ }`), a line comment expanding the braces multiline (`{\n\ta // c\n}`) — while tsv keeps it after `}` with the braces inline (indenting `from …` when a line comment forces the break). Both positions are dual-stable in our formatter. (Otherwise the comments are dropped entirely — content loss.)

**Import attributes header comments**: A comment in an import's attributes header — between the source and the `with` keyword (`import x from 'a' /* c */ with {…}`), or between `with` and the attributes `{` (`import x from 'a' with /* c */ {…}`) — is preserved where the user placed it. Prettier keeps a source→`with` block comment in place (dual-stable), relocates a `with`→`{` block comment to before `with` (`import x from 'a' /* c */ with {…}`), and floats a `with`→`{` line comment past the `;` (`import x from 'a' with {…}; // c`, the before-semicolon/float-out rule); a source→`with` _line_ comment instead makes Prettier's `typescript` parser **throw** (`'(' expected.`), so that form has no oracle — see [Prettier rejects valid input](#prettier-rejects-valid-input) and [with_keyword_comment_line](../tests/fixtures/typescript/modules/imports/with_keyword_comment_line_prettier_divergence/). tsv keeps each comment where the user wrote it; when a line comment forces the `with`/`{…}` onto a new line, tsv indents that continuation one level. The attributes `}`→`;` comment is covered by the before-semicolon rule above. (Otherwise the header comments are dropped entirely — content loss.)

**Keyword → specifier brace comments**: A comment between the `import`/`export` keyword and the named-specifier `{` (`import /* c */ { a }`, `export /* c */ { a }`) is preserved before the brace; Prettier relocates it _into_ the braces as the first specifier's leading comment — a block comment inline (`import { /* c */ a }`), a line comment expanding the braces multiline. tsv keeps it where the user wrote it; a line comment forces `{` onto the next line, indenting the `{…}` continuation one level (the uniform module-header rule below). (The `import type … { a }` type→`{` gap and the empty-braces `import /* c */ {}` gap were already preserved; this is the non-type named-specifier case, which previously **dropped** the comment — content loss now fixed.) See [imports/keyword_brace_comment](../tests/fixtures/typescript/modules/imports/keyword_brace_comment_prettier_divergence/) and [exports/keyword_brace_comment](../tests/fixtures/typescript/modules/exports/keyword_brace_comment_prettier_divergence/).

**Declaration- and module-header line-comment continuation indent**: A _line_ comment in a declaration- or module-header gap forces the following token onto a new line, and tsv indents that continuation one level — a statement spanning lines reads as a continuation, not a second statement.

For **module headers** tsv applies this **uniformly to every gap, with no exceptions**: keyword→`type`, keyword/`type`→default-binding, keyword/`type`→namespace-`*`, keyword/`type`→`{`, keyword/`type`→empty-`{}` (re-export _and_ no-`from`), bare keyword→source, export-all `export`/`type`→`*`, `*`→`from`, `*`→`as`, `as`→binding, binding/specifiers→`from`, `from`→source, the `with`/attributes-`{` gaps, and `export`/`export default`→declaration. Prettier's own handling varies per gap — it relocates the comment (into the braces, after `from`/`as`/`;`), floats it past `;`, or keeps it in place and flat — but tsv **always** indents. So where Prettier relocates, tsv's in-place indent is just one of two unrelated layouts; and where Prettier keeps the comment in place and flat (`as`→binding, keyword/`type`→default/namespace binding, `type`→namespace-`*`, bare/empty/export-all `from`→source, no-`from` empty `{}`, `export`→declaration) tsv's indent is a deliberate indent-only divergence, chosen so every continuation reads alike.

For **declaration headers** the same rule covers the keyword→name gap of `function` (incl. `async function`, `function*`, and `declare function`), `class` (incl. `abstract class`), and `enum` (incl. `const enum`), plus the keyword→declarator gap of `const`/`let`/`var`. Prettier keeps the comment in place and flat for the `function`/`class`/`enum` headers, so tsv's indent is an indent-only divergence there; for `const`/`let`/`var` Prettier **agrees** (it also indents the declarator), so there is no divergence — a regular fixture.

The deliberate exclusions are constructs where the following token isn't part of the same declaration: **ASI statement keywords** (`return`, `throw`, `break`, `continue`, `yield`) — a newline after them triggers automatic semicolon insertion, so `return // c⏎expr` is two statements, not a continuation; the **contextual declaration keywords** (`type`/`interface`/`namespace`/`module`/`declare`), whose line forms never form a single declaration — the keyword requires its following token on the **same line** (tsc's `isDeclaration`: `nextTokenIsIdentifierOnSameLine`, and the ASI modifier rule for `declare`), so a line break demotes the keyword to a plain identifier. `interface`/`namespace`/`module`→name then become unparseable (`interface // c⏎I {}` is rejected); `type`→name and `declare`→keyword **ASI-split** into two statements (`type;` then `T = …`; `declare;` then `const x = …`). tsv's parser conforms here (it previously over-accepted these line forms) — see [contextual_keywords/declaration_keyword_own_line](../tests/fixtures/typescript/syntax/contextual_keywords/declaration_keyword_own_line/); and **function/class expressions** (`const a = function // c⏎f() {}`), which are not declaration headers and stay flat in both formatters. Block comments and the no-comment case are byte-identical in every in-scope gap. (The `export`-prefixed forms — `export interface // c⏎I` etc. — are out of scope: after `export` the parser is committed to a declaration, where tsc does not apply the same-line gate, so tsv's acceptance there may be spec-correct; acorn-typescript rejects them, but that is a separate acorn-vs-tsc question.)

**Keyword-paren comments**: `if/* c */(a)` → Prettier absorbs comments between keywords and `(` into the condition parens. Applies to `if`, `while`, `for`, `switch`, `catch`, `do...while`. tsv preserves the comment between keyword and paren: `if /* c */ (a)`. For `for await`, comments in either gap are preserved: `for /* a */ await /* b */ (x of y)` → prettier absorbs both into parens. The only divergence is the comment position — the loop body layout still matches Prettier regardless: an empty body attaches as `for /* k */ (a; b; c);` (no space before `;`), a block body hugs `) {`, and a non-block body stays inline when the header fits ([keyword_comment](../tests/fixtures/typescript/statements/for/keyword_comment_prettier_divergence/)). Both positions are dual-stable in our formatter.

**Condition-absorbed comments**: `switch(x)/* c */{}` → Prettier absorbs comments between `)` and `{` into the condition parens: `switch (x /* c */) {}`. Similarly, `catch/* c */(e)` → `catch (/* c */ e)`. tsv preserves position: `switch (x) /* c */ {}`, `catch /* c */ (e)`. Both positions are dual-stable in our formatter.

**Before-semicolon comments**: `const x = 1 /* c */;` → Prettier moves comments from before `;` to after: `const x = 1; /* c */`. tsv preserves the user's position. Both positions are dual-stable in our formatter.

**Type alias head to `=`**: `type A<X>\n// c\n= B | C` → Prettier relocates a line comment between a type alias head (name + optional type parameters) and `=` to after `=` (`type A<X> =\n// c\nB | C`). tsv preserves it before `=`, keeping the comment's association with the declaration head rather than the value. (With type parameters present, the comment is otherwise easily dropped entirely — content loss.) A single-line block comment before `=` stays inline in both formatters (`type A<X> /* c */ = B | C`), so it is not a divergence — only a line comment forces the value to the next line, and the two formatters disagree on which side of `=` it lands. Both positions are dual-stable in our formatter.

**Type parameter keyword to value (own-line)**: an own-line line comment between a type parameter's `extends`/`=` keyword and its constraint/default value (`U =\n// c\nV`, `T extends\n// c\nA`) is kept on its own line in the indented value block; Prettier pulls the first leading comment up onto the keyword line (`U = // c\n\tV`) and is **non-idempotent** doing so (its first pass leaves the value at the param indent, a second pass adds the extra indent). tsv stays idempotent and preserves the author's own-line placement. A comment that is **already** on the keyword line (`R extends A | B | void = // c\n…`) is dual-stable: tsv emits it inline via `line_suffix` (zero width), so a long trailing comment never forces a preceding constraint union to break — matching prettier. Only an own-line first comment diverges; the same-line cases are a regular fixture ([type_param_keyword_line_comment](../tests/fixtures/typescript/types/comments/type_param_keyword_line_comment/)). (Emitting the same-line comment as plain text would force-break the constraint union by its width — content added — and merge two line comments onto one line — boundary loss; the `line_suffix` rendering avoids both.) See [type_param_keyword_own_line_comment_prettier_divergence](../tests/fixtures/typescript/types/comments/type_param_keyword_own_line_comment_prettier_divergence/).

**Function parameter default to value**: a line comment after a parameter's `=` default, before the value (`function fn(p = // c\n\tv) {}`), is kept after `=` with the value on the next line; Prettier floats it out to trail the whole parameter, after the value (`function fn(\n\tp = v // c\n) {}`). tsv keeps the comment associated with the default rather than floating it past the value. The parameter's type-annotation union stays inline in both (the trailing comment is zero-width). Applies to function, arrow, and method parameters; a same-line block comment (`p = /* c */ v`) stays inline in both formatters and is not a divergence. Both forms are dual-stable in our formatter. See [param_default_line_comment_prettier_divergence](../tests/fixtures/typescript/declarations/function/param_default_line_comment_prettier_divergence/).

**`as`/`satisfies` cast keyword to type**: a line comment after the cast keyword, before the type (`x as // c\n\tA`), is kept after the keyword with the type on the next line; Prettier floats it out past the whole expression to a statement-trailing position (`x as A; // c`). tsv keeps the comment associated with the cast, on the keyword line via `line_suffix` with the type indented; emitting it inline instead would **swallow the cast type** (`x as // c A` — a non-idempotent content loss). A same-line block comment (`x as /* c */ A`) stays inline in both formatters and is not a divergence. Both forms are dual-stable in our formatter. See [as_satisfies_value_line_comment_prettier_divergence](../tests/fixtures/typescript/expressions/as_satisfies_value_line_comment_prettier_divergence/).

**Call open paren `(` trailing**: `fn( // c` / `fn( /* c */` (a comment on the same line as a call's opening `(`) → Prettier relocates it to its own line as the first argument's leading comment (`fn(\n\t// c\n\t…)`). tsv keeps it trailing the `(` (`fn( // c\n\t…)`), treating the author's placement after `(` as a trailing comment on that line. This applies only when the call expands (a line comment after `(`, or own-line content among the args); a block comment that hugs the arg in a call that stays inline (`fn(/* c */ a)`) is unchanged and matches Prettier. When the author instead writes the comment on its own line, both formatters keep it there — the two positions are dual-stable. Applies to simple-callee calls (`call_formatting.rs`), member-chain calls (`chain_args.rs`), and `new` expressions (`new_expression.rs`). For chains, a block comment trailing `(` plus an own-line leading comment keep source order (the naive handling reverses them). For `new`, a line comment trailing `(` (`new Foo( // c\n\ta)`) is preserved rather than dropped entirely (content loss); prettier instead floats it out to a statement-trailing comment (`new Foo(a); // c`) or relocates a block comment before `(`.

**Object/array/block open-delimiter trailing**: the same position-preservation rule as the call `(` case, generalized to the other opening delimiters. A comment on the same line as an object literal's `{` (`const o = { // c`), an array literal's `[` (`const a = [ // c`), a block body's `{` (`function f() { // c`, plain `{ // c`, arrow `=> { // c`), a type-parameter list's `<` (`function f< // c`, also classes/interfaces/type aliases/arrows), a function/constructor-type parameter list's `(` (`type Fn = ( // c`, `new ( // c`), an object/array destructuring pattern's `{`/`[` (`const { // c } = o`, `const [ // c ] = a`), a `namespace`/`module` body's `{` (`namespace N { // c`, `module M { // c`), a class/interface/enum body's `{` (`class C { // c`, `interface I { // c`, `enum E { // c`), a type literal's `{` (`type T = { // c`), an import/export specifier list's `{` (`import { // c`, `export { // c`), a tuple type's `[` (`type T = [ // c`), an index signature's `[` (`[ // c\n\tk: V]`), a computed property/member key's `[` (`{ [ // c\n\tfoo]: 1 }`, also class members, destructuring, and interface/type-literal members), or a multi-argument type-argument list's `<` (`Map< // c`) is kept on the delimiter line; Prettier relocates it to its own line as the leading comment of the first element/property/statement/parameter/member/specifier/argument (for a computed key prettier instead relocates it out to the **member's** own leading line, or — class members — leaves it glued flush to the key). This applies only to a **line** comment (or a block comment that co-occurs with one) when the construct expands; an inline block comment that hugs content (`{ /* c */ a: 1 }`, `[/* c */ x]`, `<T /* c */>`, `(/* c */ p)`, empty body `{ /* c */ }`) and an own-line block comment (which both formatters keep on its own line) are unchanged and match Prettier. When the author instead writes the comment on its own line, both formatters keep it there — dual-stable. Object literals are handled in `objects.rs`, arrays in `arrays.rs`, block bodies in the shared block-printing path (`expressions/blocks.rs`), type-parameter declarations in `types/type_params.rs` (covering function/class/interface/type-alias/arrow), function/constructor types in `types/function_types.rs`, object/array destructuring patterns in `expressions/patterns.rs`, `namespace`/`module` bodies in the shared statement-list walk (`statements/type_declarations.rs`, reusing `build_statement_list_docs`), class/interface/enum bodies in their member loops (`build_class_body_doc` in `statements/class.rs`; `build_type_elements_doc` and `build_enum_declaration_doc` in `statements/type_declarations.rs`), type literals in the multiline member path (`build_type_literal_doc_inner` → `build_multiline_member_prefix_doc`, `types/type_literal.rs`), import/export specifier lists in the shared multiline comma-list builder (`build_hardline_comma_list`, `statements/modules/specifier_list.rs`; the `with {…}` import-attribute brace passes `None` and still relocates), tuple types in their multiline element path (`build_tuple_type_doc_with_line_comments`, `types/composite.rs`, using `build_leading_comments_multiline_opt` for the first element), index signatures in `build_index_signature_member_doc` (`types/type_members.rs`), computed property/member keys in `build_computed_key_bracket_doc` (`expressions/objects.rs`, its breaking path — a computed key never breaks on width, so a line comment in either in-bracket gap, `[`→key (this case) or key→`]` (the [close-bracket sibling](#comment-relocation)), is the only trigger; block comments and the no-comment case keep the flat layout), and multi-argument type-argument lists in their multiline path (`build_type_arguments_doc_with_line_comments`, `types/type_arguments.rs` for _type_ position; `build_type_parameter_instantiation_doc_with_line_comments`, `types/type_params.rs` for call/`new` _expression_ position — the single-argument leading-comment case hugs `<`/`>` and matches prettier) — all via the shared `Printer::delimiter_line_comment_prefix` helper (`comments/lists.rs`, wrapping `PartitionedComments` + `should_force_expansion_for_comments`). For the type-param `<` and function/constructor-type `(`, preserving the comment also fixed a prior content-loss/correctness bug (the `<` line comment was dropped; the `(` line comment swallowed the following tokens). **Within-scope exceptions** (covered constructs are enumerated above): for type literals, the standard path (type aliases, annotations, function-param literals, intersection-trailing objects) is covered, but the union-member / parenthesized-intersection _alignment_ rendering (`type T = | { // c } | B`) still relocates; type-argument lists are covered in both _type_ position (`Map<A, B>`) and call/`new` _expression_ position (`foo<A, B>(x)`, `new Foo<A, B, C>(x)`), single-argument lists hugging and already matching prettier; the `with {…}` import-attribute brace still relocates; and the method/call/construct _signature_ `(` still relocates to match prettier (both formatters agree there today) — extending the divergence to it is a consistent future increment, not a bug.

**Declaration keyword comments**: `abstract/* b */class B` → Prettier collects all comments between modifier keywords and emits them before the declaration name: `abstract class /* b */ B`. Similarly, `async /* c */ function* F()` → `async function* /* c */ F()`. tsv preserves comments between their original keywords. Both positions are dual-stable in our formatter.

**Anonymous function keyword comments**: `function /* c */ ()` → Prettier relocates comments between the keyword and `(` in anonymous function expressions, generators, and export default functions. No params: after `)` before `{`. With params: inside parens before first param. tsv preserves the comment between keyword and parens. Prettier's relocated forms are dual-stable (stable in both formatters); our keyword-adjacent form is only stable in our formatter.

**Anonymous function/class keyword line comments**: Same relocation as block comments above, but with line comments. `class // c\n{}` → Prettier moves into body: `class {\n\t// c\n}` (stable in one pass). `function // c\n()` → Prettier moves after parens: `function () // c\n{}` (pass 1), then into body: `function () {\n\t// c\n}` (pass 2, stable). Not idempotent for functions — takes 2 passes. With params: `function // c\n(x)` → Prettier moves into params: `function (\n\t// c\n\tx\n)` (stable in one pass). Also applies to generators (`function*`), async functions, and export default. tsv preserves the comment between keyword and next token. Both positions are dual-stable.

**Arrow body stripped parens**: `() => (x /* c */)` → Prettier strips parens and moves comment to params (`(/* c */) => x`). For curried arrows, parens are stripped and comment trails (`(a) => (b) => z /* c */;`). tsv preserves parens to keep comments in place: `() => (x /* c */)`, `(a) => ((b) => z /* c */)`. Same approach as unary expression fix. Each formatter normalizes to its own form (prettier strips parens, tsv preserves them).

**Sequence last-operand trailing edge (statement context)**: when a redundantly-parenthesized sequence is a statement's whole right-hand side, a trailing comment on the last operand (`const b = (x, (y /* c */));`) floats out of the sequence parens to sit right before the terminating `;` (`const b = (x, y) /* c */;`). Prettier floats it one step further — _past_ the `;` (`const b = (x, y); /* c */`), drifting across three passes to get there. tsv keeps it before the `;`, preserving the comment's association with the operand, consistent with its broader before-semicolon handling (the **Before-semicolon comments** entry above). The leading-edge counterpart (`const a = ((/* c */ x), y)` → `const a = /* c */ (x, y)`) reaches prettier's fixed point and is _not_ a divergence; and in call-expression context — where no `;` sits at the edge — both edges match prettier's fixed point (see the normalization-quirk entry **Sequence operand edge comment** / `sequence/operand_edge_comment`). _Interior_ operand comments (between two operands, `(x /* c */, /* c */ y)`) stay inline without parens and match prettier (regular fixture `sequence/operand_comments`).

**Conditional type after `:`**: `? foo : // about bar` → Prettier moves comment to trailing on true branch (`? foo // about bar`), changing association from false to true branch. Both positions are dual-stable.

**Switch case before `{`**: `case 'a': // comment {` → Prettier moves comment after opening brace. Not idempotent—takes 2-3 passes to stabilize.

**For empty clauses**: Prettier puts comments in syntactically broken positions outside the parentheses.

**Do-while after `(`**: Prettier moves comments after `} while (` to after the semicolon. Unique to do-while; other constructs keep comments inside parens.

**Do-while between `)` and `;`**: `} while (x) /* c */;` → Prettier relocates the comment inside the condition parens — a block comment before `)` (`} while (x /* c */);`), a line comment forcing the condition to break. tsv keeps it after `)`. (Otherwise the comment is dropped entirely — content loss.)

**Export `}` to `;` (no `from`)**: `export { a as x } /* c */;` → Prettier relocates the comment inside the specifier braces — a block comment trailing the last specifier (`export { a as x /* c */ };`), line comments forcing the braces to break with each attached inside. tsv keeps them after `}`: a same-line block comment trails the brace, line comments stay on their own line with `;` following. Only the no-`from` case diverges — with a `from` clause prettier keeps the comment after the source (`export { a } from './m' /* c */;`), so tsv matches prettier there. (Otherwise the comment is dropped entirely — content loss.)

**Import source to `;`**: `import { a } from './a' /* c */;` → Prettier keeps a same-line block comment after the source (matching tsv), but relocates a line comment past the `;` (`import { a } from './a'; // c`, the before-semicolon rule). tsv preserves the line comment before `;` (`import { a } from './a' // c\n;`). A block comment after `with {...}` import attributes is relocated by prettier _inside_ the attribute braces (`with { type: 'json' /* c */ }`); tsv keeps it after `}`. The same before-semicolon handling applies to re-exports (`export * from './a' // c\n;`) and import-equals references (`import x = require('./a') // c\n;`). (Otherwise the comments are dropped entirely — content loss.)

**Try/catch/finally before `{`**: Prettier absorbs line comments between keyword/paren and block into the block body. For catch, the comment gets absorbed into the parameter parens (`catch (\n\te // comment\n)`).

**While before `{}` (absorbed)**: Prettier absorbs comments between `)` and `{}` into the block body, expanding the empty block. Unique to while — `if (a) /* c */ {}` stays put.

**Between `}` and `else`**: Prettier cuddles `} else` and moves comments that were between the closing brace and the `else` keyword into the else block body. This applies to both own-line comments (`}\n/* c */\nelse`) and comments leading the `else` keyword on the same line (`/* c */ else`). tsv preserves the comments before the `else` keyword on their own line.

**Trailing member chain**: When line comments appear before trailing member access in chains (`.length` after `.filter()`), Prettier relocates the comment to after the `=` and keeps the chain inline. For a plain member (`.length`) it puts the comment on its own line under `=`; for an **optional**-chain trailing member (`?.length`) it instead trails the comment on the `=` line itself (`const b = // comment`) and de-indents the value. tsv keeps the comment before the trailing member with the chain broken in both cases.

**Block comment in computed `[]`**: When a computed member access with a block comment exceeds print width, Prettier hoists the comment from inside the brackets to before the member chain. Not idempotent.

**Switch case colon comment**: Prettier relocates comments near the colon in switch cases: `case 1: /* c */` → `case 1 /* c */:` (moved before colon); `default /* c */:` → `default: /* c */ break;` (moved to body). tsv preserves comment placement. Both positions are dual-stable in our formatter. Three stable forms exist: our input form, prettier's `output_prettier` form, and a third body-only form.

**Class property definite `!`**: `d! /* c */ = 1;` → Prettier moves comment before `!` (`d /* c */! = 1;`). tsv preserves comment after `!`. Both positions are dual-stable.

**Class property modifier**: Both positions are dual-stable in both formatters. Our canonical form puts the comment after the modifier (`a? /* c */ = 1;`), Prettier's canonical form puts it before (`a /* c */? = 1;`). The user's chosen position is preserved. Same for `!`. Note: when a type annotation follows (`a /* c */?: T`), preserving the comment is a correctness matter, not a divergence — it is otherwise dropped entirely (content loss).

**Interface member after `?`**: `a? /* c */ : number;` → Prettier moves before `?` (`a /* c */?: number;`). `b? /* c */(x): void;` → Prettier moves inside parens (`b?(/* c */ x): void;`). tsv preserves both after `?`. Both positions are dual-stable.

**Type-literal member after `?`**: the type-literal counterpart of the interface case — `type T = { a? /* c */ : number }` → Prettier moves before `?` (`a /* c */?: number`), and `m? /* c */(x): void` → inside the parens (`m?(/* c */ x): void`). tsv preserves both after `?`, the same way the interface arm does (the two type-element printers now split around `?` consistently). Both positions are dual-stable. A method that has **type parameters** is not a divergence: there Prettier keeps the comment after `?` (`m?/* c */ <T>(…)`), so both formatters agree — a regular fixture pins it together with a comment *inside* `<>` (`m<T /* c */>`, also kept in place by both): `types/type_members/method_type_params_comment`.

**Class method after `?`**: `m? /* c */(x): void {}` → Prettier moves the comment before `?` (`m /* c */?(x): void {}`), regardless of params — unlike interface/type-literal method signatures, which move it into the parens. tsv preserves it between `?` and `(`. Both positions are dual-stable. (A class _property_ with the comment after `?` and a type annotation is a match — prettier preserves it too — see `statements/class/property_modifier_type_comment`.)

**Optional `?` to `:` line comment**: a line comment in the gap between an optional `?` and the member's `:` annotation (`a? // c\n: number`) → Prettier relocates it to trail the member `;` (`a?: number; // c`). tsv preserves it after `?`; because a line comment must end its line, the annotation is forced onto the next line. Applies to interface members, type-literal members, and class properties. Preserving the comment is also a content-loss fix: emitting it inline would swallow the `: number` annotation as comment text (non-idempotent). Both positions are dual-stable in our formatter.

**Member key to `:` line comment (non-optional)**: the no-marker counterpart of the case above — a line comment between a (non-optional) property key and its `:` annotation (`a // c\n: number`) → Prettier relocates it to trail the member `;` (`a: number; // c`). tsv preserves it after the key, the annotation forced onto the next line. Applies to interface members, type-literal members, and class properties. Like the optional case this is also a content-loss fix (emitting the line comment inline would swallow the `: number` annotation — non-idempotent); a same-line block comment in the same gap stays inline in both formatters (`a /* c */: number`) and is not a divergence. Both positions are dual-stable in our formatter.

**Variable definite `!`**: `let a /* c */!: number;` → Prettier moves comment after `!` (`let a! /* c */ : number;`). tsv preserves before `!`. Both positions are dual-stable.

**Function param optional `?`**: `function fn(a /* c */?: number) {}` → Prettier moves comment after `?` (`function fn(a? /* c */ : number) {}`). tsv preserves before `?`. Both positions are dual-stable. Same pattern applies to arrow function params.

**Computed key after `]`**: `[x] /* c */ = 1` → Prettier moves inside brackets (`[x /* c */] = 1`). Applies to object literals, class members, interface members, and destructuring patterns. For interface `set` accessors, prettier moves into params instead (`set [x](/* c */ a)`). tsv preserves between `]` and the next token. Both `[x /* c */]` and `[x] /* c */` forms are dual-stable.

**Heritage last item before `{`**: `class A implements I, J // c {}` → Prettier relocates line comment from after the last heritage item into the class/interface body (`J {\n\t// c\n}`). tsv preserves the comment before `{` with a forced line break (`J // c\n{}`). When more than one comment precedes `{`, each is kept on its own line (`J // c1\n// c2\n{}`) — collapsing them onto the heritage line would absorb a following comment into the first line comment's text (`// c1 // c2` reparses as one comment), a content/boundary loss, not just a position change. Affects class `implements`, class `extends`, class expressions, and interface `extends`; the same preservation applies to the interface name/type-params→body gap when there is no `extends`. Consistent with tsv's handling of line comments before block bodies across all statement types.

**Type params to `(`**: `<T> /* c */(x: T)` → Prettier moves inside parens as leading comment on first param (`<T>(/* c */ x: T)`). Prettier's behavior is context-dependent: for function declarations/expressions and class methods **with a body**, prettier preserves the comment between `>` and `(` (no divergence). For body-less declarations (overloads, abstract methods, interface/type literal method signatures, call/construct signatures, function/constructor types, declare functions) and arrow functions, prettier moves the comment inside parens. tsv preserves between `>` and `(` in all cases. Both positions are dual-stable.

**Union infix `|` line comment**: `A | // c\n B` (a line comment trailing the infix `|`) → Prettier relocates the comment to trail the previous member (`| A // c\n| B`). tsv keeps it on the separator/`B` side, on its own line so the pipe stays attached (`| A\n// c\n| B`) — the comment sits after the `|`, so tsv associates it with `B` rather than `A`. Both forms are dual-stable; the divergence is in how the infix-pipe input normalizes.

**Retained paren union member comment**: a block comment inside a parenthesized union member whose parens are **retained** — because the member nests in an outer union or intersection (`a | (b | c /* c */)`, `(a | b /* c */) | c`, `a & (b | c /* c */)`) → Prettier hoists the comment out of the parens (a trailing comment after `)`, a leading comment before `(`: `a | (b | c) /* c */`, `a | /* c */ (b | c)`). tsv keeps it inside the parens, associating it with the parenthesized member. (Otherwise the comment is dropped entirely — content loss.) When the parens are redundant and stripped — a top-level or single-member union — both formatters keep the comment in place (no divergence; see `union_intersection_parens_comment`). The parenthesized-_intersection_-in-union member (`a | (b & c /* c */)`) already preserves in place through a separate path. Both positions are dual-stable in our formatter.

**Retained paren union member line comment**: the line-comment analog of the above — a line comment trailing the last inner member of a retained parenthesized union (`(a | b // c) | c`, `a | (b | c // c) | d`) → Prettier hoists it out to trail the whole member and keeps the inner union inline (`| (a | b) // c`). tsv keeps it inside the parens; because a line comment must end its line, the parenthesized union expands to its broken form (one member per line) with `)` on its own line. (Otherwise the comment is dropped entirely — content loss.) Unlike the block-comment case — which stays inline because a block comment can — the line comment forces the expanded layout. Our expanded form is stable in tsv; Prettier's inline-with-relocated-comment form is its own stable shape.

**Retained paren first-member leading line comment**: a **leading** line comment inside a retained parenthesized union member, when that member is the **first** member of the outer union (`(// c\n A | B) | C`) → Prettier moves the comment out of the parens to lead the member, keeping the inner union inline when it fits (`| // c\n (A | B)`). tsv keeps it inside the parens leading the inner union; because a line comment must end its line, the parenthesized union expands to its broken form with `)` on its own line. (Otherwise the comment is dropped entirely — content loss: the inner-leading line comment has no previous member to relocate onto.) This is the leading-comment counterpart of the trailing **Retained paren union member line comment** above, and mirrors its keep-inside behavior. A leading line comment inside a **later** member's parens instead relocates to trail the previous member, where both formatters agree (see the [Tabs-Only Alignment](#tabs-only-alignment) `union_paren_member_long_line_comment` fixture); only the first member, lacking a previous member, keeps the comment inside.

**Retained paren intersection member comment**: the intersection counterpart of the retained-paren-union case — a block comment inside a parenthesized **intersection** member whose parens are retained because it nests in an outer union (`(a & b /* c */) | c`, `a | (/* c */ b & c)`, `a | (b & c /* c */)`) → Prettier hoists the comment out of the parens (trailing after `)`, leading before `(`: `(a & b) /* c */ | c`, `a | /* c */ (b & c)`). tsv keeps it inside the parens, associating it with the parenthesized member. (Unlike the union case this never dropped — it preserves through the paren-unwrapping path — but it is the same comment-position divergence and is pinned for completeness.) Both positions are dual-stable in our formatter.

#### JSDoc / paren semantics

- JSDoc type cast parens (standalone TS) — [jsdoc_type_cast_ts](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_ts_prettier_divergence/)
- JSDoc type cast in Svelte template / directive positions — [jsdoc_cast_template](../tests/fixtures/svelte/syntax/comments/jsdoc_cast_template_svelte_prettier_divergence/)

**JSDoc type cast parens.** `/** @type {T} */ (expr)` is a TypeScript type **assertion** (cast): the parentheses are required, and `/** @type {T} */ expr` (no parens) is *not* a cast. tsv therefore **preserves** the parens of a JSDoc cast everywhere — the parser records the cast in an internal `JsdocCast` wrapper node, keyed on the exact paren extent, so `(a.b)` (cast the member) and `(a).b` (cast the base) stay distinct and nested casts keep each level. The public AST stays paren-free, matching acorn/Svelte (which carry no `ParenthesizedExpression`). tsv never *adds* parens — only a `@type`/`@satisfies` block comment immediately before a `(` triggers preservation; a non-`@type` comment, or one not adjacent to a `(`, is stripped as ordinary grouping.

The divergence is **JS-vs-TS**, keyed on prettier's parser backend rather than on file type:

- **JS contexts** (`.js`, `.svelte` plain `<script>` — prettier's babel parser) **preserve** the cast parens too: prettier's `is-type-cast-comment.js` + the `postprocess` `shouldKeepParenthesizedExpression` check keep a `ParenthesizedExpression` whose immediately-preceding comment is a type cast. tsv **matches** here — see the match fixtures [jsdoc_type_cast_svelte](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_svelte/), [jsdoc_type_cast_extent](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_extent/), [jsdoc_type_cast_nested](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_nested/), and [jsdoc_type_cast_keyword](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_keyword/). Because tsv formats context-free, these JS matches also **oracle-anchor the preserved layout** that the TS divergence shares — the wide cases ([jsdoc_cast_call_arg_long](../tests/fixtures/typescript/expressions/calls/jsdoc_cast_call_arg_long/), [jsdoc_cast_multiarg_long](../tests/fixtures/typescript/expressions/calls/jsdoc_cast_multiarg_long/), [jsdoc_cast_arg_long](../tests/fixtures/typescript/expressions/calls/jsdoc_cast_arg_long/), [jsdoc_cast_paren_long](../tests/fixtures/typescript/calls/chained/jsdoc_cast_paren_long/), [jsdoc_cast_before_eq](../tests/fixtures/typescript/declarations/variable/jsdoc_cast_before_eq/), [jsdoc_cast_paren_span](../tests/fixtures/typescript/declarations/variable/jsdoc_cast_paren_span/), [arrow_jsdoc_cast_body_long](../tests/fixtures/typescript/calls/arrow_jsdoc_cast_body_long/)) pin how the preserved parens break against prettier-plugin-svelte rather than against tsv's own output.
- **TS contexts** (`.ts`, `.svelte` `<script lang="ts">` — prettier's oxc-ts parser) **strip** the parens: oxc-ts is cast-unaware (the preservation is gated behind `if (!isOxcTs)`), so prettier silently drops the assertion. tsv preserves → **divergence** (the fixtures listed above). When the inner expression needs parens anyway (e.g. an assignment in `return`), prettier keeps them in both backends, so that case stays a match.

tsv runs one context-free TypeScript formatter, so "preserve" is uniform: it matches prettier in JS contexts and diverges from prettier-TS's strip. This is the **more correct** behavior, not merely a taste call — the parens carry meaning, and prettier's own babel path special-cases preserving them (it strips meaningless parens aggressively everywhere else). The cast paren is **opaque to layout heuristics** (expand-last etc.), mirroring acorn's `ParenthesizedExpression`, so a long cast breaks via standard arg breaking and the inner expands inside the preserved parens. An object/array literal inner **hugs** the cast parens (`({…})` / `([…])`) — see [jsdoc_type_cast_object](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_object/) — while a line comment between the `(` and the inner forces a hardline so it can't swallow the inner — see [jsdoc_type_cast_gap_comment](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_gap_comment/). Negatives that pin the boundary: [jsdoc_type_cast_member](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_member/) and [jsdoc_type_cast_nonadjacent](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_nonadjacent/) (comments that are not casts), [jsdoc_type_cast_word_boundary](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_word_boundary/) (the ASCII `\b` after `@type`/`@satisfies`, mirroring prettier's regex), and [jsdoc_type_cast_ts](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_ts/) (never-add + no double-wrap in standalone TS).

**Svelte template / directive positions are not covered by the JS-vs-TS split.** The "JS preserves" rule above holds only inside `<script>` (prettier's babel/oxc path). In a Svelte **template** — attribute values (`title={…}`), block tests (`{#if …}`), and mustache tags (`{…}`, `{@html …}`) — prettier-plugin-svelte routes the expression through a path that **strips** the cast even in a plain (JS) component. tsv preserves uniformly → divergence, pinned by [jsdoc_cast_template](../tests/fixtures/svelte/syntax/comments/jsdoc_cast_template_svelte_prettier_divergence/) (also a `_svelte_divergence` — the cast comment's `leadingComments` attachment differs from Svelte; see [conformance_svelte.md](./conformance_svelte.md) §Comment Attachment Differences). The `{@const}` case is a prettier **bug**: `{@const y = /** @type {T} */ (z)}` makes prettier-plugin-svelte emit invalid output `(z}` (it drops the `)`) and then throw on its own output; tsv preserves it correctly, so it is documented here but cannot be pinned as an `output_prettier.*` oracle.

#### Comment normalization (stable quirks)

Prettier has multiple stable forms for comment positioning. tsv normalizes to a single canonical form.

- Computed access comment — [trailing_member_computed_comment](../tests/fixtures/typescript/expressions/calls/chained/trailing_member_computed_comment_prettier_divergence/)
- Sequence operand edge comment — [operand_edge_comment](../tests/fixtures/typescript/expressions/sequence/operand_edge_comment_prettier_divergence/)
- Block comment mid-chain — [block_comment_chain](../tests/fixtures/typescript/expressions/calls/chained/block_comment_chain_prettier_divergence/)
- Intersection leading line comment — [intersection_leading_line_comment](../tests/fixtures/typescript/types/intersection_leading_line_comment_prettier_divergence/)
- Property signature leading line comment — [annotation_simple](../tests/fixtures/typescript/types/comments/annotation_simple_prettier_divergence/)
- Property signature leading block — [annotation_leading_block](../tests/fixtures/typescript/types/comments/annotation_leading_block_prettier_divergence/)

**Computed access comment**: Prettier requires 2 passes to stabilize line comments before computed access (`[0]`). The intermediate form places the comment inside brackets (`[// comment\n0]`), which then normalizes to end-of-line (`[0]; // comment`). tsv reaches the stable form in one pass.

**Sequence operand edge comment**: a redundantly-parenthesized sequence operand carrying a comment on its outer edge — leading on the first operand (`fn(((/* c */ x), y))`) or trailing on the last (`fn((x, (y /* d */)))`) — has the comment floated out of the sequence parens, matching prettier's fixed point (`fn(/* c */ (x, y))`, `fn((x, y) /* d */)`). Prettier reaches the same forms but is non-idempotent getting there — two passes (the comment stays inline on pass 1, floats on pass 2) — while tsv reaches the fixed point in one pass; the user's paren form is documented as `unformatted_ours_paren` paired with prettier's first-pass `prettier_intermediate_paren`. Each floated comment keeps its source line-treatment (own-line → own line via hardline, inline → inline via space; the trailing one defers via `line_suffix` past the enclosing comma so it re-parses to the same place), which is what makes the one-pass float idempotent even when the sequence is nested inside other comments. In statement context the trailing edge instead lands before the `;` — a genuine divergence; see [Sequence last-operand trailing edge](#comment-relocation) / `sequence/operand_edge_comment_stmt`. Interior operand comments (between two operands) stay inline and match prettier — see the regular fixture `sequence/operand_comments`.

**Block comment mid-chain**: When nested grouping parens with block comments are stripped on a member chain (e.g., `/* outer */ (/* inner */ (a).b).c(fn)`), Prettier repositions the inner comment mid-chain (`a /* inner */.b`) and breaks the chain. Prettier requires 2 passes to stabilize the spacing: pass 1 produces `a/* inner */ .b`, pass 2 produces `a /* inner */.b`. Both passes break the chain at the same point. Not JSDoc-specific — any block comment before stripped grouping parens triggers this. tsv normalizes directly to the stable form in one pass.

**Intersection leading line comment**: When a leading line comment precedes the first member of an intersection type (`(// leading\n a) & b`), Prettier requires 2 passes to stabilize. Pass 1 strips the parens but breaks the intersection across lines (`// leading\n a &\n   b`); pass 2 collapses it to inline (`// leading\n a & b`). The same pattern applies when the inner type is a parenthesized union (`(// leading\n a | b) & c`). tsv normalizes directly to the stable inline form in one pass.

**Property signature leading block**: A block comment between `:` and the type in a property signature has two intentional stable positions (`a: /* block */ X;` after `:`, and `a /* block */: X;` before `:`); both formatters preserve each when given as input. The divergence is in normalizing **unstable** layouts — when the user breaks the line around the comment (`a: /* block */\n X;` or `a:\n /* block */\n X;`), tsv compacts to the inline form after `:`, while prettier eventually relocates the block before the `:` (sometimes via a multi-pass convergence). Neither choice is information-destructive; this is purely about which canonical target to favor for ambiguous inputs.

**Property signature leading line comment**: For a line comment between `:` and an inline-renderable type in a property signature (`{ prop: // c\n X }` — covers identifiers, optional `?:`, readonly, computed keys, generics like `Array<X>`, tuples, function types, `typeof`, etc.), Prettier moves the comment past the implicit `;` to end-of-line (`prop: X; // c`); tsv keeps the comment after `:` and drops the type to a continuation line indented one level (`prop: // c\n\t X;`, the [Uniform Forced-Continuation Indent](#uniform-forced-continuation-indent)). Both forms are stable under their own formatter. A multi-member **union** in the same position is a **match** — both formatters indent the continuation (the non-divergent [annotation](../tests/fixtures/typescript/types/comments/annotation/) fixture); a multi-member **intersection** instead **diverges** (prettier keeps it flush, tsv indents — see [annotation_continuation_indent](../tests/fixtures/typescript/types/comments/annotation_continuation_indent_prettier_divergence/)). Notably, prettier's end-of-line motion is information-destructive when more than one comment touches the property: leading line + trailing line collapses to `f: X; // leading // trailing` (second `//` becomes text inside the first comment); two leading lines merge **and reverse** order (`g: // c1\n // c2\n X;` → `g: // c2 // c1\n X;`); leading line + trailing block reorders to `h: X; /* trailing */ // leading`. tsv preserves each comment at its authored position as a separate comment node. The end-of-line **relocation** is property-signature-only — prettier keeps variable declarations (`const e: // c\n X = ...`) and class properties (`class C { prop: // c\n X }`) in place — but tsv's continuation **indent** is universal across all these contexts, so those keep-in-place cases become an indent-only divergence too (the same [annotation_continuation_indent](../tests/fixtures/typescript/types/comments/annotation_continuation_indent_prettier_divergence/) fixture).

### Format-ignore directive

A comment can suppress formatting of the construct that follows it. tsv honors its own tool-neutral `format-ignore` family — `<!-- format-ignore -->`, `// format-ignore`, `/* format-ignore */`, and the range markers `format-ignore-start` / `format-ignore-end` — **in addition to** prettier's `prettier-ignore` family, which tsv keeps for drop-in compatibility (corpus files use it). Recognition is centralized in `tsv_lang::is_format_ignore_directive` and the two range predicates, shared across the TypeScript, CSS, and Svelte printers.

The `prettier-ignore` family matches prettier exactly (both emit the construct raw), so it needs no divergence fixture of its own. The `format-ignore` family is tsv-native: prettier doesn't recognize it, so prettier reformats the construct while tsv preserves it — that difference is the divergence. Most fixtures pair the spellings in one input: a `prettier-ignore`d construct (preserved by both tools, so unchanged in `output_prettier`) sits beside a `format-ignore`d one (reformatted only by prettier), making the `format-ignore` construct the sole divergence and doubling as a drop-in-compat check. The `basic` (template node) and `js_css` (embedded `<script>` + `<style>`) Svelte fixtures carry this control, as do both standalone fixtures.

- `format-ignore` in `<script>` / `<style>` — Design choice — [js_css](../tests/fixtures/svelte/syntax/format_ignore/js_css_prettier_divergence/)
- `format-ignore` template element — Design choice — [basic](../tests/fixtures/svelte/syntax/format_ignore/basic_prettier_divergence/)
- `format-ignore` nested CSS — Design choice — [css_nested](../tests/fixtures/svelte/syntax/format_ignore/css_nested_prettier_divergence/)
- `format-ignore` at-rule-body declaration — Design choice — [css_atrule_decl](../tests/fixtures/svelte/syntax/format_ignore/css_atrule_decl_prettier_divergence/)
- `format-ignore-start` / `-end` range — Design choice — [range](../tests/fixtures/svelte/syntax/format_ignore/range_prettier_divergence/)
- `format-ignore` standalone `.ts` — Design choice — [ts_standalone](../tests/fixtures/typescript/syntax/comments/format_ignore_prettier_divergence/)
- `format-ignore` standalone `.css` — Design choice — [css_standalone](../tests/fixtures/css/syntax/comments/format_ignore_prettier_divergence/)

The first five are Svelte-embedded; the last two pin the **standalone**
`.ts` / `.css` paths (acorn-typescript / `parseCss` + `tsv_ts` / `tsv_css`
directly), so the directive is covered in every language outside a Svelte host
too.

See [directives.md](./directives.md) for the user-facing reference.

---

## Tooling

**Corpus comparison** validates formatting against Prettier on real codebases:

```bash
deno task corpus:compare:format --all --explain           # All default corpus repos (~5600 files)
deno task corpus:compare:format ~/dev/project --explain  # Single project (scans all files recursively)
```

**Divergence audit** (static check) verifies all documented divergences have registered detectors:

```bash
deno task divergence:audit  # Cross-refs pattern fixture lists vs this doc (no runtime)
```

Every pattern in `benches/js/lib/divergence/patterns.ts` links to:

- `conformance_sections` — Section names from this document
- `fixtures` — Fixture paths the pattern detects (enforced by the behavioral
  fixture-coverage audit in `deno task test:deno`)

See ./divergence_detector.md for implementation details.

**Triage caveat — prettier-plugin-svelte's verbatim fallback**: when the
embedded formatter throws on any construct in a `<script>` block,
prettier-plugin-svelte emits the **whole block verbatim** instead of failing.
The plugin routes `<script lang="ts">` through prettier's babel-based
`babel-ts` parser, so the trigger is babel rejecting the code — e.g.
`@(f()).g` is a babel SyntaxError (babel follows the strict TC39 decorator
grammar; tsc accepts it). **Both tsv pipelines disarm this with
`PRETTIER_DEBUG=1`** (the tsv_debug sidecar sets it on the Deno spawn; the
`corpus:compare:format:run` task sets it in its env), which makes the plugin
and prettier-core rethrow — so `compare`, fixture validation,
`fixtures_update`, and corpus runs all report a hard prettier error (with a
code frame) instead of fake-stable output. The caveat applies when probing
prettier **outside** these pipelines (a bare `prettier` invocation, editor
integrations, upstream issue repros): there the fallback silently "preserves"
the whole script. Forms that only crash prettier's `typescript`
parser (e.g. `@(a?.b)()`, a `TypeError` in needs-parens) do **not** trigger
the fallback in `.svelte` — babel-ts accepts them and the script formats
normally; they fail visibly on pure-`.ts` runs instead, where no fallback
exists. Confirm by re-running the suspect construct in a single-form file or
as pure `.ts`. (Also see
[fixture_overview.md §Common Pitfalls](./fixture_overview.md#common-pitfalls) —
the fallback can fake a "prettier-stable" fixture input.)

---

## Related

- ./conformance_svelte.md — Svelte parser differences
- ./fixture_overview.md — Fixture system details
