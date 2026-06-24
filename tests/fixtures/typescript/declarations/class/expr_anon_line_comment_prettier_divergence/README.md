# Anonymous class expression line comment divergence

Prettier relocates line comments between the `class` keyword and
opening `{` in anonymous class expressions into the class body:

- Empty body: `class // c\n{}` → `class {\n\t// c\n}`
- With body: `class // c\n{ x = 1; }` → `class {\n\t// c\n\tx = 1;\n}`

We preserve the comment in the user's original position between keyword and
body. Per comment placement policy, user intent is preserved when prettier
moves comments to different syntactic positions.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
