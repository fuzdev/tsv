# Prettier Conformance

Prettier was tsv's initial guide, and the formatter still tracks it for the common case — but tsv has its own identity and makes **intentional, cataloged choices** to diverge where they're more defensible. This document catalogs those divergences along with bugs that tsv does not replicate.

The catalog is split by language; this doc holds the shared frame — terminology, the
`◆reason` tags, the prettier-bug index, and the decision framework every entry answers to.

## Terminology

**Matched**: tsv produces identical output to Prettier — the goal and the common case (measure current rates with `deno task corpus:compare:format --all --summary`).

**Unmatched**: tsv produces different output. The suffix `_prettier_divergence` marks these fixtures. This document explains WHY for each case.

## Reasons tsv Differs

Each divergence is tagged with one or more `◆reason` tokens — single greppable
units (search `◆` for every tag, `◆prettier_bug` for one category):

- `◆spec_violation` — Prettier emits spec-violating CSS/HTML/JS. tsv follows the spec
- `◆spec_precedence` — Prettier's output is valid but tsv emits the spec's canonical serialized form
- `◆stable_quirk` — Prettier preserves multiple forms without normalizing. tsv normalizes consistently
- `◆prettier_bug` — Prettier is non-idempotent, emits invalid output, or changes meaning (e.g. strips required parens). tsv produces stable, valid, meaning-preserving output
- `◆parser_compat` — Prettier's output breaks Svelte's parser. tsv produces Svelte-compatible output
- `◆print_width` — Prettier allows lines to exceed printWidth. tsv breaks to stay within the limit
- `◆bom_stripping` — Prettier preserves byte-order marks. tsv strips them
- `◆comment_preservation` — Prettier moves comments to a different syntactic position. tsv preserves comment position
- `◆content_preservation` — Prettier silently drops authored content — usually comments, sometimes other semantics-bearing tokens (a directive `|modifier`, a list element). tsv preserves it
- `◆design_choice` — Other deliberate behavior differences, with rationale in the fixture

> Most `◆comment_preservation` and `◆content_preservation` divergences live in the prose-form [TypeScript: Comments](./conformance_prettier_ts_comments.md#typescript-comments) and [CSS: Comments](./conformance_prettier_css.md#css-comments) catalogs, not the tag-prefixed catalog lists — they're the largest divergence category but don't reduce to a single bullet.

### Prettier bug index

Every `◆prettier_bug` — cases where Prettier produces output that is non-idempotent, fails to re-parse, throws, or changes meaning, and tsv refuses to replicate. Grep `◆prettier_bug` for each in context.

**Cataloged** (pinned by a fixture oracle or marker):

- CSS empty value + `!important` — never converges (oscillates every pass) — [empty_value_important](../tests/fixtures/css/values/variables/empty_value_important_prettier_divergence/)
- CSS escaped whitespace in a value (`50px\ ;`) — drops the escape's payload, stranding the `\` onto the `;` / `)` → output fails to re-parse; also splits an ident on an escaped `,` / `+`, and wraps a long value *inside* an escape — [escaped_whitespace](../tests/fixtures/css/values/escaped_whitespace_prettier_divergence/), [escaped_whitespace_long](../tests/fixtures/css/values/escaped_whitespace_long_prettier_divergence/)
- CSS escaped whitespace ending an at-rule prelude (`@layer a\ ;`) — same, stranded onto the `;` → the at-rule never ends; alone the output fails to re-parse, in context two at-rules silently merge — [layer_escaped_whitespace](../tests/fixtures/css/at_rules/layer_escaped_whitespace_prettier_divergence/)
- CSS escaped whitespace ending a `url()` (`url(x\ )`) — same, stranded onto the url's closing paren → the token never terminates and the output fails to re-parse — [url_escaped_whitespace](../tests/fixtures/css/values/functions/url_escaped_whitespace_prettier_divergence/)
- CSS comma list ending in an empty element (`transition: a,,`, `linear-gradient(red,,)`, `--x: a,,`) — writes one comma too few, so its own next pass reads the element as gone and deletes it (non-idempotent); the shortened list is a **semantic** change — an invalid gradient the UA discards becomes a valid one that paints, and a custom property's substituted token sequence loses a token — [comma_trailing_empty_element](../tests/fixtures/css/values/lists/comma_trailing_empty_element_prettier_divergence/)
- CSS empty `<media-query>` in `@import` position (`@import url('a.css'),;`) — deletes the list's only entry; that list is the single query `not all` (**false**, so the import never applies), and an *empty* `<media-query-list>` evaluates **true** (mediaqueries-4 §"Media Queries"), so the import now always applies — a semantic change — [media_query_empty](../tests/fixtures/css/at_rules/media_query_empty_prettier_divergence/)
- `<svelte:element this={'x'}>` — ignores `singleQuote` and skips escaping → invalid output (`this={"a"b"}`) — [svelte_element_this_string](../tests/fixtures/svelte/special_elements/svelte_element_this_string_prettier_divergence/)
- `<svelte:element class="a  b">` — fails to collapse repeated whitespace — [svelte_element_class_whitespace](../tests/fixtures/svelte/special_elements/svelte_element_class_whitespace_prettier_divergence/)
- Space after block element — strands a leading space, non-idempotent — [space_after_block](../tests/fixtures/svelte/elements/space_after_block_prettier_divergence/)
- `//` comment in a `<pre>` / `<textarea>` attribute list — ejects the comment out of the element (`</pre> // c`, `</textarea // c⏎>`), so it renders as page text or is dropped on the next pass; non-idempotent either way — [ws_sensitive_attr_comment_line](../tests/fixtures/svelte/elements/ws_sensitive_attr_comment_line_prettier_divergence/)
- Constrained `infer … extends` operand parens — strips required parens → output fails to re-parse — [constrained_extends_parens](../tests/fixtures/typescript/types/infer/constrained_extends_parens_prettier_divergence/)
- Negative literal type sign comment (`-/* c */ 1`) — *adds* parens to hold the comment (`-(/* c */ 1)`), but no such type exists: `-` is a negative literal type only when the next token is a numeric/bigint literal → output fails to re-parse — [negative_literal_sign_comment](../tests/fixtures/typescript/types/negative_literal_sign_comment_prettier_divergence/)
- TS instantiation parens (`(x ? y : z)<T>`) — strips parens, semantic change — [instantiation_parens](../tests/fixtures/typescript/typescript_specific/assertions/instantiation_parens_prettier_divergence/); the class-operand / `export default` case (`export default (class {}<T>)`) is worse still (strips → a class _declaration_) — [export_default_instantiation](../tests/fixtures/typescript/modules/exports/default_wrappable_leftmost_operators/instantiation_prettier_divergence/)
- Svelte destructuring rename-with-default key drop (`{ a: b = 1 }` → `{ b = 1 }`) — semantic change — [each](../tests/fixtures/svelte/blocks/each/destructure_rename_default_prettier_divergence/), [await](../tests/fixtures/svelte/blocks/await/destructure_rename_default_prettier_divergence/)
- `x?.#a` (optional chain to private field) — throws on valid input (pinned by `prettier_rejects.txt`) — [private_fields_optional_chain](../tests/fixtures/typescript/declarations/class/private_fields_optional_chain_prettier_divergence/)
- JSDoc cast + an enclosing paren — emits the paren *between* the comment and its `(`, so the cast re-binds to the wider expression; non-idempotent, and a **semantic** change (oxfmt and biome share the bug) — [jsdoc_type_cast_enclosing_parens](../tests/fixtures/typescript/syntax/comments/jsdoc_type_cast_enclosing_parens_prettier_divergence/)
- Bundler annotation (`/* @__PURE__ */`) + an enclosing paren — same relocation, so the annotation ends up leading a paren instead of the call it marks and the call is no longer treated as side-effect-free; a **semantic** change, and unlike the cast prettier is *idempotent* on its own output, so nothing reveals it — [pure_annotation_enclosing_parens](../tests/fixtures/typescript/syntax/comments/pure_annotation_enclosing_parens_prettier_divergence/)
- Multiline block comment before a postfix `++`/`--` — strips the grouping parens that held it (`(d /* m1⏎m2 */)++` → `d /* m1⏎m2 */++`), putting a line break in a `[no LineTerminator here]` gap → output fails to re-parse — [update_postfix_paren_line_comment](../tests/fixtures/typescript/expressions/unary/update_postfix_paren_line_comment_prettier_divergence/)
- Own-line JSDoc cast comment in a computed key — non-idempotent: pass 1 emits a broken mid-line form for every member kind, pass 2 reflows a property key but holds a method/accessor key broken — [objects/computed_key_jsdoc_cast_own_line](../tests/fixtures/typescript/expressions/objects/computed_key_jsdoc_cast_own_line_prettier_divergence/), [class/computed_key_jsdoc_cast_own_line](../tests/fixtures/typescript/statements/class/computed_key_jsdoc_cast_own_line_prettier_divergence/), [destructuring/computed_key_jsdoc_cast_own_line](../tests/fixtures/typescript/expressions/destructuring/computed_key_jsdoc_cast_own_line_prettier_divergence/) — see [§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)

**Prose-only** (no `output_prettier.*` oracle — Prettier drops or throws, so the bug can't be pinned as a fixture):

- `import defer * as ns from 'm'` — silently deletes the `defer` phase (information loss) — [§Import-phase proposals](./conformance_prettier_ts.md#import-phase-proposals)
- `import source x from 'm'` — Prettier's TS printer throws (`'=' expected`) — [§Import-phase proposals](./conformance_prettier_ts.md#import-phase-proposals)
- `{@const y = /** @type {T} */ (z)}` — Prettier emits invalid `(z}` then throws on its own output — [§JSDoc / paren semantics](./conformance_prettier_ts_comments.md#jsdoc--paren-semantics)

## Decision Framework

**When to match Prettier:**

- Cosmetic choices (spacing preferences, quote styles)
- Output that's valid and reasonable
- Unclear which approach is "better"

**When to differ:** any reason in [Reasons tsv Differs](#reasons-tsv-differs) above. The two cross-cutting principles — comment position and print width — are detailed below.

### Comment Position Philosophy

**A formatter should not move a comment to a different syntactic position — unless
the move is lossless and the position carries no authorship signal.** Comment
placement is usually a deliberate authoring choice — it communicates what the comment
refers to — so preserving it is tsv's default and its single largest category of
divergence from Prettier (see [TypeScript: Comments](./conformance_prettier_ts_comments.md#typescript-comments)).

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

A corollary the before-`:`/`=` gaps make explicit: **own-line-ness is authoring
signal for a leading position, not a trailing one.** A single-line block comment
that trails a head token (a key before its `:`, a name before its `=`) has its
unforced breaks collapsed — it stays in its authored gap, inline
(`a /* c */: 1`) — while a comment that leads a value (after the `:`/`=`) keeps
its authored own line **where the container keeps lines at all**: at statement-
and member-level initializers (`const x =⏎\t/* c */⏎\t1`) and inside object
literals (whose authored multiline-ness tsv preserves) the hang survives, but a
container that flattens when it fits — a destructuring pattern, a parameter
list — collapses the comment's break with the rest of its layout, the comment
staying glued leading the value (`{ a: /* c */ b }`, `a = /* c */ b`). The
author chooses the **association** by which side of the separator the comment
sits on, and tsv always preserves that; line structure is preserved only as far
as the enclosing layout keeps lines — for a **single-line** block, not at all on
the trailing side (it is pure layout there), per-container on the leading side.
A **multiline** block the author **broke after** is the exception on both sides:
that break is authoring signal (the [§Authored breaks in value
position](#authored-breaks-in-value-position) rule), so a trailing-side one
keeps it too — the comment trails the head and the separator + tail drop to a
continuation line (the Pre-separator multiline-block entry in [§Comment
relocation](./conformance_prettier_ts_comments.md#comment-relocation)). See the key→`:` own-line
block entries and
[rename_key_colon_own_line_block_comment](../tests/fixtures/typescript/expressions/destructuring/rename_key_colon_own_line_block_comment_prettier_divergence/)
in [§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation).

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
  continuation indent](./conformance_prettier_ts_comments.md#comment-relocation).
- **Prefix type operators** — the `keyof`/`typeof` operand hang
  (`type A = keyof // c⏎\t\tB`), shared via `append_keyword_value_line_comments` with
  type-parameter constraint/default values and class-property initializers. See
  [Prefix type-operator operand hang](./conformance_prettier_ts_comments.md#comment-relocation).
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
  **Exception**: an own-line `format-ignore` / `prettier-ignore` directive in the gap
  is NOT pulled up to trail the `:` — it keeps its authored own-line placement, so the
  freeze survives a second pass (a head-trailing directive is inert under the placement
  classification) — see **On single-child type positions** under
  [§Format-ignore directive](./conformance_prettier_ignore.md#format-ignore-directive).
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
  function parameters
  ([param_key_colon_line_comment](../tests/fixtures/typescript/declarations/function/param_key_colon_line_comment_prettier_divergence/)),
  destructuring renames
  ([rename_key_colon_line_comment](../tests/fixtures/typescript/expressions/destructuring/rename_key_colon_line_comment_prettier_divergence/)),
  and named tuple members — label→`:`, the optional `?`→`:`, and a rest member's
  label
  ([tuple/label_colon_line_comment](../tests/fixtures/typescript/types/tuple/label_colon_line_comment_prettier_divergence/)).
  Prettier keeps the continuation flush — and for property signatures / class
  properties (and every named-tuple member) relocates the comment to end-of-line. An **own-line** authoring of
  the same comment (`prop⏎// c⏎: T`) pulls up to trail the head and normalizes
  to the same continuation form in one pass — own-line-ness is authoring signal
  for a leading position, not a trailing one (the corollary in
  [§Comment Position Philosophy](#comment-position-philosophy)) — pinned as
  `unformatted_ours_own_line` variants across the family's fixtures.
- **Index-signature bracket gaps** — the `]`→value-`:` continuation
  (`[k: T] // c⏎\t: V`). See [Index signature `]`→value-`:`](./conformance_prettier_ts_comments.md#comment-relocation).
  A computed key's after-`]` separator gaps take the same layout — object `]`→`:`
  ([computed_key_bracket_colon_line_comment](../tests/fixtures/typescript/expressions/objects/computed_key_bracket_colon_line_comment_prettier_divergence/)),
  class `]`→`=`
  ([computed_key_bracket_line_comment](../tests/fixtures/typescript/statements/class/computed_key_bracket_line_comment_prettier_divergence/)),
  and destructuring `]`→`:`
  ([computed_key_bracket_colon_line_comment](../tests/fixtures/typescript/expressions/destructuring/computed_key_bracket_colon_line_comment_prettier_divergence/)).
- **Pre-keyword gaps** — the head→keyword half of a `<head> <keyword> <value>`
  construct, routed through the shared `route_pre_keyword_gap`: a type parameter's
  name→`extends` and before-`=` default continuations (`<T // c⏎\t\textends A>`,
  `<T extends A // c⏎\t\t= B>`), in every context including Svelte `{#snippet}`
  generics
  ([type_param_before_extends_line_comment](../tests/fixtures/typescript/types/comments/type_param_before_extends_line_comment_prettier_divergence/),
  [type_param_before_eq_line_comment](../tests/fixtures/typescript/types/comments/type_param_before_eq_line_comment_prettier_divergence/)),
  and a mapped type's key→`in` and constraint→`as`
  ([mapped/before_keyword_line_comment](../tests/fixtures/typescript/types/mapped/before_keyword_line_comment_prettier_divergence/)).
  Prettier relocates the comment past the keyword. Inlining is content loss, not a
  layout choice — the `//` swallows the `extends`/`=`/`in`/`as` tail into the comment.
  These gaps gate on **line comments only**: prettier glues a broke-after multiline
  block exactly as it glues a not-broke-after one at both sites, so there is no
  authoring distinction to carry and tsv matches (the switch-case head→`:`
  precedent). The pre-separator `:`/`=` gaps below hang it instead — not because
  prettier always distinguishes the two there (at a binding default it glues both,
  the same as here) but because a **separator** gap has a value on the far side
  whose own gap already honors the authored break, so collapsing it on the head
  side alone would answer the same question two ways within one construct. A
  keyword gap has no such counterpart.
- **Callee→empty argument list** — the call/`new` head→`()` continuation
  (`call // c⏎\t()`), uniformly for a plain callee, `new`, explicit type arguments,
  an optional call, and a member-chain callee. Inlining here is content loss, not a
  layout choice — the `//` swallows the call's own parens and the `;`. See
  [Callee→empty argument list](./conformance_prettier_ts_comments.md#comment-relocation).
- **Function/constructor-type `=>`→return type** — the arrow's operand hang
  (`() => // c⏎\tT`), the same seam this gap's *frozen* arm already reached through
  the shared `append_keyword_value_line_comments`. A union return keeps its own
  break-after-arrow layout. See [Fn/ctor-type `=>`→return-type line
  comment](./conformance_prettier_ts_comments.md#comment-relocation).
- **Switch `case`→test and case head→`:`** — the label's two gaps, one rule. The
  separator gap (`case x // c⏎\t\t\t:`) hangs the bare `:` itself; the keyword→test
  gap one step earlier (`case // c⏎\t\t\tx:`) hangs the test *and* that `:`.
  Inlining either swallows the colon into the comment (content loss). See
  [Switch `case`→test line comment](./conformance_prettier_ts_comments.md#comment-relocation)
  and [Switch case head→`:` line comment](./conformance_prettier_ts_comments.md#comment-relocation). A labeled statement's
  name→`:` gap is not a site of this rule — both formatters hoist the comment to
  lead the whole statement there (a match).
- **Svelte braced heads** — the head→value gap of every `{…}`, uniformly
  (`{@html // c⏎\texpr}`), via the shared `leading_line_comment_hangs_value`: the
  prefixed tags (`{@html}`, `{@render}`, `{@debug}`, `{...spread}`, `{@attach}`), the
  `{expr}` tag and attribute values
  ([expr_leading_line](../tests/fixtures/svelte/syntax/comments/expr_leading_line_prettier_divergence/),
  the whole family in one sweep;
  [expression_tag_line_comment](../tests/fixtures/svelte/syntax/comments/expression_tag_line_comment_prettier_divergence/)
  and [expr_multibyte](../tests/fixtures/svelte/syntax/comments/expr_multibyte_prettier_divergence/)
  are the `{expr}` tag's own pair, in ASCII and with multibyte comment text), a
  directive value whose expression self-expands
  ([on/line_comment](../tests/fixtures/svelte/directives/on/line_comment_prettier_divergence/)),
  the `{@const}` init (through its break-after-operator layout), and the block heads
  ([condition_breaking_comment](../tests/fixtures/svelte/blocks/if/condition_breaking_comment_prettier_divergence/)).
  Prettier keeps the continuation flush at all of them, and strips the comment outright
  at `{@debug}`. The **`}` column** moves with the indent and is the same question —
  a run-final `//` supplies the break the closer reuses, so that break must be emitted
  one level out or the closer lands at the *content's* column
  ([expr_trailing_indented_content](../tests/fixtures/svelte/syntax/comments/expr_trailing_indented_content_prettier_divergence/)).
  What differs across the family is only what each **host** does with its delimiters —
  a block head dangles its `}` at base ([§Svelte: Blocks](./conformance_prettier_svelte.md#svelte-blocks)), a prefixed
  tag hugs it, and a value that always block-wraps (`bind:`, and any directive whose
  expression does not self-expand) reaches the same hang through the block's own
  `indent`, with the comment on its own line inside the braces.

**Two gaps are outside this rule, and the grammar is what excludes them**: an
`as`/`satisfies` cast's operand→keyword gap and a postfix `++`/`--`'s
operand→operator gap are `[no LineTerminator here]`, so a continuation line would
be a syntax error — the keyword/operator may not start a line. There a comment
that spans lines keeps the operand's **grouping parens** instead
(`(⏎\tx // c⏎) as A`), which is the only position that expresses it. See
[`as`/`satisfies` operand, postfix operand](./conformance_prettier_ts_comments.md#comment-relocation).

The indent is tsv's own layout choice; prettier handles each site differently — it
relocates the comment (into braces/parens, after `from`/`as`/`;`), floats it past
`;`, keeps the continuation **flush**, or drops the comment onto its own line. So
this rule is a deliberate `_prettier_divergence` everywhere it bites: where prettier
**indents** the declarator (`const`/`let`/`var` keyword→declarator) it agrees, a
regular fixture; the multi-member union after `:` is a divergence (prettier drops the comment
onto its own line). The payoff is
internal consistency: every forced continuation reads the same regardless of which
construct the comment split.

### Authored breaks in value position

The complement of [§Uniform Forced-Continuation Indent](#uniform-forced-continuation-indent),
and the rule that decides when that one applies at all. A **line** comment *forces* a
break — `//` runs to end-of-line, so the tail cannot stay on the line, and the only
question left is how far to indent it. A **block** comment forces nothing: `x = /* c */ y`
is legal on one line, so a break after `/* c */` is the author's, not the comment's.

**tsv reflows an unforced break in value position.** A line break between a value gap's
head and its value carries no meaning tsv preserves — it is ordinary layout, and the TS
printer decides layout by width. This holds uniformly across every value position:
`const`/`let`/`export const` `=`, `type =`, `export =`, `export default`, assignment
expressions, class-property initializers, object-literal values, parameter defaults,
arrow bodies, `:` annotations, return types, `satisfies`, and `as`.

Prettier preserves the break at some of these sites and reflows it at others, so this is a
deliberate divergence wherever prettier happens to preserve. The payoff is the same as the
forced-continuation rule's: one answer regardless of which construct the comment sits in.

Two comment shapes are **outside** this rule, because their break is not unforced layout —
they hang the value, and the gate is the shared `comment_hangs_next`:

- A **line** comment forces the break (`//` runs to end-of-line), so the value drops to a
  continuation indented one level — [§Uniform Forced-Continuation
  Indent](#uniform-forced-continuation-indent).
- A **multiline** block the author broke after (`kw /* …⏎… */⏎v`) hangs the value; one whose
  value shares its closing line (`kw /* …⏎… */ v`) stays inline, the way prettier keeps it.

A **blank line** inside the gap **yields with the break**. `A = /* c */⏎⏎1` formats to
`A = /* c */ 1`, the same fixed point as the single-newline authoring. This is not a second
rule but a consequence of the first: a blank line is a property of a line break, so once the
break is judged unforced and reflowed there are no longer two lines for the blank to
separate. Elsewhere tsv treats a blank line as Tier-1 authoring intent and preserves it —
that still holds, and it is what decides this case rather than contradicting it. The blank
survives exactly where the break survives, which is the **own-line** authoring
(`= ⏎/* c */⏎⏎v`): there the comment does occupy its own line, the break is forced, and both
the break and the blank are kept. An author who wants the blank writes that form.

The rule holds at every value gap whose own-line authoring **hangs** the comment (the
initializer family: declarator, class property, object value, enum member, and the `=`/`:`
gaps that share their emitters).

**The converse holds in a forced continuation, on both sides of the separator.** Where the
break *is* forced — every site of [§Uniform Forced-Continuation
Indent](#uniform-forced-continuation-indent) — an authored blank inside the gap survives with
it, whether it sits between two comments or between the last comment and the tail
(`const e // c1⏎⏎\t// c2⏎\t= 1`). This is the same sentence as above read forwards rather than
backwards: the blank survives exactly where the break survives, and a `//` can only end its
line. It is one answer across the family's emitters — the forced-continuation run
(`build_trailing_comments_hang_next`, via `build_continuation_indent`), the declaration-header
run (`build_header_comment_run`), and the prefix-keyword operand seam
(`append_keyword_value_line_comments`) — each reaching the shared
`push_blank_preserving_hardline` off the physically-next-comment anchor (`blank_scan_end`), so
gate and emitter cannot answer differently. Prettier **agrees** wherever it leaves the run in
the gap (declaration header, `:`→type annotation, prefix type-operator operand, callee→empty
argument list, type-parameter `extends`), differing only in the continuation indent already
cataloged there; it drops the blank only where it **relocates** the run out of the gap
entirely — past the `=` at a binding initializer or default, to end-of-line at a property
signature — so the drop is incidental to the relocation rather than a considered answer.
Cataloged at
[continuation_blank_between_comments](../tests/fixtures/typescript/syntax/comments/continuation_blank_between_comments_prettier_divergence/).

**Known residual.** Two gaps still keep the blank: `await`→operand and `new`→callee (inside a
declarator; a bare expression statement has no group to collapse into, so it holds the break
regardless of width). Those emitters **relocate** an own-line comment up onto the head line
while the leading run picks its separator from the comment's *authored* position, so the two
disagree: pass 1 relocates and keeps the break, pass 2 sees a now-glued comment and collapses
it. That is not a stable resting place — it is a **non-idempotent format**, and the blank is
incidental to it.

The import/export header family had the same defect and is now fixed. The diagnosis there was
**not** a comment-position question: `gap_comment_continuation_tail` simply never consulted its
gate, keying the choice on line-vs-block alone while `comment_hangs_next` — the shared
keyword→value rule its `export default` / `export =` siblings already use — says a single-line
block collapses from *any* authored position. Routing the emitter through that gate made all 19
header gaps idempotent and is what `export * from` needed too (it had been the lone header gap
preserving the break, for want of the same collapse). The same shape is the likely fix for the
two remaining gaps: ask the gate, don't re-derive the answer at the call site.

Prettier is split here and not along tsv's line: it preserves the blank for a declarator and
an object value while collapsing it (with a relocation) for a class property, a parameter
default, and an enum member. Neither tool follows the other, so prettier is not the tiebreak;
the rule above is tsv's own, and it is a deliberate divergence wherever prettier preserves.

One site deliberately keeps a blank the rule would drop: a **conditional branch** — the
expression ternary's (`a ? /* c */⏎⏎b`, via `comment_followed_by_blank`) and the
conditional type's (`T extends U ? X : /* c */⏎⏎Y`, via the branch-gap run's blank arm)
alike — where the blank is itself a break trigger because prettier breaks on it too. There
the break survives, so the blank does — consistent with the rule, by a gate that forces
the break rather than an exception to it.

The rule is scoped to **value gaps** (head→value: `=`, `:`, `as`, a keyword). The
*pre*-separator gap (key→`:`, name→`=`) is governed by the trailing-position corollary in
[§Comment Position Philosophy](#comment-position-philosophy) instead — its unforced breaks
collapse the same way, because the comment there trails the head rather than leading the
value. The two shapes outside this rule are outside it there too: a **line** comment takes
the continuation indent, and a **multiline** block the author **broke after** keeps its
break — the separator + tail drop to the continuation line (the Pre-separator
multiline-block entry in [§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)) — so the broke-after
distinction survives on both sides of the separator.

A declaration's **head→body `{`** gap is the trailing-position corollary read where the
tail is a body brace, and takes all three answers unchanged: a single-line block collapses
onto the head line with `{` hugging it, and a line comment or a broke-after multiline block
drops `{` to its own line. The one adaptation is the indent — the brace lands **flush** with
the head rather than continuation-indented, because it owns the indent level beneath it and
must stay aligned with its `}`. It is one answer across every braced-body declaration
(function declaration and expression, class method, getter/setter, constructor, object
method, class, interface, `enum`, `namespace`/`module`), reached through the shared gate
rather than re-derived per emitter — the three
[declaration head→body entries](./conformance_prettier_ts_comments.md#comment-relocation)
catalog what prettier does instead. A **list** gap
is governed by ordinary blank-line preservation instead — a blank between two list items is
authoring tsv keeps, so an array element's leading comment preserves it
([arrays/end_of_line_block_comment](../tests/fixtures/typescript/expressions/arrays/end_of_line_block_comment_prettier_divergence/)).
The printer names the split as the two leading-run modes `AdjacentValueGap` and `Adjacent`.

Distinct from all of the above: a comment prettier **relocates** across the gap (`enum`
`A = /* c */⏎1` → `A /* c */ = 1`; binary `b + /* c */⏎c` → `b /* c */ + c`) is a
comment-*position* divergence, governed by [§Comment Position
Philosophy](#comment-position-philosophy), not by this rule.

### Print Width Philosophy

**Prettier treats `printWidth` as a soft target.** Lines may exceed it in various edge cases (fill algorithm boundaries, block expressions, template literals, certain constructs that "don't look good" when wrapped).

**tsv treats `printWidth` as a hard limit.** If content exceeds 100 characters, tsv breaks it when possible. This is a deliberate design choice affecting many divergences in this document:

- Block expression conditions (`{#if}`, `{#each}`, etc.) with logical operators
- Template literal interpolations
- Fill algorithm edge cases (101-char boundaries)
- Various "short expression" tolerances

The benefit: predictable output that respects the configured line length. The tradeoff: some constructs may break where Prettier would keep them inline.

**"When possible" has two systematic exceptions.**

*Whitespace-sensitive content.* Inside `<pre>` / `<textarea>` a line break *is* content, so print width yields to render semantics and an over-width line stands — those elements never reach the shared layout analysis (see [§Svelte: Inline content block-style](./conformance_prettier_svelte.md#svelte-inline-content-block-style)). Both formatters agree the *content* never re-wraps; the plain overflowing fill is pinned by [fill_tail_after_expr_pre](../tests/fixtures/svelte/elements/fill_tail_after_expr_pre_long/), whose two cases are the same overflowing fill in a `<pre>` and a `<textarea>`, and both formatters leave it intact. Where the over-width line carries a welded run with a tag in it, though, prettier finds a render-free break *inside tag syntax* — it dangles the closing tag's `>` (`…<b>welded</b⏎\t>tail …`) to duck under printWidth. tsv deliberately does not follow: the only available break is the tag-delimiter dangling tsv excludes everywhere else (§Svelte: Inline content block-style — tags stay intact), and adopting it solely inside `<pre>` costs more than the overrun it cures — a `>`-led continuation line reads as content exactly where code samples make a literal `>` ordinary, the machinery would insert breaks inside tag syntax within verbatim subtrees (where a one-byte miss is silent content corruption, and where prettier's own machinery has a cataloged bug — the attr-comment ejection above), and the shape occurs in no known real code. So the line stands, and an authored dangle rejoins to the intact line (tag-syntax whitespace is not content). Pinned by [ws_sensitive_welded_dangle_long](../tests/fixtures/svelte/elements/ws_sensitive_welded_dangle_long_prettier_divergence/) (the 100/101 boundary: at 100 both leave the run intact).

*A `{tag}` welded to a preceding word* is an instance of the rule, not an exception. When a `{expr}` / `{@html}` / `{@render}` tag is glued to the end of a text word with no whitespace (`… tsv is ~{ratio}`), the word and its tag are the **smallest welded unit**: they share one fit check, and when the pair does not fit, tsv breaks at the whitespace boundary *before* the word — the pair travels to the fresh line together, holding ≤ 100. Prettier keeps the tag *outside* the text fill, so its fill never sees the tag's width and never breaks before the word it is welded to: the word stays put and the tag rides past printWidth after it — a cataloged divergence, pinned by [fill_glued_tag_travel_long](../tests/fixtures/svelte/elements/fill_glued_tag_travel_long_prettier_divergence/) (the exact 100/101 boundary, a spaced follower packing after the traveled pair, and a tag whose expression must break — the pair travels first and the expression breaks internally on the fresh line). Breaking *between* the word and the tag is never an option — the glued boundary is render-significant. A tag *separated* from the preceding word by whitespace breaks before the tag itself ([fill_break_before_expr_long](../tests/fixtures/svelte/elements/fill_break_before_expr_long_prettier_divergence/)): there the whitespace boundary sits *directly before the tag*, and tsv breaks it (its hard limit outranks prettier, which overflows to 101). That boundary measures the tag as a **whole flat unit**, so a spaced tag whose expression itself must break travels the same way: it starts on the fresh line — collapsing flat there when it fits, breaking internally there when even a full line cannot hold it — and never opens mid-line at the end of the text line (the wide-element drop's tag analog). Prettier's boundary measurement stops at the expression's first internal break, so it keeps such a tag on the text line and opens it mid-line — a stable form each fixture pins as a `prettier_variant_midline`, while prettier also keeps tsv's traveled form, making the divergence one of normalization. Pinned by [fill_spaced_tag_travel_long](../tests/fixtures/svelte/elements/fill_spaced_tag_travel_long_prettier_divergence/) (the exact 100/101 pack/travel boundary, plus travel-and-collapse and travel-then-break-internally), [fill_expr_travel_continuation_long](../tests/fixtures/svelte/elements/fill_expr_travel_continuation_long_prettier_divergence/) and [fill_expr_travel_middle_long](../tests/fixtures/svelte/elements/fill_expr_travel_middle_long_prettier_divergence/) (text and further tags flowing after the traveled tag), [fill_expr_travel_middle_before_long](../tests/fixtures/svelte/elements/fill_expr_travel_middle_before_long_prettier_divergence/) (a second wide tag mid-run after it, which takes the same boundary break — travelling whole and breaking internally on its own line), and [fill_expr_travel_boundary_long](../tests/fixtures/svelte/elements/fill_expr_travel_boundary_long_prettier_divergence/) (the continuation line's own width boundary). The rule is uniform over runs holding **multiple** breakable expression tags ([fill_multi_expr_travel_long](../tests/fixtures/svelte/elements/fill_multi_expr_travel_long_prettier_divergence/)): every welded unit shares one fit check, a unit that fits at its position stays flat, and the first that does not travels whole — no expression is torn open while a whitespace boundary could still absorb the overflow. Text *following* a leading tag packs under the same limit — the fill's leading `line` is measured together with the word it stands before, so a word joins the tag's line only while the line holds ≤ 100, where prettier packs one word past the limit ([fill_leading_tag_pack_long](../tests/fixtures/svelte/elements/fill_leading_tag_pack_long_prettier_divergence/), the pack decision's own 100/101 boundary; contrast [fill_leading_line](../tests/fixtures/svelte/elements/fill_leading_line/), where the tag line sits at exactly 100 and both formatters break the leading line). A tag welded *onward* — into an **inline** element or component, glued text, or another tag — extends the unit through the weld, and the whole unit travels the same way (see [§Svelte: Inline content block-style](./conformance_prettier_svelte.md#svelte-inline-content-block-style), [inline_break_before_glued_long](../tests/fixtures/svelte/elements/inline_break_before_glued_long_prettier_divergence/) and [inline_welded_run_travel_long](../tests/fixtures/svelte/elements/inline_welded_run_travel_long_prettier_divergence/)); a tag glued to a following **block** element does not extend it — the block detaches to its own line regardless (render-free at a block boundary), so the weld survives only in the source and the measured unit stays the word+tag pair (the block-follower cases in inline_break_before_glued_long).

*A lone braced module list.* Two `{ … }` lists in the module grammar hold one line at any width, because breaking them buys a worse shape than the overrun: a **single named specifier** (`import { <name> } from '…'`, `export { <a> as <b> } from '…'`, either with a per-specifier or declaration-level `type`) and a **lone `type` import attribute** (`with { type: 'json' }`, the clause nearly every real import carries). Both formatters agree — these are Prettier's `canBreak` (`printModuleSpecifiers`) and `removeLines`-over-`isSingleTypeImportAttributes` (`printImportAttributes`) — so neither is a divergence; they are pinned by [imports/single_specifier_long](../tests/fixtures/typescript/modules/imports/single_specifier_long/), [exports/single_specifier_long](../tests/fixtures/typescript/modules/exports/single_specifier_long/) and [attributes_single_type_long](../tests/fixtures/typescript/modules/imports/attributes_single_type_long/). The rule is narrow on purpose and every way out of it restores ordinary width-driven breaking: a second specifier or attribute, a leading default/namespace binding, a non-`type` attribute key, a non-string attribute value, or a comment on the specifier / attribute (including one in a header gap, which Prettier attaches to the specifier — [type_keyword_comment_long](../tests/fixtures/typescript/modules/imports/type_keyword_comment_long_prettier_divergence/)).

*A test call's name.* `it` / `test` / `describe` and their member forms (`.only`, `.skip`, `.fixme`, `.step`, `test.describe.*`, plus `xit` / `fit` / `xdescribe` / …), called with a string or template name and a callback, keep their head on one line at any width — the name is an identifier a human reads, and wrapping it buys a worse shape while making every test in a suite taller. Both formatters agree (prettier's `isTestCall`, `utilities/test-libraries.js`), so this is not a divergence; it is pinned by [test_functions](../tests/fixtures/typescript/expressions/calls/test_functions/). The rule is narrow the same way the module-list one is, and every way out restores ordinary width-driven breaking: an **optional** callee (`describe?.(…)`) is excluded outright ([test_functions_optional](../tests/fixtures/typescript/expressions/calls/test_functions_optional/)), as are a non-string first argument, a non-callback second, a third argument that is not a numeric timeout, and any callee outside the pattern list — an ordinary call with the same shape breaks all its arguments out at 101, pinned at the exact boundary by [trailing_arrow_long](../tests/fixtures/typescript/expressions/calls/trailing_arrow_long/). The callback's own **parameters** ride the same rule, value and type alike: they are part of the head a reader takes in as one unit, and the line is already licensed to overrun, so breaking them buys a worse shape without restoring the limit. Prettier agrees, reaching it from the other side — its parameter printers ask `isTestCall` of the callback's *parent* (`print/function-parameters.js` `isParametersInTestCall`, `print/type-parameters.js` `isParameterInTestCall`). What goes flat is the **list**, never its contents: a single destructured parameter still expands on its own (`({⏎a,⏎b⏎}) => {}`), which both formatters reach through their hug path, and a comment in the list still opens it. Pinned by [test_functions_params](../tests/fixtures/typescript/expressions/calls/test_functions_params/). That licence belongs to the flat **layout**, not to the callee's name, and so lapses with it: when a call gives up the flat layout because an argument gap holds a comment ([§Comment relocation](./conformance_prettier_ts_comments.md#comment-relocation)), both halves of the argument fail — the name and the callback already sit on separate lines, and breaking the list now *does* restore the limit — so the parameters break at the ordinary 100/101 boundary like any other call's, value and type alike. Prettier holds them flat there but is not evidence, because it is never in that state: `printCallExpression` takes its test-call branch unconditionally, so for the 2- and 3-argument shapes tsv implements, `isTestCall` of the parent is true exactly when the call already printed flat (the `parent` argument separates the two only for the one-argument Angular wrapper, which tsv does not implement). Pinned at both boundaries by [test_call_expanded_params_long](../tests/fixtures/typescript/expressions/calls/test_call_expanded_params_long_prettier_divergence/). The **3-argument** form is narrower than the 2-argument one in both formatters — its callback must have a block body and at most one parameter — so `test('<long>', (a, b) => {}, 2500)` and `test('<long>', (a) => fn(a), 2500)` are ordinary calls that break every argument out and hold 100, pinned by [test_functions_timeout](../tests/fixtures/typescript/expressions/calls/test_functions_timeout/).

Everywhere else — the welded pair included — a line tsv *can* break is a line tsv *does* break.

---

## Catalogs

Each catalog is self-contained and governed by the decision framework above.

| doc | covers |
| --- | --- |
| [conformance_prettier_css.md](./conformance_prettier_css.md) | at-rules, selectors, values, layout, comments, CDO/CDC |
| [conformance_prettier_svelte.md](./conformance_prettier_svelte.md) | elements, inline content block-style, attributes, blocks, destructuring, form feed |
| [conformance_prettier_ts.md](./conformance_prettier_ts.md) | expressions, types, modules, template literals, input prettier rejects |
| [conformance_prettier_ts_comments.md](./conformance_prettier_ts_comments.md) | comment relocation, multi-word keywords, JSDoc / paren semantics, normalization |
| [conformance_prettier_ignore.md](./conformance_prettier_ignore.md) | the `format-ignore` / `prettier-ignore` freeze rule, across all three languages |

One catalog entry is a single rule with one bullet per language, so it stays here rather
than repeating three times.

## Whitespace: BOM Handling

**◆bom_stripping.** Prettier preserves byte-order marks. tsv strips them (they serve no purpose in UTF-8).

- Svelte — [bom](../tests/fixtures/svelte/syntax/whitespace/bom_prettier_divergence/)
- CSS — [bom](../tests/fixtures/css/tokens/whitespace/bom_prettier_divergence/)
- TypeScript — [bom](../tests/fixtures/typescript/syntax/whitespace/bom_prettier_divergence/)

---

## Tooling

**Corpus comparison** validates formatting against Prettier on real codebases:

```bash
deno task corpus:compare:format --all --explain           # The gates corpus view (~6,200 files: real repos + prettier suites)
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
For `<script lang="ts">` the plugin formats the embedded script through
prettier's real **`typescript`** parser (`embed.ts` →
`textToDoc(content, {parser: 'typescript'})`), so a construct that path throws
on — e.g. `@(a?.b)()` (a `TypeError` in prettier's needs-parens printer) or
`x?.#a` (`An optional chain cannot contain private identifiers.`) — **does**
trigger the verbatim fallback in `.svelte`, exactly as on a pure-`.ts` run. (A
`<script>` with **no** explicit `lang` is formatted through `babel-ts` instead,
whose stricter TC39 decorator grammar rejects forms like `@(f()).g` that tsc
and the `typescript` parser accept.) **Both tsv pipelines disarm this with
`PRETTIER_DEBUG=1`** (the tsv_debug sidecar sets it on the Deno spawn; the
`corpus:compare:format:run` task sets it in its env), which makes the plugin
and prettier-core rethrow — so `compare`, fixture validation,
`fixtures_update`, and corpus runs all report a hard prettier error (with a
code frame) instead of fake-stable output. The caveat applies when probing
prettier **outside** these pipelines (a bare `prettier` invocation, editor
integrations, upstream issue repros): there the fallback silently "preserves"
the whole script. Confirm by re-running the suspect construct in a single-form
file or as pure `.ts`, where no fallback exists so it fails visibly. (Also see
[fixture_overview.md §Common Pitfalls](./fixture_overview.md#common-pitfalls) —
the fallback can fake a "prettier-stable" fixture input.)

---

## Related

- ./conformance_svelte.md — Svelte parser differences
- ./fixture_overview.md — Fixture system details
