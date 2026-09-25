# scope_boundary_run_prettier_divergence

A non-ASCII space (U+00A0, U+FEFF) at either end of a `@scope` prelude: after the
at-keyword ahead of the root clause (`@scope <NBSP> (.a)`) or ahead of `to`
(`@scope <ZWNBSP> to (.b)`), and after the last clause, before the block's `{`
(`(.a) <NBSP> {`, `to (.b) <ZWNBSP> {`). `parseCss` reads the prelude raw and trims it, so
the run is outside the wire's `prelude` string; to css-syntax-3 it is identifier content.
tsv keeps it where the author put it, one ASCII space either side; prettier drops it
(`@scope (.a) {`). The gaps where prettier keeps the run too — a clause's tail, both sides of
`to`, and a bare `@scope <NBSP> {` — are pinned by the plain
[scope_boundary_run](../scope_boundary_run/).

## Reason

Content preservation. Dropping a character the author wrote is content loss the corpus
SAFETY check reads as `content_lost`; tsv keeps the run at every juncture the parser steps
one at. See
[conformance_prettier_css.md §CSS: Selectors](../../../../../docs/conformance_prettier_css.md#css-selectors).
