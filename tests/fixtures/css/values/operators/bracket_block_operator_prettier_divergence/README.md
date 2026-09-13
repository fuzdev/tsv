# bracket_block_operator_prettier_divergence

A `[…]` simple block whose content holds a `*`, a `+` or a `:` — Tailwind's
`--modifier(2.5, [*])`, and the same block inside any function, standing alone, or with a
member after it.

tsv: `[*]` — the block is word text, kept exactly as written
Prettier: `[ *]` — its word tokenizer ends a word at `*`, `+` and `:` inside the brackets
too, so the block's interior becomes several value nodes and the operator rule spaces them;
with a member after the block the second gap opens as well (`a[*](2.5)` → `a[ * ](2.5)`).
The `:` cells read `[a: b]`, the colon's own spacing, and are reachable only where a `:` is a
value token at all — inside a function's arguments and at a custom property's top level, both
spelled here (prettier's parser throws "Missed semicolon" on a plain property's `b: [a:b]`,
so that authoring has no oracle on either side)

`◆prettier_bug`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Operator byte inside a `[…]` block".

## Why the tag is `◆prettier_bug`

Not for instability — prettier's `[ *]` is prettier's own fixed point. The criterion it meets
is the **meaning change**: the output carries a `<whitespace-token>` the author did not write,
inside a simple block whose payload no grammar asked anyone to respace. css-syntax-3
§"Component value" makes a `[…]` block's payload a list of component values, and a
`<whitespace-token>` is one of them — so the block that comes back is not the block that went
in. That is the same argument [operator_head_weld](../operator_head_weld_prettier_divergence/)
makes one construct over, where prettier *removes* a separator and merges two tokens into one:
in a custom property's value the token sequence **is** the value, and a bracket block's
interior is read by whoever defined the syntax that put it there — Tailwind's `--modifier(…,
[*])` arbitrary value is not a CSS value at all — so neither formatter has a reading to
respace by.

⚠️ **tsv holds two forms here, and the second is a residual of its own.** Its output for an
authored `[*]` is `[*]`, but its output for prettier's `[ *]` is `[ * ]` — an authored space
inside a bracket block splits the value before the block rule can see it. That leg is separate
from the claim above, and it is why every cell here is authored glued.

## Why prettier writes the space

css-syntax-3 §"Component value" makes `[…]` a simple block whose payload is a list of
component values, but no value *formatter* descends into it: prettier's value parser leaves a
bracket block's interior alone wherever there is no word to split — `[1.50]` keeps its number
on both sides, and so do `[a]`, `[a-b]` and `[/]` (all controls here). What reaches inside is
only postcss-values-parser's **tokenizer**, which ends a word at `*`, `+` and `:` whatever
brackets are around it, and prettier's printer then spaces the pieces it finds (the `:` is
the colon node's own `[":", line]`, so its cells read `[a: b]` rather than `[a : b]`). The result is a word
split leaking through a block rather than a reading of one, and the pattern of spaces shows
it: a gap opens at every boundary the tokenizer found *except* the one before a group's last
node, which takes no separator — hence `[ *]` where the block ends the value, `[a *]`,
`[ * b *]`, and `[ * ]` where a member follows the block (`a[*](2.5)`).

## Why tsv keeps the block whole

tsv takes the block whole (`operators.rs`'s `simple_block_end`), which is the same answer
prettier gives everywhere its tokenizer has no word to split, and the only one that keeps
`--modifier(…, [*])` — a real Tailwind authoring — as the author wrote it. A `[…]` block's
content is not required to be a value at all, so respacing it is a rewrite of text the
grammar never asked anyone to read.

⚠️ One cell has a residual of its own: `a[*](2.5)` reaches tsv's verbatim value path. The
region before the `(` is read as a function name, and a `*` is not word content there (it is
one of the bytes postcss's own tokenizer reads across), so the value refuses the function
reading and the whole declaration prints from its source span — no number normalization
included. That is why the cell is spelled the same in `input` and in the
`unformatted_ours_numbers` variant.

See also [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Operator byte inside a `[…]` block").
