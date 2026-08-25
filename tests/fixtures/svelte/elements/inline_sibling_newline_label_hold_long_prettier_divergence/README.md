# inline_sibling_newline_label_hold_long_prettier_divergence

The sibling-newline flow rule's prose gate
([inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/))
at the print-width boundary. A one-word run is a label whose authored newline is held, but its
**space** spelling is still a fill: at 100 chars `<Comp … /> text1` stays inline, and at 101 the
fill wraps at the separator — onto the newline spelling, which is then the held form. Both the
component and the void-element (`<input … /> text1`, the checkbox-and-caption shape) boundaries
are pinned at 100 and 101, and a tag too wide to fit even alone breaks its attributes instead,
the word staying hugged to the closing delimiter (`/> text1`), as prettier does too.

## Reason

Two claims, one of them a divergence. The **wrap** is tsv's print-width-as-hard-limit stance
(the Elements catalog's fill-boundary family): prettier's fill tolerates the 101-char line and
leaves the space spelling as it is, so `prettier_variant_space.svelte` — the space spelling of
every case — is a prettier fixed point, which tsv normalizes to `input.svelte`. The **hold** is
what the wrap lands on: once the fill has written `<Comp … />⏎text1`, the newline is a
one-word run's and the flow rule holds it, so the wrapped form never re-packs when the line
later has room — the same one-way ratchet prettier has at this shape, and the cost the label
hold accepts on purpose. This fixture pins that the width-driven wrap and the authored newline
are one spelling (`input.svelte` is both formatters' fixed point), so the ratchet is the
composition of two pinned rules rather than a third.

See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements)
(the fill-boundary family) and
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
(the prose gate).
