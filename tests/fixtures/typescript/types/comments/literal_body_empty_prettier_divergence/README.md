# literal_body_empty_prettier_divergence

An object type literal whose body is nothing but an interior comment —
`type B = { /* block comment */ }`. tsv keeps the braces spaced; prettier 3.9.5
tightens them to `{/* block comment */}`.

tsv: `type B = { /* block comment */ };`
Prettier: `type B = {/* block comment */};`

(A line-comment body, `type A`, breaks multiline in both — no divergence there.)

## Reason

tsv applies bracket spacing uniformly: any object body that keeps its braces on
one line gets the ` … ` padding, a comment-only body included. It is not
special-cased on emptiness, so `{ /* … */ }` reads the same as any other
single-line body. Prettier 3.9.5 changed to strip the padding when the sole body
content is a comment (a truly empty `{}` has no content to space and stays tight
in both). Bracket spacing is hardcoded in tsv, so this is a fixed design choice,
not a configurable gap.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Empty-object comment bracket spacing.
