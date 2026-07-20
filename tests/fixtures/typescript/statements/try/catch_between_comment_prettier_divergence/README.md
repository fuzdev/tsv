# catch_between_comment_prettier_divergence

Prettier moves comments between try/catch/finally blocks into the subsequent block
body (`catch (e) {\n\t// comment` instead of `} // comment\ncatch (e) {`).

tsv: preserves comments where the user placed them
Prettier: relocates comments inside the next block

"Where the user placed them" is the whole rule, and the *authoring* is the signal:
a comment that trailed the `}` stays trailing (the keyword drops to the next line
unless the comment is a block comment), while one on its own line keeps its own
line. A blank line above an own-line comment is authoring intent and is preserved
— the `}`→continuation-keyword gap has no body `{` to sit below it (see
§"No blank above a body block's `{`" in the conformance doc).

This is exactly what the `}`→`else` gap already does. Prettier does **not**
relocate at `else`, only here, so it is internally inconsistent on the same
question and is no oracle for this gap.

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
