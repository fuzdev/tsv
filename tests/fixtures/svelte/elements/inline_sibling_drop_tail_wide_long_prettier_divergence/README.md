# inline_sibling_drop_tail_wide_long_prettier_divergence

tsv: when the element inside an inline-sibling wrap is wide enough to lay its **own content** out
block-style, the non-terminal text run after it takes the ordinary per-width boundary — it hugs the
intact `</a>` when it fits there. Prettier: drops the tail to its own line once the element is
multiline. (Prettier's own-line form is now *also* a tsv fixed point: in that geometry the comment
sits on its own line, the element is unwrapped, and the tail's authored newline after a
multiline-rendering element is preserved — the layout-keyed rule. So `output_prettier` holds under
both formatters, and the divergence is which form each *reaches* from the wrap authorings.)

## Reason

This is the width just past
[inline_sibling_drop_tail_flow_long](../inline_sibling_drop_tail_flow_long_prettier_divergence/),
and it is the razor that retired the fused element+tail measurement that fixture used to pin.

That measurement resolved the two boundaries meeting on the element *outside-in*: fusing element and
tail into one unit is what pushed the leading boundary over, and the tail rode that break. It was
conditioned on the element sitting inside its inline-sibling wrap — a property its own output
destroys, since the wrap exists only while the sibling and the element share a line and breaking
that line is the fusion's whole action. Where the element stayed **intact** the two answers agreed
by arithmetic: its closing tag sits at print width, so the tail did not fit after it either way.
A **block-styled** element puts `</a>` back at the content indent, where the tail *does* fit — so
the same document formatted to the dropped tail and reformatted to the hugged one, forever. No
existing gate could see it: the strayed pass is only reachable at widths no fixture happened to sit
at, which is why it took the width sweep (`deno task razor:audit`) to surface.

The per-width answer is the only convergeable one here, and it is what every neighbouring shape
already gave: the tail boundary's **space** spelling after a multiline inline element is a fill
decision measured from the closing tag's own column, however the element came to be multiline
([inline_wide_content_text_sibling_long](../inline_wide_content_text_sibling_long_prettier_divergence/)
for the unwrapped prose-content case). Three of the five sibling kinds — an element, a tag, a block
element — always took it; only the two non-flowing ones (a comment, a control-flow block) were held
back by the fusion.

## Cases

- **100 (control)** — the element's line is exactly print width, so its content stays intact and the
  tail takes its own line. Both formatters agree here.
- **101** — the same document one character wider: the content lays out block-style and the tail
  hugs the closing tag. This is the sole line prettier differs on.

`unformatted_ours_same_line` authors both documents with the comment, the element and the tail all
on one line — the authoring the wrap comes from — and normalizes to `input` in one pass.
`divergent_variant_dangle` is prettier's own fixed point for that authoring, where it dangles the
closing `>` rather than laying the content out block-style; tsv rewrites it to `output_prettier`'s
form (block-style content, the authored newline tail preserved).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
