# hyphen_word_extent_prettier_divergence

A `-` where an **operand** is expected — the run's head, or straight after another
operator — followed by a byte that opens neither an ident nor a number for it. The
divergence is visible only behind another operator, which is where the two readings give
the `-` a different neighbour (`+-[a]`, `1.5 / /-[a]`).

tsv: the `-` is the operator (css-syntax-3 §4.3.9 / §4.3.10 read at the `-` itself), so the
head rule glues it to the member after it — `+-[a]`, one form for every authoring of the
run
Prettier: postcss reads `-[a]` as one **word**, so the `+` before it is a head `+` in front
of a word and keeps its gap — `+ -[a]`

The cells here are a **sample** of the rule, not its set — the rule is the §4.3.9 / §4.3.10
question, and the catalog entry below enumerates every printable-ASCII byte the two
readings part on.

`◆prettier_bug`. See
[conformance_prettier.md §Reasons tsv Differs](../../../../../../docs/conformance_prettier.md#reasons-tsv-differs)
and
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Hyphen word extent at an operand position".

## Why the tag is `◆prettier_bug`: prettier is not idempotent on its own pass-1 output

Prettier does have a fixed point here — `+ -[a]`, which is what `output_prettier.svelte`
pins, and which it reaches from `input` in one pass. What it is not is **idempotent** on
the form it writes from a *spaced* authoring: that first pass lands on `+-[a]`, and its own
second pass moves it.

| pass | input | prettier's output |
| --- | --- | --- |
| 1 | `+- [a]` | `+-[a]` — the head `-` has nothing but an operator on its left, so `isSubtractionNode`'s unconditional disjunct drops the separator |
| 2 | `+-[a]` | `+ -[a]` — re-tokenized, `-[a]` is now one `word` node, and the head `+`'s disjunct is gated on `requireSpaceAfterOperator`, which a word turns on |
| 3 | `+ -[a]` | `+ -[a]` — the gap survives from here |

The row-1 authorings are the ones no fixture marker can name. `+- [a]` and `+ - [a]` are
both authorings prettier normalizes to *tsv's* form, so they fit neither `unformatted_*`
(which a divergence fixture may not hold) nor `unformatted_ours_*` (which prettier must not
take to `input`). Those two spacings are pinned by
[tests/css_operand_hyphen_convergence.rs](../../../../../css_operand_hyphen_convergence.rs)
instead. The **third** spacing — a space before the member, none between the operators
(`+ -[a]`) — is a perfectly good `unformatted_ours_*`: prettier holds it (row 3) while tsv
normalizes it to `input`, and that is `unformatted_ours_spaces.svelte`.

## Which reading of the `-` each formatter takes

The question both formatters answer at an operand position is *where the token after the
sign begins*, and they answer it from different grammars:

| authoring | tsv | prettier | postcss's members | css-syntax-3's tokens |
| --- | --- | --- | --- | --- |
| `+-[a]` | `+-[a]` | `+ -[a]` | `operator +`, `word -[a]` | `<delim +>` `<delim ->` `[`-block |
| `+-#a` | `+-#a` | `+ -#a` | `operator +`, `word -#a` | `<delim +>` `<delim ->` `<hash a>` |
| `+-.a` | `+-.a` | `+ -.a` | `operator +`, `word -.a` | `<delim +>` `<delim ->` `<delim .>` `<ident a>` |
| `+-!a` | `+-!a` | `+ -!a` | `operator +`, `word -!a` | `<delim +>` `<delim ->` `<delim !>` `<ident a>` |
| `+-%a` | `+-%a` | `+ -%a` | `operator +`, `word -%a` | the same, at `%` |
| `+-]a` | `+-]a` | `+ -]a` | `operator +`, `word -]a` | the same, at an unmatched `]` |
| `+-~a` | `+-~a` | `+ -~a` | `operator +`, `word -~a` | the same, at `~` |
| `+->a` | `+->a` | `+ ->a` | `operator +`, `word ->a` | the same, at `>` |
| `+-^a` | `+-^a` | `+ -^a` | `operator +`, `word -^a` | the same, at `^` |
| `1.5 / /-[a]` | `1.5 / /-[a]` | `1.5 / / -[a]` | `number`, `operator /`, `operator /`, `word -[a]` | the same three delims one operator in |
| `+ -a` | `+ -a` | `+ -a` | `operator +`, `word -a` | `<delim +>` `<ident -a>` — §4.3.9's first bullet |
| `+-1.5` | `+-1.5` | `+-1.5` | `operator +`, `number -1.5` | `<delim +>` `<number -1.5>` — §4.3.10 |
| `+ -_a` | `+ -_a` | `+ -_a` | `operator +`, `word -_a` | `<delim +>` `<ident -_a>` — `_` is an ident-start code point |

The last three are the bound, and they are what makes the rule a rule rather than a list:
tsv asks css-syntax-3's own question — "would the `-` and what follows it start an
identifier (§4.3.9: an ident-start code point, a second `-`, or a valid escape) or a number
(§4.3.10)?" — so wherever the answer is yes the `-` is that token's head on both sides, and
the divergence is confined to the bytes where it is no.

## Why tsv does not take postcss's word

postcss's word scan runs to the next byte in *its* end set, and that set was drawn for a
value **printer**, not for a tokenizer: `#`, `.`, `!`, `%`, `[`, `]`, `~`, `>`, `^` and the
rest of the catalog's list are all word content to it, so a `-` in front of any of them is
swallowed by the word after it. tsv's own lexer does not read those bytes that way —
`-[a]` is a `<delim ->` and a simple block, as
[bracket_block_operator](../bracket_block_operator_prettier_divergence/) already relies on
when it keeps the block whole — and a splitter that disagreed with the lexer is what left
the run with two forms: the `-` at the end of `+-` is an operator on any reading, so the
head rule glued it onto the block, and the glued text then read back as one word. One
grammar for the `-` at both positions is what gives the document a single fixed point.

## Related

- [operator_head_glue](../operator_head_glue/) — the head rule these cells ride, and the
  agreement cells for a `-` that stands alone (`-[a]`, `- [a]` → `-[a]` on both formatters:
  the glue is the same, only the member count differs)
- [operator_head_weld](../operator_head_weld_prettier_divergence/) — the head rule's one
  exception, where the glue would **merge** two tokens; the bound rows here are the same
  §4.3.9 / §4.3.10 question asked one position over
- [double_dash_word_extent](../double_dash_word_extent_prettier_divergence/) — the other
  word-extent disagreement, running the other way: there postcss ends a word tsv keeps
  whole
