# number_shaped_name_prettier_divergence

A word glued to a `(` whose text **is a number** — `50%(1.5)`, `1.5(1.5)`, `1#(1.5)`,
`.5(1.5)`. tsv reads it as the function's name and keeps the authoring; prettier splits it
into a numeric node and a parenthesized group and puts a space between them.

`◆design_choice`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Number-shaped function name".

## Where the two readings part

The value layer's oracle is postcss-values-parser, the parser behind prettier's CSS printer,
and its `<function-token>` is a **word token immediately followed by `(`** — not CSS Syntax
3's ident sequence, which is why `50%` and `aa/` are names there at all. But `splitWord`
tests that word against its own number production first
(`/^[+-]?((\d+(\.\d*)?)|(\.\d+))([eE][+-]?\d+)?/`), and a word that matches becomes a
`value-number` node carrying the rest as its **unit** — `50%` is the number `50` with unit
`%`, `1#` the number `1` with unit `#`. A numeric node is no name, so the `(…)` beside it is
a group rather than an argument list, and the two print as two members.

The gap between them is then not a question prettier asks. Its value printer has exactly two
arms for a group glued to the member before it — an `@word` (`softline`) and SCSS/Less unary
minus — and a numeric node is neither, so the loop falls through to its own closing line,
`// Be default all values go through 'line'`. Unlike the operator case it does not even
consult `hasEmptyRawBefore`: there is no authoring a number can have here that prettier
prints glued.

tsv takes the word whole. The detector asks what the word is made of, not whether it happens
to spell a number, so the name is `50%` and the arguments are arguments — which is what gets
`50%(1.50)` its `1.5`, where the numeric reading leaves the group's interior untouched.

## Why that is the side to be on

The glue is the author's, and tsv keeps one everywhere else it can appear — this is the same
answer [operator_before_group](../../operators/operator_before_group_prettier_divergence/)
gives for `1.5/(2.5)`, on the same synthesized group, with a weaker reason on prettier's
side (there the question is at least asked). Reading the word as a name also reaches the
*interior*: every normalization inside the parens runs on one side and not the other, so the
divergence is one fabricated space rather than a whole value frozen.

The `.5` cell is the price. A name is emitted from its span — the rule that keeps
`#FFF/(1.5)`, `u\72 l(…)` and every other authored spelling intact
([punctuation_name](../punctuation_name/), [escaped_name](../escaped_name/)) — so a name that
is a number does not take the number printer's canonical leading zero. Prettier, reading a
numeric node, writes `0.5`.

## Cells

| authoring | tsv | prettier |
| --- | --- | --- |
| `50%(1.5)` | `50%(1.5)` | `50% (1.5)` |
| `1e5%(1.5)` | `1e5%(1.5)` | `1e5% (1.5)` |
| `1#(1.5)` | `1#(1.5)` | `1# (1.5)` |
| `1.5(1.5)` | `1.5(1.5)` | `1.5 (1.5)` |
| `.5(1.5)` | `.5(1.5)` | `0.5 (1.5)` |
| `0a(1.5)` | `0a(1.5)` | `0a (1.5)` |

The last three declarations are the controls, where the two agree: a word the number
production does not match (`a0`, `a.5`) is a name on both sides, and an authored gap
(`1.5 (1.5)`) is a gap on both sides.
