# Arrow body paren comment divergence

When grouping parens around arrow body expressions contain trailing
comments (`() => (x /* c */)`), tsv preserves the parens to keep the
comment in its original position (same approach as unary expressions).

Prettier 3.9 strips the parens and relocates the comment:
- Simple body: `() => x /* c */` (strips parens, comment trails the body)
- Call arg: `f(() => x /* c */)` (strips parens, comment trails)
- Curried: `(a) => (b) => z /* c */` (strips every level, comment trails)
- Line comment: `() =>\n\tx; // c` (strips parens, floats the line comment past `;`)
- Own-line block: `() =>\n\tx;\n\t/* c */` (strips parens, block comment after `;`)

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Arrow body stripped parens) and §Comment Position
Philosophy.
