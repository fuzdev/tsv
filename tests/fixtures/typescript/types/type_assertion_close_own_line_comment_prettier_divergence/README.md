# Own-line comment before `>` in an angle-bracket type assertion

A line comment on its **own line** in the type→`>` gap of a cast, following a
trailing-type comment (`<⏎string // d⏎// e⏎>x`). Sibling of
[`../type_assertion_line_comment_prettier_divergence`](../type_assertion_line_comment_prettier_divergence/),
split out because prettier has no fixed point here.

## Formatter divergence (`_prettier`) — prettier non-convergent

Prettier cannot decide which side of the `>` the own-line comment belongs on. It
oscillates forever, flipping on every pass between pulling the comment past `>` to
lead the expression (`string // d⏎>// e⏎x`) and keeping it inside the cast on its
own line (`string // d⏎// e⏎>x`). With no fixed point, no `output_prettier.svelte`
is expressible — `prettier_nonconvergent.txt` records the claim and the validator
live-verifies it (rule F5).

tsv treats the cast's `>` as a semantic boundary and keeps the comment where the
author wrote it: on its own line before `>`, inside the cast. The `input.svelte`
form is idempotent under tsv. See
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation (Angle-bracket type assertion).
