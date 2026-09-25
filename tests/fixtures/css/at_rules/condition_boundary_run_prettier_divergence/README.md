# condition_boundary_run_prettier_divergence

A non-ASCII space (U+00A0, U+FEFF) at the head of a condition prelude
(`@supports <NBSP> (a: b)`), after its `not` (`not <NBSP> (a: b)`), and at its tail before the
block's `{` (`(a: b) <ZWNBSP> {`, `@container (a: b) <NBSP> {`). tsv keeps the run where the
author put it, one ASCII space either side, and still reads the prelude as the condition it
is, so its parts normalize (`unformatted_ours_compact`: `(a:b)` → `(a: b)`). Prettier drops
the run at the head and the tail, and after `not` it RELOCATES it inside the paren
(`not (<NBSP> a: b)`) — a different token position to css-syntax-3, whose whitespace is ASCII
only and which reads the run as identifier content. The gaps beside a connector, where
prettier keeps the run in place, are pinned by the plain
[condition_boundary_run](../condition_boundary_run/).

## Reason

Content preservation. A character the author wrote is neither dropped nor moved across a
token; tsv keeps the run in place at every juncture the parser steps one at. See
[conformance_prettier_css.md §CSS: Selectors](../../../../../docs/conformance_prettier_css.md#css-selectors).
