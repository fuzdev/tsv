# Negative literal type sign comment divergence

A comment between a negative literal type's `-` and its numeral. Prettier wraps the
numeral in parens to hold the comment, producing output that **fails to re-parse**:

- `type A = -/* c */ 1;` → `type A = -(/* c */ 1);`
- Union member: `-1 | -/* c */ 2` → `-1 | -(/* c */ 2)`
- Indexed-access index: `D[-/* c */ 1]` → `D[-(/* c */ 1)]`
- BigInt: `-/* c */ 1n` → `-(/* c */ 1n)`

tsv keeps the comment where the author wrote it, between the sign and the numeral.

## Reason

Prettier bug. A parenthesized operand is valid after a *value*-position unary minus,
so prettier reuses that comment-holding trick in type position — but there is no such
type. TypeScript reads `-` as a negative literal type only when the very next token is
a numeric or bigint literal (`parseNonArrayType`: `lookAhead(nextTokenIsNumericOrBigIntLiteral)
? parseLiteralTypeNode(true) : parseTypeReference()`), and the neighbouring comment in
tsc's own parser spells out the intent — "We don't want to consider things like '(1)'
a type." A `(` fails that lookahead, so prettier's output is rejected by tsc,
acorn-typescript, and Svelte's parser alike.

Preserving in place is what keeps tsv's output valid: `nextToken()` skips trivia, so a
comment between the sign and the numeral does not break the lookahead the way a paren
does. Without preservation the comment was dropped entirely (content loss) — the
position has no other emitter.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
