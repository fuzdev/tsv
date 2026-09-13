# operator_head_weld_prettier_divergence

An operator with nothing before it — at the head of the value, after another operator,
after a comma, at a group's head — where gluing it to the member after it would **merge
the two into one token**: `+ 2.5`, `- 2.5`, `- a`, `- -a`, `- f(2.5)`, and the same two
merges at every other spelling of what they merge into (`- \61`, `- é`, `- ×a`, `- _a`,
`- 0.5`).

tsv: keeps the authored gap (`- 2.5`), so the value's token stream is the one the author
wrote
Prettier: welds (`-2.5`), because its head rule is keyed on the *node* beside the operator
and never asks what the two spell together

`◆prettier_bug`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Operator head weld".

## Why prettier welds

`printCommaSeparatedValueGroup`'s "Formatting `/`, `+`, `-` sign" arm drops the separator
when the operator has no operand on its left (`isMathOperator && (!iPrevNode ||
isMathOperatorNode(iPrevNode))`), and its subtraction disjunct carries no further
condition — so a head `-` glues in front of every member kind and a head `+` glues in
front of every member kind postcss did not read as a word. The arm reasons about node
adjacency, not about the tokens the glued text lexes as.

## Why tsv keeps the gap

The weld changes the token stream, the same reason tsv keeps the gap after a lone `@`
([atword_empty_gap](../atword_empty_gap_prettier_divergence/)). In a custom property's
value the token sequence *is* the value (css-variables-1 §"Custom Property Value
Syntax"), and even outside one a formatter that turns two tokens into one has rewritten
the author's tokens rather than their spacing:

| authoring | tsv | prettier | what the weld does to the tokens |
| --- | --- | --- | --- |
| `+ 2.5` | `+ 2.5` | `+2.5` | §4.3.1: a `+` whose next code points would start a number is consumed *into* it — `<delim +>` `<number 2.5>` become `<number +2.5>` |
| `- 2.5` | `- 2.5` | `-2.5` | §4.3.1 / §4.3.10, the same for `-` — `<number -2.5>` |
| `- a` | `- a` | `-a` | §4.3.9 "would start an identifier": `-` followed by an ident-start code point — `<delim ->` `<ident a>` become `<ident -a>` |
| `- -a` | `- -a` | `--a` | §4.3.9's second bullet (`-` followed by `-`) — one `<ident --a>`, the custom-property-shaped spelling |
| `- f(2.5)` | `- f(2.5)` | `-f(2.5)` | the ident weld with a `(` after it: §4.3.4 "Consume an ident-like token" makes `-f(` a `<function-token>`, so the value now calls a *different* function |
| `- \61` | `- \61` | `-\61` | §4.3.9's third bullet: `-` followed by a **valid escape** starts an ident too, so the pair is the single `<ident -a>` — `\61` spells `a` |
| `- \?` | `- \?` | `-\?` | the same bullet, whatever the escape spells: `<ident -?>` |
| `- \31` | `- \31` | `-\31` | and again where it spells a digit: `<ident -1>`, which is the escape's whole point — unescaped, `- 1` would merge into the *number* `-1` instead |
| `- é` | `- é` | `-é` | the ident-start set is not the ASCII letters: §4.2 takes in the **non-ASCII ident code points** too, and `é` (U+00E9) is inside the U+00D8–U+00F6 range — `<ident -é>` |
| `- _a` | `- _a` | `-_a` | and `_` — `<ident -_a>` |
| `- 0.5` | `- 0.5` | `-0.5` | §4.3.10, the `2.5` row's merge one canonical form over; the `unformatted_ours_numbers` variant authors it as `- .50`, where §4.3.10's second bullet (a `.` then a digit) is what the `-` would consume |
| `+ 0.5` | `+ 0.5` | `+0.5` | the same for `+` |
| `- ×a` | `- ×a` | `-×a` | the same ident weld at a code point §4.2's enumeration leaves out: tsv's lexer reads every code point at U+00A0 or above as ident content, so the pair is one `<ident -×a>` — see the section below for which set is normative here |
| `- ÷a` | `- ÷a` | `-÷a` | and again at U+00F7, the other gap in §4.2's ranges — `<ident -÷a>` |

Five cells are the same two welds at the remaining head positions — after another
operator (`1.5 / - a`, `1.5 * - 2.5`, and the glued head pair `+- 2.5`, whose `+-` glues
on both formatters because a `-` merges into nothing), inside a function's arguments
(`f(- 2.5)`) and after a comma (`a, - 2.5`) — each of which prettier's head rule reaches
and each of which merges exactly as above. ⚠️ **Only the `-` disjunct reaches all four.** After another
operator prettier's `/` and `+` disjuncts are still gated on
`requireSpaceBeforeOperator` / `requireSpaceAfterOperator`, so a head `/` or `+` there
keeps its gap in front of a word — an `@`-word and a `[…]` block included — or a
function (`1.5 / / a` and `1.5 / / f(2.5)` are agreement cells in
[operator_head_glue](../operator_head_glue/); `1.5 / / @a` and `1.5 / / [a]` agree too); the unconditional glue at those
positions is `isSubtractionNode`'s alone, which is why every cell here but the two `+`
ones is a `-`.

## Which ident-start set the refusal is graded against

The merge question is "would the glued text lex as one token", and *whose* lexing answers it
is a choice this fixture makes: **tsv's own lexer's ident-start set is normative here**, not
css-syntax-3 §4.2's enumerated ranges. tsv reads any code point at U+00A0 or above as ident
content (`lexer::identifiers::is_non_ascii_identifier_codepoint`), and so does the parser tsv
is a drop-in for — Svelte's `parseCss` reads an identifier with `codePointAt(0) >= 160` in
its own `read_identifier`. Both therefore read `-×a` as one `<ident>`, and `parseCss` proves
it at a position where it does tokenize an ident: the selector `-×a` comes back as a single
`TypeSelector` named `-×a`.

§4.2 is the only dissenter. Its `non-ASCII ident code point` is a list of ranges, and
U+00D7 (`×`) and U+00F7 (`÷`) fall in the two holes it leaves (U+00C0–U+00D6, U+00D8–U+00F6),
so by that reading `-×a` is `<delim ->` `<delim ×>` `<ident a>` both glued and spaced, and
prettier's weld would be **lossless** at exactly these cells. tsv keeps the gap anyway, and
that is the choice: grading the refusal against a set tsv does not tokenize with would make
the formatter's answer disagree with its own parser and with the parser it replaces, at the
one spelling where the two readings differ. `- ×a` and `- ÷a` pin it.

## The line, and the sibling that holds the other side of it

The head rule itself is not the divergence: where the glue is **lossless** tsv glues too,
and [operator_head_glue](../operator_head_glue/) pins that — `/2.5`, `/a`, `-@a`,
`-(2.5)`, `-[a]`, `-'x'`, `-+a`, `+(2.5)`, `+'x'`, none of which can continue the
operator's token (an `@`, a `(`, a `[`, a quote and a second operator all end it, and a
`/` is a `<delim-token>` beside anything). One rule, one exception: **an operator with
nothing before it glues to the member after it unless the glue would lex as one token.**

## Related

- [operator_head_glue](../operator_head_glue/) — the lossless side of the same rule, where
  the two formatters agree
- [atword_empty_gap](../atword_empty_gap_prettier_divergence/) — the same refusal one
  member kind over: prettier welds a lone `@` onto what follows, turning a `<delim-token>`
  and an ident into one `<at-keyword-token>`
- [signed_number_after_word](../signed_number_after_word_prettier_divergence/) — the mirror
  image: there prettier **splits** a `<number-token>` whose sign the author glued
