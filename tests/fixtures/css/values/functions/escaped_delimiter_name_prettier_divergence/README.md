# escaped_delimiter_name_prettier_divergence

A function whose **name** carries an escape spelling a delimiter — `a\)b(0.1)`,
`a\"b(0.1)`, `a\'b(0.1)`.

Per CSS Syntax 3 a `\` followed by anything but a newline "is a valid escape"
(§"Check if two code points are a valid escape") whose escaped code point is the character
itself (§"Consume an escaped code point"), and §"Consume an ident sequence" consumes escapes
as ident content. So `a\)b` is the single ident `a)b`, and an ident sequence immediately
followed by `(` is a `<function-token>` (§"Consume an ident-like token"). tsv steps every
escape whole while it validates the name, so these parse as functions and their arguments
normalize like any other — `a\)b(.10)` → `a\)b(0.1)`.

Prettier's CSS parser (postcss) reads the escape's payload **byte** as structure: the `\)`
unbalances its paren count and the `\"` / `\'` open a string it never closes, so it
**stops normalizing** and emits each declaration verbatim, keeping the `.10`. The same
blindness makes it throw on
[escaped_close_paren_arg](../escaped_close_paren_arg_prettier_divergence/) and stop
normalizing on [escaped_paren_arg](../escaped_paren_arg_prettier_divergence/) — this is the
**name**-side of that argument-side pair. Which spelling of a character the author used is
all that separates the two sides: the hex forms `a\28 b(.10)` and `a\29 b(.10)` carry no raw
delimiter byte, and prettier normalizes those, agreeing with tsv (pinned by the plain
sibling [escaped_name](../escaped_name/)).

The escaped **open** paren is the payload tsv refuses outright: `a\(b(...)` leaves the name
region ending in a dangling `\`, which is not a valid escape and so not word content, so tsv
reads no function and prints the declaration verbatim — the same output prettier reaches by
unbalancing. That refusal is what keeps the value parser's escape-blind search for the
opening `(` sound, and the agreeing cell is pinned as a control in
[escaped_name](../escaped_name/). It is **wider than that agreement**, though: where the
escaped `(` happens to *balance* at the value's end, prettier reads its own one-byte word `\`
as the name and normalizes inside (`a\(2.50)` → `a\(2.5)`, `\(2.50)` → `\(2.5)`), so those
cells are an accepted cost of keeping the search blind rather than a shape the two agree on.

No `output_prettier.*`: on tsv's canonical form (`input`) prettier agrees, so the
divergence lives only in `prettier_variant_numbers` — a form prettier keeps stable that
tsv normalizes to `input`.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values).
