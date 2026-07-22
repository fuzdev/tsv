# union_empty_object_member_prettier_divergence

An empty object type whose body is an interior comment, appearing as a
union/intersection member — `type Commented = A | { /* c */ }`. tsv keeps its
bracket spacing where prettier 3.9.5 tightens it.

tsv: `type Commented = A | { /* c */ };`
Prettier: `type Commented = A | {/* c */};`

(A truly empty `{}` member — no comment — stays tight in both; those
non-diverging cases live in the regular
[union_empty_object_member](../union_empty_object_member/) fixture.)

## Reason

tsv applies bracket spacing uniformly: an object body kept on one line gets the
` … ` padding whether its content is members or a comment. It is not
special-cased on emptiness, so a comment-only body reads the same as any other
single-line body. Prettier 3.9.5 changed to strip the padding when the sole body
content is a comment. Bracket spacing is hardcoded in tsv, so this is a fixed
design choice, not a configurable gap. Same divergence as
[literal_body_empty](../comments/literal_body_empty_prettier_divergence/),
pinned here in union/intersection position.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Empty-object comment bracket spacing.
