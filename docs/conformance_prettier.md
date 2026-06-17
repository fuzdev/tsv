# Prettier Conformance

The tsv formatter tracks Prettier closely, matching its output except where it **intentionally differs**. This document catalogs those divergences.

## Terminology

**Matched**: tsv produces identical output to Prettier — the goal and the common case (measure current rates with `deno task corpus:compare:format --all --summary`).

**Unmatched**: tsv produces different output. The suffix `_prettier_divergence` marks these fixtures. This document explains WHY for each case.

## Reasons tsv Differs

| Reason                    | Description                                             | tsv action                            |
| ------------------------- | ------------------------------------------------------- | ------------------------------------- |
| **Spec violation**        | Prettier violates CSS/HTML/JS spec                      | tsv follows the spec                  |
| **Stable quirk**          | Prettier preserves multiple forms without normalizing   | tsv normalizes consistently           |
| **Prettier bug**          | Prettier is non-idempotent or emits invalid output      | tsv produces stable, valid output     |
| **Parser compat**         | Prettier's output breaks Svelte's parser                | tsv produces Svelte-compatible output |
| **Print width**           | Prettier allows lines to exceed printWidth              | tsv breaks to stay within limit       |
| **Tabs-only indent**      | Prettier mixes tabs and spaces under --use-tabs         | tsv uses whole tabs only              |
| **BOM stripping**         | Prettier preserves byte-order marks                     | tsv strips them                       |
| **Semantic preservation** | Prettier changes meaning (strips parens)                | tsv preserves original semantics      |
| **Comment preservation**  | Prettier moves comments to different syntactic position | tsv preserves comment position        |
| **Content preservation**  | Prettier silently drops user comments                   | tsv preserves all comments            |
| **Design choice**         | Other deliberate behavior differences                   | Documented rationale in fixture       |

> Most `Comment preservation` and `Content preservation` divergences live in the prose-form [TypeScript: Comments](#typescript-comments) and [CSS: Comments](#css-comments) catalogs, not the Reason-column tables — they're the largest divergence category but don't fit a one-word table cell.

## Decision Framework

**When to match Prettier:**

- Cosmetic choices (spacing preferences, quote styles)
- Output that's valid and reasonable
- Unclear which approach is "better"

**When to differ:** any reason in [Reasons tsv Differs](#reasons-tsv-differs) above. The three cross-cutting principles — comment position, print width, and tabs-only indentation — are detailed below.

### Comment Position Philosophy

**A formatter should not move comments to different syntactic positions.** Comment
placement is a deliberate authoring choice — it communicates what the comment refers
to. This is the single largest category of divergence (see [TypeScript: Comments](#typescript-comments)).

Prettier's comment handling is its weakest area. It routinely moves comments from
between syntactic boundaries into adjacent blocks, parens, or other positions, changing
the apparent association. tsv treats comment position as semantic and preserves it.

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

**When reviewing comment-related fixes:** Default to preserving position. Only match
Prettier's repositioning when the original position is clearly wrong (e.g., comment
inside a token boundary). Create `_prettier_divergence` fixtures for cases where
Prettier moves comments and we preserve position.

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
  `*`→`as`, and every other header gap (`import // c⏎{a} from 'm'`,
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
  ([index_signature_key_type_line_comments](../tests/fixtures/typescript/types/type_members/index_signature_key_type_line_comments_prettier_divergence/))
  and value-type
  ([index_signature_value_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_value_line_comment_prettier_divergence/)).
- **Before-`:` key/binding gap** — the complement of the colon→type case: a line
  comment between a key/binding name (or its `?`/`!` marker) and the `:`
  (`prop // c⏎\t\t: T`) keeps the comment after the marker and indents the whole
  `: type` continuation one level, via the shared `build_marker_colon_line_continuation`.
  Uniform across index signatures
  ([index_signature_key_colon_line_comment](../tests/fixtures/typescript/types/type_members/index_signature_key_colon_line_comment_prettier_divergence/)),
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

| Feature                | Reason         | Fixture                                                                                                                |
| ---------------------- | -------------- | ---------------------------------------------------------------------------------------------------------------------- |
| @container spacing     | Spec violation | [container_spacing](../tests/fixtures/css/at_rules/container_spacing_prettier_divergence/)                             |
| @container line wrap   | Print width    | [container_long](../tests/fixtures/css/at_rules/container_long_prettier_divergence/)                                   |
| @import line wrap      | Print width    | [import_media_query_long](../tests/fixtures/css/at_rules/import_media_query_long_prettier_divergence/)                 |
| @media boolean spacing | Spec violation | [media_boolean_spacing](../tests/fixtures/css/at_rules/media_boolean_spacing_prettier_divergence/)                     |
| @media line wrap       | Print width    | [media_long](../tests/fixtures/css/at_rules/media_long_prettier_divergence/)                                           |
| @scope whitespace      | Stable quirk   | [scope_complex](../tests/fixtures/css/at_rules/scope_complex_prettier_divergence/)                                     |
| @scope newlines        | Stable quirk   | [scope_selector](../tests/fixtures/css/at_rules/scope_selector_prettier_divergence/)                                   |
| @supports line wrap    | Print width    | [supports_long](../tests/fixtures/css/at_rules/supports_long_prettier_divergence/)                                     |
| SCSS directive numbers | Design choice  | [scss_directive_number_preserved](../tests/fixtures/css/at_rules/scss_directive_number_preserved_prettier_divergence/) |

**Spec violations**: CSS Syntax 3 §4.3.4 specifies that an identifier immediately followed by `(` tokenizes as a `<function-token>`, not as an `<ident-token>` plus `(`. Media Queries 4 §3 explicitly notes: "Whitespace is required between a 'not', 'and', or 'or' keyword and the following '(' character, because without it that would instead parse as a `<function-token>`." Container Queries (CSS Conditional 5) use the same grammar pattern. Prettier normalizes this for `@supports` but not `@media` or `@container`.

**SCSS directive numbers**: SCSS/Sass directives (`@include`, `@mixin`, `@if`, `@for`, `@each`, `@while`, `@function`, `@return`, `@debug`) are not standard CSS. Per CSS Syntax 3 §5.4.2 an unknown at-rule's prelude is consumed as an opaque list of component values with no defined grammar, so tsv preserves it verbatim (e.g. `@include foo(.5s)` stays `.5s`, `@include baz(1.50)` stays `1.50`). Prettier keeps a hardcoded SCSS-directive list whose params it re-parses as a value AST and number-normalizes (`.5s`→`0.5s`, `1.50`→`1.5`). tsv applies number normalization only to contexts whose grammar it parses — declaration values and `@media`/`@supports` preludes (see [CSS: Values](#css-values) "Number dot-ident"); unrecognized directive preludes stay raw. Both outputs are valid CSS (`.5` and `0.5` are the same `<number>` token); the divergence is one of scope, not correctness.

**Media comma-list wrapping (not a divergence)**: a comma-separated `@media` query list (Media Queries 4 §"media query list") that exceeds print width is broken at every top-level comma — one query per line, one indent level — matching Prettier exactly. The `@media line wrap` divergence below applies only to a _single_ `and`-joined query (no comma), which Prettier never wraps but tsv does.

### CSS: Selectors

| Feature                  | Reason        | Fixture                                                                                  |
| ------------------------ | ------------- | ---------------------------------------------------------------------------------------- |
| Column combinator `\|\|` | Parser compat | [column](../tests/fixtures/css/selectors/combinators/column_prettier_divergence/)        |
| :nth-child() An+B        | Stable quirk  | [nth_child](../tests/fixtures/css/selectors/pseudo_class/nth_child_prettier_divergence/) |

### CSS: Values

| Feature                    | Reason         | Fixture                                                                                                                    |
| -------------------------- | -------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Ratio in media queries     | Stable quirk   | [ratio](../tests/fixtures/css/values/ratio/ratio_prettier_divergence/)                                                     |
| Transform list wrap        | Print width    | [transform_long](../tests/fixtures/css/values/functions/transform_long_prettier_divergence/)                               |
| Space-separated value wrap | Print width    | [space_separated_long_wrap](../tests/fixtures/css/values/lists/space_separated_long_wrap_prettier_divergence/)             |
| Comma+space value boundary | Print width    | [comma_space_separated_long](../tests/fixtures/css/values/lists/comma_space_separated_long_prettier_divergence/)           |
| Number dot-ident           | Spec violation | [number_dot_ident](../tests/fixtures/css/values/numbers/number_dot_ident_prettier_divergence/)                             |
| Block-valued custom prop   | Design choice  | [block_value](../tests/fixtures/css/values/variables/block_value_svelte_prettier_divergence/)                              |
| Empty custom-prop value    | Stable quirk   | [empty_value](../tests/fixtures/css/values/variables/empty_value_prettier_divergence/)                                     |
| Empty value + `!important` | Prettier bug   | [empty_value_important](../tests/fixtures/css/values/variables/empty_value_important_prettier_divergence/)                 |
| var() value-less fallback  | Prettier bug   | [var_empty_fallback_degenerate](../tests/fixtures/css/values/variables/var_empty_fallback_degenerate_prettier_divergence/) |

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

**Stable quirk.** Prettier has stable variants for comment positioning. tsv normalizes consistently.

- At-rule before `{` — [atrule_before_opening_brace](../tests/fixtures/css/tokens/comments/atrule_before_opening_brace_prettier_divergence/)
- At-rule in prelude — [atrule_in_prelude](../tests/fixtures/css/tokens/comments/atrule_in_prelude_prettier_divergence/)
- After colon in values — [in_property_value_after_colon](../tests/fixtures/css/tokens/comments/in_property_value_after_colon_prettier_divergence/)
- Before colon in values — [in_property_value_before_colon](../tests/fixtures/css/tokens/comments/in_property_value_before_colon_prettier_divergence/)
- @media prelude — [media_list](../tests/fixtures/css/tokens/comments/media_list_prettier_divergence/)
- @media long with `/* */` — [media_long](../tests/fixtures/css/tokens/comments/media_long_prettier_divergence/)
- Selector before `{` — [selector_before_opening_brace](../tests/fixtures/css/tokens/comments/selector_before_opening_brace_prettier_divergence/)
- Selector before `{` (≥2) — [selector_before_opening_brace_multiple](../tests/fixtures/css/tokens/comments/selector_before_opening_brace_multiple_prettier_divergence/)
- Selector list — [selector_list](../tests/fixtures/css/tokens/comments/selector_list_prettier_divergence/)

### Whitespace: BOM Handling

**BOM stripping.** Prettier preserves byte-order marks. tsv strips them (they serve no purpose in UTF-8).

- Svelte — [bom](../tests/fixtures/svelte/syntax/whitespace/bom_prettier_divergence/)
- CSS — [bom](../tests/fixtures/css/tokens/whitespace/bom_prettier_divergence/)
- TypeScript — [bom](../tests/fixtures/typescript/syntax/whitespace/bom_prettier_divergence/)

### Svelte/HTML

| Feature                   | Reason               | Fixture                                                                                                                       |
| ------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Menu block element        | Spec violation       | [menu_block](../tests/fixtures/svelte/elements/menu_block_prettier_divergence/)                                               |
| Self-closing non-void     | Design choice        | [self_closing_nonvoid](../tests/fixtures/svelte/elements/self_closing_nonvoid_prettier_divergence/)                           |
| Fill after inline         | Print width          | [fill_after_inline](../tests/fixtures/svelte/elements/fill_after_inline_prettier_divergence/)                                 |
| Fill boundary             | Print width          | [inline_element_fill_long](../tests/fixtures/svelte/elements/inline_element_fill_long_prettier_divergence/)                   |
| Fill after breaking attr  | Print width          | [multiline_value_inline_long](../tests/fixtures/svelte/attributes/multiline_value_inline_long_prettier_divergence/)           |
| Component fill boundary   | Print width          | [inline_component_fill_long](../tests/fixtures/svelte/elements/inline_component_fill_long_prettier_divergence/)               |
| Fill multiple expr        | Print width          | [fill_multiple_expr_long](../tests/fixtures/svelte/elements/fill_multiple_expr_long_prettier_divergence/)                     |
| Inline content hug        | Design choice        | [inline_content_hug_long](../tests/fixtures/svelte/elements/inline_content_hug_long_prettier_divergence/)                     |
| Block multiline attrs hug | Print width          | [block_multiline_attrs_content_hug](../tests/fixtures/svelte/elements/block_multiline_attrs_content_hug_prettier_divergence/) |
| Fill expr break boundary  | Print width          | [fill_expr_break_boundary_long](../tests/fixtures/svelte/elements/fill_expr_break_boundary_long_prettier_divergence/)         |
| @debug comments           | Content preservation | [debug_comment](../tests/fixtures/svelte/tags/debug/debug_comment_prettier_divergence/)                                       |
| svelte:element `this`     | Prettier bug         | [svelte_element_this_string](../tests/fixtures/svelte/special_elements/svelte_element_this_string_prettier_divergence/)       |
| svelte:element class ws   | Prettier bug         | [svelte_element_class_whitespace](../tests/fixtures/svelte/special_elements/svelte_element_class_whitespace_prettier_divergence/) |

**Fill after inline**: Prettier's fill algorithm allows lines to exceed print width when text follows an inline element closing tag. Prettier produces 111 char lines, tsv breaks at exactly 100 chars.

**Fill boundary**: When fill content exceeds print width, Prettier tolerates the overage while tsv breaks earlier. This is an emergent behavior from prettier-plugin-svelte's doc structure (separate fills per text node with `group([line, element])` wrappers). See also `inline_element_fill_100/` which shows both formatters match at exactly 100 chars.

**Fill after breaking attr**: When an inline element has breaking attributes (e.g., multiline values) and long trailing text follows, Prettier's `handleTextChild` early return for last-child text skips wrapping the previous element, so there's no break point between the closing tag and trailing text. Prettier allows the line to exceed print width (102 chars). tsv's fill correctly breaks trailing words at the print width boundary (100 chars). See also `multiline_value_inline/` which shows both formatters match when trailing text is short.

**Component fill boundary**: Same fill boundary behavior as above but with component elements (`<Comp>text</Comp>`) instead of HTML inline elements. At 101 chars, Prettier keeps everything on one line while tsv breaks the closing `>` of the closing tag to stay within print width. At 100 chars both formatters match.

**Fill expr break boundary**: When fill content includes a multiline expression (e.g., binary `+` that breaks across lines), subsequent text continues on the continuation line. At the width boundary, Prettier allows the continuation line to reach 101 chars while tsv breaks at 96 to stay under 100. See also `fill_expr_break_continuation_long/` for matching behavior when continuation stays under 100.

**Fill multiple expr**: When fill mode content has multiple expression tags with ternaries, Prettier breaks the opening bracket and packs more onto a single line (101 chars), breaking within the comparison operator (`!==`). tsv keeps the opening bracket hugging content and breaks earlier at the first ternary operator (`?`) to stay within 100 chars (max 79).

**Inline content hug**: When inline element content with _breakable_ expressions (ternaries, `&&`/`||`, `+`/`-`) exceeds print width, Prettier breaks the opening bracket while tsv prefers breaking expressions internally. The benefit is reduced indentation drift (1 less tab level per nesting depth). When expressions are simple (identifiers, property access) and can't break internally, tsv matches Prettier's behavior by breaking the opening bracket instead.

**Menu block element**: prettier-plugin-svelte's `blockElements` list includes `ol` and `ul` but omits `menu`. The HTML spec treats `<menu>` identically to `<ul>` — same `display: block`, same CSS rules (`padding-inline-start: 40px`, `counter-reset: list-item`). The spec explicitly says: _"The `menu` element is simply a semantic alternative to `ul`."_ tsv includes `<menu>` in the block element list (spec-compliant), causing block formatting (expanded content) where Prettier hugs content. This only manifests when compact input is formatted — both formatters preserve the block form if given it directly.

**Block multiline attrs hug**: When whitespace-sensitive elements (`<pre>`, `<textarea>`) have multiline attributes and hugged content that would exceed print width, Prettier keeps `>{content}</tag>` on the attribute line (101+ chars). tsv breaks `>` to its own line (`\n>{content}`) to respect print width while preserving whitespace semantics (no text node added since `>` immediately precedes content).

**svelte:element `this`**: prettier-plugin-svelte 4.x ignores `singleQuote` for a brace-wrapped string literal in `<svelte:element this={…}>`, always emitting double quotes (`this={'hello'}` → `this={"hello"}`), and skips escaping entirely — so `this={'a"b'}` becomes the invalid `this={"a"b"}` and `this={'a\b'}` corrupts to a backspace. tsv delegates the literal to the normal string printer, honoring `singleQuote` and escaping like any other string. The bug is narrow — only a *directly* brace-wrapped string literal triggers it; concatenations, template literals, and other expressions delegate to the JS printer and agree (boundary fixture [svelte_element_this](../tests/fixtures/svelte/special_elements/svelte_element_this/)). One adjacent facet diverges further: a *parenthesized* literal `this={('hello')}` collapses to the plain attribute `this="hello"` under prettier (structural rewrite), while tsv keeps `this={'hello'}` — encoded by the fixture's `unformatted_ours_paren` + `variant_paren_collapse` pair. A fix restoring JS-printer delegation has been prepared for prettier-plugin-svelte; once released and re-pinned, this divergence retires.

**svelte:element class ws**: prettier-plugin-svelte 4.x no longer collapses repeated whitespace in a `class` attribute on `<svelte:element>` (`class="a   b    c"` stays verbatim), while it still collapses on a plain element (`<div class="a   b    c">` → `class="a b c"`) — and so does tsv, everywhere. The `<svelte:element>` attribute path regressed off the normal attribute printer in the same 4.x modern-ast migration as the `this` bug (3.5.2 collapsed here too). tsv collapses uniformly across all elements. Both formatters keep the single-spaced form stable; the divergence shows only when collapsing multi-space input. Retires once a plugin fix releases and tsv's oracle is re-pinned.

### Svelte: Attributes

**Trailing comments in `{...}`** (content preservation) — [expr_trailing](../tests/fixtures/svelte/syntax/comments/expr_trailing_prettier_divergence/) (block comments, inline); [expr_trailing_line](../tests/fixtures/svelte/syntax/comments/expr_trailing_line_prettier_divergence/) (line comments — `}` kept on its own line so the `//` doesn't swallow it).

**Same-line `//` comment placement in the attribute list** (comment preservation) — [comment_same_line](../tests/fixtures/svelte/attributes/comment_same_line_prettier_divergence/): a line comment the author put on the same line as the tag name (`<div // foo`) or trailing an attribute that has more attributes after it (`a="1" // mid`) stays trailing that token; prettier relocates it to its own line. A `//` trailing the *last* attribute (before `>`/`/>`) already stays inline in both formatters, so it is not a divergence — [comment_trailing_same_line](../tests/fixtures/svelte/attributes/comment_trailing_same_line/). Block comments and own-line comments are preserved as-written by both. See [Comment Position Philosophy](#comment-position-philosophy).

### Svelte: Blocks

- `{#each}` line wrap — [each_long](../tests/fixtures/svelte/blocks/each/long_prettier_divergence/)
- `{#await}` line wrap — [await_long](../tests/fixtures/svelte/blocks/await/long_prettier_divergence/)
- `{#key}` line wrap — [key_long](../tests/fixtures/svelte/blocks/key/long_prettier_divergence/)
- `{#if}` logical wrap — [if_long](../tests/fixtures/svelte/blocks/if/long_prettier_divergence/)
- `{#if}` last block quirk — [last_block](../tests/fixtures/svelte/blocks/if/last_block_prettier_divergence/)
- `{#if}` short expr 100+ — [in_inline_element_long](../tests/fixtures/svelte/blocks/if/in_inline_element_long_prettier_divergence/)

**Print width.** Prettier doesn't apply width-based wrapping to block expressions:

- **Method chains** in `{#each}`, `{#await}`, `{#if}`, `{#key}`, etc. don't account for the tag prefix width, resulting in 140+ char lines. tsv passes context offset for proper wrapping.
- **Logical expressions** (`&&`, `||`) in block conditions are never wrapped internally by Prettier, even when exceeding 100 chars. (In `<script>`, assignments provide a break point so this isn't an issue.) tsv wraps them with proper indentation.
- **Function calls** in block expressions don't wrap their arguments. tsv wraps function arguments when they exceed print width.

**Last block not expanded**: Prettier expands `{#if a} content {/if}` (symmetric spaces) to multiline, but has a quirk: the last block in a file stays inline. A single block appears preserved only because it's last. tsv expands consistently regardless of position.

**Short expr 100+**: Prettier tolerates exceeding print width (100+ chars) for short comparison expressions in block conditions like `{#if typeof x === 'string'}`. tsv breaks to respect print width strictly. This affects expressions that are just slightly over the limit (e.g., 103 chars).

### TypeScript

| Feature                                   | Reason                | Fixture                                                                                                                                          |
| ----------------------------------------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Empty statement blank lines               | Design choice         | [empty_standalone](../tests/fixtures/typescript/statements/empty_standalone_prettier_divergence/)                                                |
| Return type generic union                 | Print width           | [return_type_generic_union_long](../tests/fixtures/typescript/declarations/function/return_type_generic_union_long_prettier_divergence/)         |
| Single specifier import                   | Print width           | [single_specifier_long](../tests/fixtures/typescript/modules/imports/single_specifier_long_prettier_divergence/)                                 |
| Module path calls                         | Print width           | [path_calls_long](../tests/fixtures/typescript/modules/imports/path_calls_long_prettier_divergence/)                                             |
| Instantiation expression parens           | Semantic preservation | [instantiation_parens](../tests/fixtures/typescript/typescript_specific/assertions/instantiation_parens_prettier_divergence/)                    |
| Optional-chain base member chain          | Semantic preservation | [optional_paren_member_chain](../tests/fixtures/typescript/expressions/chain/optional_paren_member_chain_prettier_divergence/)                   |
| Optional-chain non-null base member chain | Semantic preservation | [optional_paren_non_null_member_chain](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_member_chain_prettier_divergence/) |
| Optional-chain non-null new callee        | Prettier bug          | [optional_paren_non_null_new_callee](../tests/fixtures/typescript/expressions/chain/optional_paren_non_null_new_callee_prettier_divergence/)     |
| Non-null parenthesized base               | Design choice         | [non_null_paren_base_long](../tests/fixtures/typescript/expressions/member/non_null_paren_base_long_prettier_divergence/)                        |
| Constrained infer extends-operand parens  | Prettier bug          | [constrained_extends_parens](../tests/fixtures/typescript/types/infer/constrained_extends_parens_prettier_divergence/)                           |
| Arrow type param trailing comma           | Design choice         | [single_type_param](../tests/fixtures/typescript/expressions/arrow/generic/single_type_param_prettier_divergence/)                              |

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

**Arrow type param trailing comma**: For a generic arrow with a **single type param that has no constraint** (`<T>`, default-only `<T = string>`, or `const`-modified `<const T>`), Prettier forces a trailing comma — `<T,>` — via `shouldForceTrailingComma` (`language-js/print/type-parameters.js`). It does so to keep the output valid as TSX, where a bare `<T>` is ambiguous with a JSX element; the guard fires whenever the file is not known to end in `.ts`, which is always the case for a Svelte `<script>` body (prettier-plugin-svelte hands it to prettier without a `.ts` filepath). tsv has no JSX — it never emits TSX, and Svelte's own parser accepts bare `<T>` in every TS position (`<script>`, template `{...}`, `{@const}`) — so the disambiguation is vestigial and tsv emits the bare canonical form. Multi-param (`<T, U>`), constrained (`<T extends X>`), and empty (`<>`) type params are unaffected; prettier never forces the comma for those and tsv matches. The accepted tradeoff: in a mixed-tool repo prettier rewrites `<T>` back to `<T,>`, so the two ping-pong on this construct (reviewed and accepted — bare `<T>` is correct for a non-JSX formatter). Fixtures: [single_type_param](../tests/fixtures/typescript/expressions/arrow/generic/single_type_param_prettier_divergence/), [const_type_param_arrow](../tests/fixtures/typescript/typescript_specific/generics/const_type_param_arrow_prettier_divergence/), and — stacked with the acorn-typescript async param-drop parser bug — [async_generic/basic](../tests/fixtures/typescript/expressions/arrow/async_generic/basic_svelte_prettier_divergence/) and [curried_typed_callback](../tests/fixtures/typescript/expressions/arrow/curried_typed_callback_svelte_prettier_divergence/). The comment-relocation fixture [arrow_type_params_paren_comment](../tests/fixtures/typescript/declarations/function/arrow_type_params_paren_comment_prettier_divergence/) also exercises it.

### Prettier rejects valid input

These inputs are **valid** by tsv's parse oracle (Svelte / acorn-typescript) and our formatter keeps them stable, but prettier's `typescript` parser/printer **throws** on them — so there is no `output_prettier.*` oracle. Each fixture carries a `prettier_rejects.txt` marker pinning the exact error; rule F6 live-verifies that prettier still rejects the input (failing loudly if the bug is fixed upstream or the error morphs). All three reproduce in plain prettier (`parser: 'typescript'`, zero Svelte) and are fine under `babel-ts`; the 4.x prettier-plugin-svelte bump surfaced them because the plugin switched `lang="ts"` formatting from `babel-ts` to the real `typescript` parser.

| Construct                                                     | Prettier error                                              | Fixture                                                                                                                                                       |
| ------------------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Optional chain to private field (`x?.#a`)                     | `An optional chain cannot contain private identifiers.`     | [private_fields_optional_chain](../tests/fixtures/typescript/declarations/class/private_fields_optional_chain_prettier_divergence/)                           |
| Parenthesized optional-chain decorator callee (`@((a?.b)())`) | `Cannot read properties of undefined (reading 'type')`      | [parenthesized_optional_chain](../tests/fixtures/typescript/typescript_specific/decorators/parenthesized_optional_chain_prettier_divergence/)                 |
| Line comment before import-attributes `with`                  | `'(' expected.`                                             | [with_keyword_comment_line](../tests/fixtures/typescript/modules/imports/with_keyword_comment_line_prettier_divergence/)                                      |

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
- Switch empty body → Discriminant parens — [empty_comment](../tests/fixtures/typescript/statements/switch/empty_comment_prettier_divergence/)
- Switch case before `{` → After opening brace — [case_block_comment](../tests/fixtures/typescript/statements/switch/case_block_comment_prettier_divergence/)
- Switch discriminant trailing → Switch body — [discriminant_trailing_comment](../tests/fixtures/typescript/statements/switch/discriminant_trailing_comment_prettier_divergence/)
- For after `)` → Inline with update clause — [trailing_comment](../tests/fixtures/typescript/statements/for/trailing_comment_prettier_divergence/)
- For empty clauses → Outside parentheses (broken) — [empty_clauses_comment](../tests/fixtures/typescript/statements/for/empty_clauses_comment_prettier_divergence/)
- For non-empty header after `)` (line) → First comment relocated into the parens (trailing the last clause) — [header_body_comment](../tests/fixtures/typescript/statements/for/header_body_comment_prettier_divergence/)
- For-of loop header → Outside loop header — [of_line_comment](../tests/fixtures/typescript/statements/for/of_line_comment_prettier_divergence/)
- For-in/of own-line comment → Before statement or after `)` — [in_of_own_line_comment](../tests/fixtures/typescript/statements/for/in_of_own_line_comment_prettier_divergence/)
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
- Block comment in computed `[]` → Before member chain (hoisted) — [block_comment_computed_member_long](../tests/fixtures/typescript/syntax/comments/block_comment_computed_member_long_prettier_divergence/)
- Switch case colon comment → Before colon or into body — [case_colon_comment](../tests/fixtures/typescript/statements/switch/case_colon_comment_prettier_divergence/)
- Class property definite `!` → Before `!` modifier — [property_definite_comment](../tests/fixtures/typescript/statements/class/property_definite_comment_prettier_divergence/)
- Class property modifier → Before `?`/`!` modifier — [property_modifier_comment](../tests/fixtures/typescript/statements/class/property_modifier_comment_prettier_divergence/)
- Between member modifiers → After the last modifier — [modifier_pair_comment](../tests/fixtures/typescript/declarations/class/modifier_pair_comment_prettier_divergence/)
- Interface member after `?` → Before `?` or inside parens — [modifier_after_comment](../tests/fixtures/typescript/types/type_members/modifier_after_comment_prettier_divergence/)
- Type-literal member after `?` → Before `?` or inside parens — [optional_marker_comment](../tests/fixtures/typescript/types/type_literal/optional_marker_comment_prettier_divergence/)
- Class method after `?` → Before `?` modifier — [optional_marker_comment](../tests/fixtures/typescript/declarations/class/optional_marker_comment_prettier_divergence/)
- Optional `?` to `:` line comment (all contexts) → Trailing the member `;` — [optional_marker_line_comment](../tests/fixtures/typescript/syntax/comments/optional_marker_line_comment_prettier_divergence/)
- Member key to `:` line comment (non-optional) → Trailing the member `;` — [key_colon_line_comment](../tests/fixtures/typescript/syntax/comments/key_colon_line_comment_prettier_divergence/)
- Variable definite `!` → After `!` modifier — [definite_comment](../tests/fixtures/typescript/declarations/variable/definite_comment_prettier_divergence/)
- Function param optional `?` → After `?` modifier — [param_optional_comment](../tests/fixtures/typescript/declarations/function/param_optional_comment_prettier_divergence/)
- Computed key after `]` (object) → Inside brackets `[x /* c */]` — [computed_key_bracket_colon_comment](../tests/fixtures/typescript/expressions/objects/computed_key_bracket_colon_comment_prettier_divergence/)
- Computed key after `]` (class) → Inside brackets `[x /* c */]` — [computed_key_bracket_comment](../tests/fixtures/typescript/statements/class/computed_key_bracket_comment_prettier_divergence/)
- Computed key after `]` (iface) → Inside brackets (set: into params) — [computed_key_bracket_comment](../tests/fixtures/typescript/types/type_members/computed_key_bracket_comment_prettier_divergence/), [paren_in_comment](../tests/fixtures/typescript/types/type_members/computed_key_bracket_paren_in_comment_prettier_divergence/)
- Computed key after `]` (destr) → Inside brackets `[x /* c */]` — [computed_key_bracket_comment](../tests/fixtures/typescript/expressions/destructuring/computed_key_bracket_comment_prettier_divergence/)
- `readonly` keyword to `[` (index sig) → Inside brackets before the key `[/* c */ k` — [index_signature_readonly_comment](../tests/fixtures/typescript/types/type_members/index_signature_readonly_comment_prettier_divergence/)
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
- Name to type params (line) → End of declaration line — [name_type_params_line_comment](../tests/fixtures/typescript/declarations/class/name_type_params_line_comment_prettier_divergence/)
- Method name to type params (line) → End of method line — [method_name_type_params_line_comment](../tests/fixtures/typescript/declarations/class/method_name_type_params_line_comment_prettier_divergence/)
- Heritage last item before `{` → Into class/interface body — [heritage_last_item_line_comment](../tests/fixtures/typescript/declarations/class/heritage/heritage_last_item_line_comment_prettier_divergence/)
- Arrow body stripped parens → Into arrow params or trailing — [body_paren_comment](../tests/fixtures/typescript/expressions/arrows/body_paren_comment_prettier_divergence/)
- Sequence last-operand trailing edge (stmt) → After the `;` (before it, in tsv) — [operand_edge_comment_stmt](../tests/fixtures/typescript/expressions/sequence/operand_edge_comment_stmt_prettier_divergence/)
- Between keyword and `(` → Inside parens — [keyword_paren_comment](../tests/fixtures/typescript/syntax/comments/keyword_paren_comment_prettier_divergence/)
- `for await` keyword gaps → Inside parens — [for_await_keyword_comment](../tests/fixtures/typescript/statements/for/for_await_keyword_comment_prettier_divergence/)
- Between `)` and `{` (switch) → Inside condition parens — [condition_absorbed_comment](../tests/fixtures/typescript/syntax/comments/condition_absorbed_comment_prettier_divergence/)
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
- Function param default to value (line) → After the value + its comma — [param_default_line_comment](../tests/fixtures/typescript/declarations/function/param_default_line_comment_prettier_divergence/)
- `as`/`satisfies` keyword to type (line) → Statement-trailing (after `;`) — [as_satisfies_value_line_comment](../tests/fixtures/typescript/expressions/as_satisfies_value_line_comment_prettier_divergence/)
- Heritage keyword to type (line) → Up before the keyword — [extends_keyword_line_comment](../tests/fixtures/typescript/class/extends_keyword_line_comment_prettier_divergence/)
- Conditional `extends` to check (line) → Trailing the extends-type — [check_extends_line_comment](../tests/fixtures/typescript/types/conditional/check_extends_line_comment_prettier_divergence/)
- Mapped `:` to value (line) → Trailing the member `;` — [mapped_value_line_comment](../tests/fixtures/typescript/types/mapped_value_line_comment_prettier_divergence/)
- Type predicate `is` to type (line) → Trailing the body `{` — [predicate_is_line_comment](../tests/fixtures/typescript/types/predicate_is_line_comment_prettier_divergence/)
- After last list-member comma → Before the comma — [objects](../tests/fixtures/typescript/expressions/objects/trailing_comma_comment_prettier_divergence/), [patterns](../tests/fixtures/typescript/expressions/patterns/trailing_comma_comment_prettier_divergence/), [arrays](../tests/fixtures/typescript/expressions/arrays/trailing_comma_comment_prettier_divergence/), [destructuring](../tests/fixtures/typescript/expressions/destructuring/trailing_comma_comment_prettier_divergence/), [calls](../tests/fixtures/typescript/expressions/calls/trailing_comma_comment_prettier_divergence/), [imports](../tests/fixtures/typescript/modules/imports/trailing_comma_comment_prettier_divergence/), [exports](../tests/fixtures/typescript/modules/exports/trailing_comma_comment_prettier_divergence/), [params](../tests/fixtures/typescript/declarations/function/trailing_comma_comment_prettier_divergence/), [function_type](../tests/fixtures/typescript/types/function_type/trailing_comma_comment_prettier_divergence/), [tuple](../tests/fixtures/typescript/types/tuple/trailing_comma_comment_prettier_divergence/), [type_params](../tests/fixtures/typescript/types/type_params/trailing_comma_comment_prettier_divergence/)
- Call open paren `(` trailing → Onto its own line — [open_paren_comment](../tests/fixtures/typescript/expressions/calls/open_paren_comment_prettier_divergence/), [chain](../tests/fixtures/typescript/expressions/calls/chain_open_paren_comment_prettier_divergence/), [new](../tests/fixtures/typescript/expressions/calls/new_open_paren_comment_prettier_divergence/)
- Object literal `{` trailing → Onto its own line — [open_brace_comment](../tests/fixtures/typescript/expressions/objects/open_brace_comment_prettier_divergence/)
- Array literal `[` trailing → Onto its own line — [open_bracket_comment](../tests/fixtures/typescript/expressions/arrays/open_bracket_comment_prettier_divergence/)
- Block body `{` trailing → Onto its own line — [block_open_brace_comment](../tests/fixtures/typescript/statements/block_open_brace_comment_prettier_divergence/)
- Type-parameter `<` trailing → Onto its own line — [open_angle_comment](../tests/fixtures/typescript/types/type_params/open_angle_comment_prettier_divergence/)
- Function/constructor-type `(` trailing → Onto its own line — [open_paren_comment](../tests/fixtures/typescript/types/function_type/open_paren_comment_prettier_divergence/)
- Fn/ctor-type empty-params `(` trailing → After `)` (out of empty parens) — [empty_param_line_comment](../tests/fixtures/typescript/types/function_type/empty_param_line_comment_prettier_divergence/)
- Fn/ctor-type pre-arrow `)` trailing (params) → Onto the last param (`a: T, // c`) — [pre_arrow_param_line_comment](../tests/fixtures/typescript/types/function_type/pre_arrow_param_line_comment_prettier_divergence/)
- Call/construct signature `(` trailing → Onto its own line (method keeps) — [signature_params_leading_line_comment](../tests/fixtures/typescript/types/comments/signature_params_leading_line_comment_prettier_divergence/)
- Object destructuring `{` trailing → Onto its own line — [object_open_brace_comment](../tests/fixtures/typescript/expressions/destructuring/object_open_brace_comment_prettier_divergence/)
- Array destructuring `[` trailing → Onto its own line — [array_open_bracket_comment](../tests/fixtures/typescript/expressions/destructuring/array_open_bracket_comment_prettier_divergence/)
- Namespace/module body `{` trailing → Onto its own line — [open_brace_comment](../tests/fixtures/typescript/declarations/namespace/open_brace_comment_prettier_divergence/)
- Class/interface/enum body `{` trailing → Onto its own line — [class](../tests/fixtures/typescript/statements/class/open_brace_comment_prettier_divergence/), [interface](../tests/fixtures/typescript/statements/interface/open_brace_comment_prettier_divergence/), [enum](../tests/fixtures/typescript/declarations/enum/open_brace_comment_prettier_divergence/)
- Type literal body `{` trailing → Onto its own line — [type_literal_open_brace_comment](../tests/fixtures/typescript/types/type_literal_open_brace_comment_prettier_divergence/)
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

**Import/export keyword-to-braces comments**: `import /* c */ {} from 'a'` → Prettier relocates comments between the `import`/`export` keyword and empty specifier braces to after `from`: `import {} from /* c */ 'a'`. tsv preserves the comment between keyword and braces. The same holds for an empty type-only import or re-export around the `type` keyword — both the keyword→`type` gap (`import /* c */ type {} from 'a'`, `export /* c */ type {} from 'a'`) and the `type`→`{}` gap (`import type /* c */ {} from 'a'`) are preserved in place while Prettier relocates them after `from`. With **named specifiers** the same two gaps (`import /* c */ type {A} from 'a'`, `import type /* c */ {A} from 'a'`, and the export forms) are likewise preserved in place; here Prettier relocates the comment _into_ the specifier braces as the first specifier's leading comment (`import type {/* c */ A} from 'a'`, a line comment also expanding the braces). Both positions are dual-stable in our formatter. A line comment in any of these gaps — including the no-`from` `export // c⏎{}` — indents the continuation one level (the uniform module-header rule below); for the no-`from` form Prettier keeps the comment in place and flat, so tsv's indent is an indent-only divergence.

**Default / namespace import + export-all header comments**: the same preserve-in-place rule covers the remaining module-header shapes. In a **default** (`import /* c */ Foo from 'a'`) or **namespace** (`import /* c */ * as ns from 'a'`) import, tsv keeps each header comment where the user wrote it; Prettier keeps a comment already adjacent to the binding but relocates a comment between `import` and `type` to the binding side of `type` (`import type /* c */ Foo`). In an **export-all** (`export /* c */ * from 'a'`, including `export /* c */ type * from 'a'`) Prettier relocates _every_ header comment — around `export`, `type`, and `*` — to after `from`, before the source; tsv preserves them in place. A comment between `*` and `as` in a namespace binding (`import * /* c */ as ns`, `export * /* c */ as ns from 'a'`) is likewise preserved in place — Prettier relocates it to after `as`, before the binding. Both positions are dual-stable in our formatter. Per the uniform module-header rule below, a line comment in **every** one of these gaps indents the continuation one level — the keyword→default/namespace binding, `type`→default/namespace binding, `type`→namespace-`*`, export-all `export`/`type`→`*`, `*`→`from`, `*`→`as`, and `as`→binding gaps alike. Where Prettier relocates (export-all, `*`→`as`) tsv is free; where Prettier keeps the comment in place and flat (keyword/`type`→binding, `type`→namespace-`*`, `as`→binding) tsv's indent is a deliberate indent-only divergence.

**Import/export binding-or-specifiers to `from`**: A comment in the gap between an import's binding/specifiers (or a re-export's specifiers) and the `from` keyword is preserved where the user placed it; Prettier's relocation depends on the binding shape. For a **default** or **namespace** binding (no braces) a same-line block comment stays in place in both formatters (`import Foo /* c */ from 'a'` — dual-stable), but Prettier floats a line comment past the `;` to a statement-trailing position (`import Foo from 'a'; // c`, the before-semicolon/float-out rule) while tsv keeps it before `from`, indenting the `from …` continuation one level onto its own line. For **named specifiers** (`import {a} /* c */ from 'a'`, `export {a} /* c */ from 'a'`) Prettier relocates the comment _into_ the braces as the last specifier's trailing comment — a block comment inline (`{a /* c */}`), a line comment expanding the braces multiline (`{\n\ta, // c\n}`) — while tsv keeps it after `}` with the braces inline (indenting `from …` when a line comment forces the break). Both positions are dual-stable in our formatter. (Otherwise the comments are dropped entirely — content loss.)

**Import attributes header comments**: A comment in an import's attributes header — between the source and the `with` keyword (`import x from 'a' /* c */ with {…}`), or between `with` and the attributes `{` (`import x from 'a' with /* c */ {…}`) — is preserved where the user placed it. Prettier keeps a source→`with` block comment in place (dual-stable), relocates a `with`→`{` block comment to before `with` (`import x from 'a' /* c */ with {…}`), and floats a `with`→`{` line comment past the `;` (`import x from 'a' with {…}; // c`, the before-semicolon/float-out rule); a source→`with` _line_ comment instead makes Prettier's `typescript` parser **throw** (`'(' expected.`), so that form has no oracle — see [Prettier rejects valid input](#prettier-rejects-valid-input) and [with_keyword_comment_line](../tests/fixtures/typescript/modules/imports/with_keyword_comment_line_prettier_divergence/). tsv keeps each comment where the user wrote it; when a line comment forces the `with`/`{…}` onto a new line, tsv indents that continuation one level. The attributes `}`→`;` comment is covered by the before-semicolon rule above. (Otherwise the header comments are dropped entirely — content loss.)

**Keyword → specifier brace comments**: A comment between the `import`/`export` keyword and the named-specifier `{` (`import /* c */ {a}`, `export /* c */ {a}`) is preserved before the brace; Prettier relocates it _into_ the braces as the first specifier's leading comment — a block comment inline (`import {/* c */ a}`), a line comment expanding the braces multiline. tsv keeps it where the user wrote it; a line comment forces `{` onto the next line, indenting the `{…}` continuation one level (the uniform module-header rule below). (The `import type … {a}` type→`{` gap and the empty-braces `import /* c */ {}` gap were already preserved; this is the non-type named-specifier case, which previously **dropped** the comment — content loss now fixed.) See [imports/keyword_brace_comment](../tests/fixtures/typescript/modules/imports/keyword_brace_comment_prettier_divergence/) and [exports/keyword_brace_comment](../tests/fixtures/typescript/modules/exports/keyword_brace_comment_prettier_divergence/).

**Declaration- and module-header line-comment continuation indent**: A _line_ comment in a declaration- or module-header gap forces the following token onto a new line, and tsv indents that continuation one level — a statement spanning lines reads as a continuation, not a second statement.

For **module headers** tsv applies this **uniformly to every gap, with no exceptions**: keyword→`type`, keyword/`type`→default-binding, keyword/`type`→namespace-`*`, keyword/`type`→`{`, keyword/`type`→empty-`{}` (re-export _and_ no-`from`), bare keyword→source, export-all `export`/`type`→`*`, `*`→`from`, `*`→`as`, `as`→binding, binding/specifiers→`from`, `from`→source, the `with`/attributes-`{` gaps, and `export`/`export default`→declaration. Prettier's own handling varies per gap — it relocates the comment (into the braces, after `from`/`as`/`;`), floats it past `;`, or keeps it in place and flat — but tsv **always** indents. So where Prettier relocates, tsv's in-place indent is just one of two unrelated layouts; and where Prettier keeps the comment in place and flat (`as`→binding, keyword/`type`→default/namespace binding, `type`→namespace-`*`, bare/empty/export-all `from`→source, no-`from` empty `{}`, `export`→declaration) tsv's indent is a deliberate indent-only divergence, chosen so every continuation reads alike.

For **declaration headers** the same rule covers the keyword→name gap of `function` (incl. `async function`, `function*`, and `declare function`), `class` (incl. `abstract class`), and `enum` (incl. `const enum`), plus the keyword→declarator gap of `const`/`let`/`var`. Prettier keeps the comment in place and flat for the `function`/`class`/`enum` headers, so tsv's indent is an indent-only divergence there; for `const`/`let`/`var` Prettier **agrees** (it also indents the declarator), so there is no divergence — a regular fixture.

The deliberate exclusions are constructs where the following token isn't part of the same declaration: **ASI statement keywords** (`return`, `throw`, `break`, `continue`, `yield`) — a newline after them triggers automatic semicolon insertion, so `return // c⏎expr` is two statements, not a continuation; the **contextual declaration keywords** (`type`/`interface`/`namespace`/`module`/`declare`), whose line forms never form a single declaration — the keyword requires its following token on the **same line** (tsc's `isDeclaration`: `nextTokenIsIdentifierOnSameLine`, and the ASI modifier rule for `declare`), so a line break demotes the keyword to a plain identifier. `interface`/`namespace`/`module`→name then become unparseable (`interface // c⏎I {}` is rejected); `type`→name and `declare`→keyword **ASI-split** into two statements (`type;` then `T = …`; `declare;` then `const x = …`). tsv's parser conforms here (it previously over-accepted these line forms) — see [contextual_keywords/declaration_keyword_own_line](../tests/fixtures/typescript/syntax/contextual_keywords/declaration_keyword_own_line/); and **function/class expressions** (`const a = function // c⏎f() {}`), which are not declaration headers and stay flat in both formatters. Block comments and the no-comment case are byte-identical in every in-scope gap. (The `export`-prefixed forms — `export interface // c⏎I` etc. — are out of scope: after `export` the parser is committed to a declaration, where tsc does not apply the same-line gate, so tsv's acceptance there may be spec-correct; acorn-typescript rejects them, but that is a separate acorn-vs-tsc question.)

**Keyword-paren comments**: `if/* c */(a)` → Prettier absorbs comments between keywords and `(` into the condition parens. Applies to `if`, `while`, `for`, `switch`, `catch`, `do...while`. tsv preserves the comment between keyword and paren: `if /* c */ (a)`. For `for await`, comments in either gap are preserved: `for /* a */ await /* b */ (x of y)` → prettier absorbs both into parens. The only divergence is the comment position — the loop body layout still matches Prettier regardless: an empty body attaches as `for /* k */ (a; b; c);` (no space before `;`), a block body hugs `) {`, and a non-block body stays inline when the header fits ([keyword_comment](../tests/fixtures/typescript/statements/for/keyword_comment_prettier_divergence/)). Both positions are dual-stable in our formatter.

**Condition-absorbed comments**: `switch(x)/* c */{}` → Prettier absorbs comments between `)` and `{` into the condition parens: `switch (x /* c */) {}`. Similarly, `catch/* c */(e)` → `catch (/* c */ e)`. tsv preserves position: `switch (x) /* c */ {}`, `catch /* c */ (e)`. Both positions are dual-stable in our formatter.

**Before-semicolon comments**: `const x = 1 /* c */;` → Prettier moves comments from before `;` to after: `const x = 1; /* c */`. tsv preserves the user's position. Both positions are dual-stable in our formatter.

**Type alias head to `=`**: `type A<X>\n// c\n= B | C` → Prettier relocates a line comment between a type alias head (name + optional type parameters) and `=` to after `=` (`type A<X> =\n// c\nB | C`). tsv preserves it before `=`, keeping the comment's association with the declaration head rather than the value. (With type parameters present, the comment is otherwise easily dropped entirely — content loss.) A single-line block comment before `=` stays inline in both formatters (`type A<X> /* c */ = B | C`), so it is not a divergence — only a line comment forces the value to the next line, and the two formatters disagree on which side of `=` it lands. Both positions are dual-stable in our formatter.

**Type parameter keyword to value (own-line)**: an own-line line comment between a type parameter's `extends`/`=` keyword and its constraint/default value (`U =\n// c\nV`, `T extends\n// c\nA`) is kept on its own line in the indented value block; Prettier pulls the first leading comment up onto the keyword line (`U = // c\n\tV`) and is **non-idempotent** doing so (its first pass leaves the value at the param indent, a second pass adds the extra indent). tsv stays idempotent and preserves the author's own-line placement. A comment that is **already** on the keyword line (`R extends A | B | void = // c\n…`) is dual-stable: tsv emits it inline via `line_suffix` (zero width), so a long trailing comment never forces a preceding constraint union to break — matching prettier. Only an own-line first comment diverges; the same-line cases are a regular fixture ([type_param_keyword_line_comment](../tests/fixtures/typescript/types/comments/type_param_keyword_line_comment/)). (Emitting the same-line comment as plain text would force-break the constraint union by its width — content added — and merge two line comments onto one line — boundary loss; the `line_suffix` rendering avoids both.) See [type_param_keyword_own_line_comment_prettier_divergence](../tests/fixtures/typescript/types/comments/type_param_keyword_own_line_comment_prettier_divergence/).

**Function parameter default to value**: a line comment after a parameter's `=` default, before the value (`function fn(p = // c\n\tv) {}`), is kept after `=` with the value on the next line; Prettier floats it out to trail the whole parameter, after the value and its comma (`function fn(\n\tp = v, // c\n) {}`). tsv keeps the comment associated with the default rather than floating it past the value. The parameter's type-annotation union stays inline in both (the trailing comment is zero-width). Applies to function, arrow, and method parameters; a same-line block comment (`p = /* c */ v`) stays inline in both formatters and is not a divergence. Both forms are dual-stable in our formatter. See [param_default_line_comment_prettier_divergence](../tests/fixtures/typescript/declarations/function/param_default_line_comment_prettier_divergence/).

**`as`/`satisfies` cast keyword to type**: a line comment after the cast keyword, before the type (`x as // c\n\tA`), is kept after the keyword with the type on the next line; Prettier floats it out past the whole expression to a statement-trailing position (`x as A; // c`). tsv keeps the comment associated with the cast, on the keyword line via `line_suffix` with the type indented; emitting it inline instead would **swallow the cast type** (`x as // c A` — a non-idempotent content loss). A same-line block comment (`x as /* c */ A`) stays inline in both formatters and is not a divergence. Both forms are dual-stable in our formatter. See [as_satisfies_value_line_comment_prettier_divergence](../tests/fixtures/typescript/expressions/as_satisfies_value_line_comment_prettier_divergence/).

**After last list-member comma**: `b: 2, /* c */` → Prettier relocates a block comment trailing the **last** member's comma to before the comma (`b: 2 /* c */,`). tsv preserves it after the comma. Only the last member diverges (a comment after a non-last member's comma attaches as the next member's leading comment, where both formatters agree), and only in multiline form (inline lists drop the trailing comma, so both converge on `b: 2 /* c */`). Both positions are dual-stable in our formatter. Applies across comma-separated lists — object properties, object/array patterns, array elements, call arguments, import/export specifiers, function parameters (declarations, arrows, methods, function types), tuple-type elements, and type parameters. Generic type **arguments** (`Map<A, B>`) and array **arguments** in concise/fill layout keep no trailing comma, so both formatters converge there (no divergence). Enum members and `;`-separated interface/type members already preserve the comment position in both formatters.

**Call open paren `(` trailing**: `fn( // c` / `fn( /* c */` (a comment on the same line as a call's opening `(`) → Prettier relocates it to its own line as the first argument's leading comment (`fn(\n\t// c\n\t…)`). tsv keeps it trailing the `(` (`fn( // c\n\t…)`), treating the author's placement after `(` as a trailing comment on that line. This applies only when the call expands (a line comment after `(`, or own-line content among the args); a block comment that hugs the arg in a call that stays inline (`fn(/* c */ a)`) is unchanged and matches Prettier. When the author instead writes the comment on its own line, both formatters keep it there — the two positions are dual-stable. Applies to simple-callee calls (`call_formatting.rs`), member-chain calls (`chain_args.rs`), and `new` expressions (`new_expression.rs`). For chains, a block comment trailing `(` plus an own-line leading comment keep source order (the naive handling reverses them). For `new`, a line comment trailing `(` (`new Foo( // c\n\ta)`) is preserved rather than dropped entirely (content loss); prettier instead floats it out to a statement-trailing comment (`new Foo(a); // c`) or relocates a block comment before `(`.

**Object/array/block open-delimiter trailing**: the same position-preservation rule as the call `(` case, generalized to the other opening delimiters. A comment on the same line as an object literal's `{` (`const o = { // c`), an array literal's `[` (`const a = [ // c`), a block body's `{` (`function f() { // c`, plain `{ // c`, arrow `=> { // c`), a type-parameter list's `<` (`function f< // c`, also classes/interfaces/type aliases/arrows), a function/constructor-type parameter list's `(` (`type Fn = ( // c`, `new ( // c`), an object/array destructuring pattern's `{`/`[` (`const { // c } = o`, `const [ // c ] = a`), a `namespace`/`module` body's `{` (`namespace N { // c`, `module M { // c`), a class/interface/enum body's `{` (`class C { // c`, `interface I { // c`, `enum E { // c`), a type literal's `{` (`type T = { // c`), an import/export specifier list's `{` (`import { // c`, `export { // c`), a tuple type's `[` (`type T = [ // c`), an index signature's `[` (`[ // c\n\tk: V]`), a computed property/member key's `[` (`{[ // c\n\tfoo]: 1}`, also class members, destructuring, and interface/type-literal members), or a multi-argument type-argument list's `<` (`Map< // c`) is kept on the delimiter line; Prettier relocates it to its own line as the leading comment of the first element/property/statement/parameter/member/specifier/argument (for a computed key prettier instead relocates it out to the **member's** own leading line, or — class members — leaves it glued flush to the key). This applies only to a **line** comment (or a block comment that co-occurs with one) when the construct expands; an inline block comment that hugs content (`{ /* c */ a: 1 }`, `[/* c */ x]`, `<T /* c */>`, `(/* c */ p)`, empty body `{ /* c */ }`) and an own-line block comment (which both formatters keep on its own line) are unchanged and match Prettier. When the author instead writes the comment on its own line, both formatters keep it there — dual-stable. Object literals are handled in `objects.rs`, arrays in `arrays.rs`, block bodies in the shared block-printing path (`expressions/blocks.rs`), type-parameter declarations in `types/type_params.rs` (covering function/class/interface/type-alias/arrow), function/constructor types in `types/function_types.rs`, object/array destructuring patterns in `expressions/patterns.rs`, `namespace`/`module` bodies in the shared statement-list walk (`statements/type_declarations.rs`, reusing `build_statement_list_docs`), class/interface/enum bodies in their member loops (`build_class_body_doc` in `statements/class.rs`; `build_type_elements_doc` and `build_enum_declaration_doc` in `statements/type_declarations.rs`), type literals in the multiline member path (`build_type_literal_doc_inner` → `build_multiline_member_prefix_doc`, `types/type_literal.rs`), import/export specifier lists in the shared multiline comma-list builder (`build_hardline_comma_list`, `statements/modules.rs`; the `with {…}` import-attribute brace passes `None` and still relocates), tuple types in their multiline element path (`build_tuple_type_doc_with_line_comments`, `types/composite.rs`, using `build_leading_comments_multiline_after_delim` for the first element), index signatures in `build_index_signature_member_doc` (`types/type_members.rs`), computed property/member keys in `build_computed_key_bracket_doc` (`expressions/objects.rs`, its breaking path — a computed key never breaks on width, so a line comment in either in-bracket gap, `[`→key (this case) or key→`]` (the [close-bracket sibling](#comment-relocation)), is the only trigger; block comments and the no-comment case keep the flat layout), and multi-argument type-argument lists in their multiline path (`build_type_arguments_doc_with_line_comments`, `statements/type_declarations.rs` for _type_ position; `build_type_parameter_instantiation_doc_with_line_comments`, `types/type_params.rs` for call/`new` _expression_ position — the single-argument leading-comment case hugs `<`/`>` and matches prettier) — all via the shared `Printer::delimiter_line_comment_prefix` helper (`comments.rs`, wrapping `PartitionedComments` + `should_force_expansion_for_comments`). For the type-param `<` and function/constructor-type `(`, preserving the comment also fixed a prior content-loss/correctness bug (the `<` line comment was dropped; the `(` line comment swallowed the following tokens). **Scope:** this covers object/array _literals_, object/array _destructuring patterns_, block bodies, `namespace`/`module` bodies, class/interface/enum bodies, type literals (the standard path — type aliases, annotations, function-param literals, intersection-trailing objects; the union-member / parenthesized-intersection _alignment_ rendering, e.g. `type T = | { // c } | B`, still relocates), import/export specifier braces (the `with {…}` import-attribute brace still relocates), tuple types `[`, index signatures `[`, computed property/member keys `[`, type-parameter `<`, function/constructor-type `(`, and multi-argument type-argument lists `<` in both _type_ position (`Map<A, B>`) and call/`new` _expression_ position (`foo<A, B>(x)`, `new Foo<A, B, C>(x)`) (single-argument lists hug and already match prettier). The analogous delimiter in method/call/construct _signature_ `(` still relocates to match prettier (both formatters agree there today) — extending the divergence to it is a consistent future increment, not a bug.

**Declaration keyword comments**: `abstract/* b */class B` → Prettier collects all comments between modifier keywords and emits them before the declaration name: `abstract class /* b */ B`. Similarly, `async /* c */ function* F()` → `async function* /* c */ F()`. tsv preserves comments between their original keywords. Both positions are dual-stable in our formatter.

**Anonymous function keyword comments**: `function /* c */ ()` → Prettier relocates comments between the keyword and `(` in anonymous function expressions, generators, and export default functions. No params: after `)` before `{`. With params: inside parens before first param. tsv preserves the comment between keyword and parens. Prettier's relocated forms are dual-stable (stable in both formatters); our keyword-adjacent form is only stable in our formatter.

**Anonymous function/class keyword line comments**: Same relocation as block comments above, but with line comments. `class // c\n{}` → Prettier moves into body: `class {\n\t// c\n}` (stable in one pass). `function // c\n()` → Prettier moves after parens: `function () // c\n{}` (pass 1), then into body: `function () {\n\t// c\n}` (pass 2, stable). Not idempotent for functions — takes 2 passes. With params: `function // c\n(x)` → Prettier moves into params: `function (\n\t// c\n\tx,\n)` (stable in one pass). Also applies to generators (`function*`), async functions, and export default. tsv preserves the comment between keyword and next token. Both positions are dual-stable.

**Arrow body stripped parens**: `() => (x /* c */)` → Prettier strips parens and moves comment to params (`(/* c */) => x`). For curried arrows, parens are stripped and comment trails (`(a) => (b) => z /* c */;`). tsv preserves parens to keep comments in place: `() => (x /* c */)`, `(a) => ((b) => z /* c */)`. Same approach as unary expression fix. Each formatter normalizes to its own form (prettier strips parens, tsv preserves them).

**Sequence last-operand trailing edge (statement context)**: when a redundantly-parenthesized sequence is a statement's whole right-hand side, a trailing comment on the last operand (`const b = (x, (y /* c */));`) floats out of the sequence parens to sit right before the terminating `;` (`const b = (x, y) /* c */;`). Prettier floats it one step further — _past_ the `;` (`const b = (x, y); /* c */`), drifting across three passes to get there. tsv keeps it before the `;`, preserving the comment's association with the operand, consistent with its broader before-semicolon handling (the **Before-semicolon comments** entry above). The leading-edge counterpart (`const a = ((/* c */ x), y)` → `const a = /* c */ (x, y)`) reaches prettier's fixed point and is _not_ a divergence; and in call-expression context — where no `;` sits at the edge — both edges match prettier's fixed point (see the normalization-quirk entry **Sequence operand edge comment** / `sequence/operand_edge_comment`). _Interior_ operand comments (between two operands, `(x /* c */, /* c */ y)`) stay inline without parens and match prettier (regular fixture `sequence/operand_comments`).

**Conditional type after `:`**: `? foo : // about bar` → Prettier moves comment to trailing on true branch (`? foo // about bar`), changing association from false to true branch. Both positions are dual-stable.

**Switch case before `{`**: `case 'a': // comment {` → Prettier moves comment after opening brace. Not idempotent—takes 2-3 passes to stabilize.

**For empty clauses**: Prettier puts comments in syntactically broken positions outside the parentheses.

**Do-while after `(`**: Prettier moves comments after `} while (` to after the semicolon. Unique to do-while; other constructs keep comments inside parens.

**Do-while between `)` and `;`**: `} while (x) /* c */;` → Prettier relocates the comment inside the condition parens — a block comment before `)` (`} while (x /* c */);`), a line comment forcing the condition to break. tsv keeps it after `)`. (Otherwise the comment is dropped entirely — content loss.)

**Export `}` to `;` (no `from`)**: `export {a as x} /* c */;` → Prettier relocates the comment inside the specifier braces — a block comment trailing the last specifier (`export {a as x /* c */};`), line comments forcing the braces to break with each attached inside. tsv keeps them after `}`: a same-line block comment trails the brace, line comments stay on their own line with `;` following. Only the no-`from` case diverges — with a `from` clause prettier keeps the comment after the source (`export {a} from './m' /* c */;`), so tsv matches prettier there. (Otherwise the comment is dropped entirely — content loss.)

**Import source to `;`**: `import {a} from './a' /* c */;` → Prettier keeps a same-line block comment after the source (matching tsv), but relocates a line comment past the `;` (`import {a} from './a'; // c`, the before-semicolon rule). tsv preserves the line comment before `;` (`import {a} from './a' // c\n;`). A block comment after `with {...}` import attributes is relocated by prettier _inside_ the attribute braces (`with {type: 'json' /* c */}`); tsv keeps it after `}`. The same before-semicolon handling applies to re-exports (`export * from './a' // c\n;`) and import-equals references (`import x = require('./a') // c\n;`). (Otherwise the comments are dropped entirely — content loss.)

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

- JSDoc type cast parens — [jsdoc_type_cast](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_prettier_divergence/)
- JSDoc cast expand-last — [arrow_jsdoc_cast_body_long](../tests/fixtures/typescript/calls/arrow_jsdoc_cast_body_long_prettier_divergence/)

**JSDoc type cast parens**: prettier-plugin-svelte preserves parentheses around JSDoc type cast expressions (`/** @type {T} */ (expr)`) because acorn's parser produces `ParenthesizedExpression` AST nodes that prettier keeps when preceded by `@type`/`@satisfies` comments. Prettier's babel/typescript parser (used for standalone `.ts`/`.js` files) strips them when `needsParens` says the inner expression doesn't need parens. When the inner expression does need parens (e.g., assignment expressions in return/throw), prettier keeps them in both contexts. tsv always strips the parens — the parser consumes them and returns the inner expression directly — and re-adds them via `needs_parens` when required. The [non-divergence fixture](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast/input.ts) confirms tsv matches prettier exactly in the TS context. In `.svelte` files, paren stripping can cause a secondary line-breaking difference: without parens the inner expression type (e.g., `CallExpression`) is visible to layout heuristics like expand-last-arg, while prettier-plugin-svelte's `ParenthesizedExpression` wrapper hides it — see [arrow_jsdoc_cast_body_long](../tests/fixtures/typescript/calls/arrow_jsdoc_cast_body_long_prettier_divergence/).

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

Every pattern in `benches/deno/lib/divergence/patterns.ts` links to:

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
