# container_kind_newline_flow_prettier_divergence

The sibling-newline flow rule asked across **container kinds**. tsv: an inline sibling isolated
by an authored newline flows back onto the content line in an inline element, a block element, a
component and a `svelte:*` special element alike. Prettier: keeps the isolated authoring as a
second stable form, in every kind.

## Reason

Design choice — the same authoring-independence
[inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/) argues for.
Svelte 5 collapses inter-sibling whitespace to one whitespace, so a newline and a space between
two siblings render identically; the newline's *spelling* carries no signal and the fill reflows
it. The element's own multiline-ness is the separate axis and is preserved — both boundaries
carry air here, so every case converges on the **multiline** form rather than a one-liner.

## What this fixture adds — the container is not an axis

The rule is asked of the **run** — is there prose for a `fill` to pack? — never of the container
holding it, and no fixture varied the container to say so. Nearly every one in the family puts
the flowing run in a **block** container (`<p>` / `<div>`); the single inline container that
carries one,
[inline_attrs_multiline_content](../inline_attrs_multiline_content_prettier_divergence/), reaches
multiline past a 99-char attribute line, so there the kind is confounded with the attribute wrap
that fixture exists to test. The axis is worth asserting rather than inheriting, because an
inline container reaches the multiline form by a *different boundary rule* — both-or-neither,
rather than the block's leading boundary alone
([boundary_air_one_sided](../boundary_air_one_sided_prettier_divergence/)) — so it is where a
container-keyed rule would show. The run is held fixed and only the container varies:

- **inline element** — `<button>` and `<label>`, the shapes real components write (an icon or a
  checkbox, then its label), multiline by their own boundary air and nothing else.
- **block element** — `<p>`, the kind the rest of the family uses, here as the control that
  holds the axis: the answer must not move.
- **component** — `<Comp2>`, the third kind.
- **special element** — `<svelte:element>` (inline-classified) and `<svelte:head>` (block), the
  fourth kind on both boundary arities. A `SpecialElement` projects onto the same element
  analysis a regular element does, and that projection is what this pair pins.

A container-keyed rule would be wrong at one of its own boundaries and right at the next: within
a single run the boundaries touching a text node would flow while the one between two adjacent
siblings did not. The [`MultilineCause`] the container reaches multiline by is likewise not part
of the question — [inline_adjacent_sibling_newline_flow](../inline_adjacent_sibling_newline_flow_prettier_divergence/)
carries that control.

## Control — what does NOT flow

A **prose-free run** in an inline container keeps its authored lines. Flowing means reflowing
into a text `fill`, and a run with no content text has none, so those newlines are the author's
only structure. It is identical in `input.svelte` and `prettier_variant_newline.svelte`, which is
the point: it is the one run whose authored lines survive under both formatters.

`prettier_variant_newline.svelte` is the isolated authoring — prettier keeps it stable, tsv
normalizes it to `input.svelte`. Every boundary tsv collapses is inter-node whitespace that
renders as one space either way, so the output renders identically to the input.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
