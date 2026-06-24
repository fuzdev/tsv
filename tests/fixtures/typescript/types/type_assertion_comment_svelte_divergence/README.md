# Parser divergence: angle-bracket type-assertion comment duplication

A block comment **inside** an angle-bracket type assertion's cast — between the
opening `<` and the type (`</* c */ string>x`), after the type before the
closing `>` (`<string /* c */>y`), or after the `>` before the expression
(`<string>/* c */ z`) — falls in a region acorn-typescript re-parses: it first
reads `<` as a less-than operator, then backtracks and reparses the whole
assertion, so its `onComment` callback fires twice and each such comment is
duplicated in the root `comments` array. Our parser keeps a single entry
(`expected_ours.json` vs `expected_svelte.json`). The set of distinct comments
is identical — only multiplicity differs — and `ast_diff` confirms semantic
equivalence.

A comment **before** the `<` (between `=` and the cast) sits outside the reparse
window, so it is not duplicated — that position is the regular (non-divergent)
sibling fixture `../type_assertion_leading_comment`.

Formatting is unaffected by the duplication: the formatter finds comments by
position, not by their count in the root array, and emits each comment once at
the author's placement — preserved at every position above (block comments;
line comments in these positions are a separate open item).
See [conformance_svelte.md](../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
