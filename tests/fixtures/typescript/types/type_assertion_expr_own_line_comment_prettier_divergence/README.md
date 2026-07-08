# Own-line comment after `>` in an angle-bracket type assertion

A line comment on its **own line** between the cast's `>` and the asserted
expression (`<string>⏎// c⏎z`). The companion to the trailing-`>` case in
[`../type_assertion_line_comment_prettier_divergence`](../type_assertion_line_comment_prettier_divergence/)
(where the comment trails `>` on the same line): there tsv keeps it trailing `>`;
here the author put it on its own line, so tsv keeps it on its own line.

## Formatter divergence (`_prettier`) — multi-pass

tsv keeps the comment on its own line, leading the expression, with the
expression on a continuation line one indent in. Prettier instead relocates it,
and is non-idempotent getting there: the first pass pulls the comment up to glue
on `>` (`<string>// c⏎z`, recorded in `output_prettier.svelte`), and a second
pass moves it the rest of the way into the cast, trailing the type
(`<⏎string // c⏎>z` — the fixed point pinned in `audit_signature.txt`, rule F4).
The fixed point is the same one prettier reaches for the trailing-`>` and
own-line-before-`>` comments — it collapses all three to a comment trailing the
type, while tsv keeps each at the author's position. See
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation (Angle-bracket type assertion).
