# Post-arrow `prettier-ignore` placement divergence

The `=>`→body gap is a **value head**, so an own-line `prettier-ignore` there freezes the
body — the whole expression rides the freeze, in either comment spelling (`a1`, `b1`). Both
formatters honor that placement.

The two part on a directive the author wrote **on the `=>`'s line**. Under tsv's placement
floor a directive trailing a head is **inert**: the comment keeps the line it was written on
and the body normalizes (`c1`, `d1`). Prettier relocates the `//` spelling down to its own
line and then honors it there, so the body it freezes is one the author never asked to
freeze — the relocation *changes what the directive does*, which is the reason for the
divergence rather than the layout difference on its own.

`unformatted_ours_spaced_body` is where that shows, and the three forms it produces are why
this is a `divergent_variant_*` rather than a plain normalization test: tsv rewrites it to
the input (proving both directives inert), while prettier holds it stable at
`divergent_variant_spaced_body` (proving both honored). The block spelling `d1` parts the
same way without moving at all — prettier leaves the glued block where it is and honors it
there, so the placement floor is the whole of the disagreement, with no relocation to point
at.

This is the `=>` spelling of the same rule the keyword gaps take, and it reads identically
there: see
[await_new_operand_prettier_ignore_head](../../await_new_operand_prettier_ignore_head_prettier_divergence/),
whose last two cases are `c1`/`d1` one keyword over.

The comment keeping the `=>`'s line at all is
[post_arrow_glued_line_comment](../post_arrow_glued_line_comment_prettier_divergence/)'s
rule; this fixture is only about what the directive then *does* from there.

Out of scope: a curried chain's **head→head** gap, where a directive is inert in both
placements — that gap heads another signature rather than a value, so there is no value for
it to freeze.

Reason: directive placement floor. See
[conformance_prettier_ignore.md](../../../../../../docs/conformance_prettier_ignore.md)
§Format-ignore directive and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
