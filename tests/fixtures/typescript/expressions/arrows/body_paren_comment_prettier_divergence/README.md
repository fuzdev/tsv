# Arrow body paren comment divergence

When grouping parens around arrow body expressions contain trailing
comments (`() => (x /* c */)`), tsv preserves the parens to keep the
comment in its original position (same approach as unary expressions).

Prettier 3.9 strips the parens and relocates the comment (first-pass output;
prettier is non-idempotent here — see `audit_signature.txt`):
- Simple body: `() => x /* c */` (strips parens, comment trails the body)
- Call arg: `f(() => x /* c */)` (strips parens, comment trails)
- Curried: `(a) => (b) => z /* c */` (strips every level, comment trails)
- Line comment: `() =>\n\tx; // c` (strips parens, floats the line comment past `;`)
- Own-line block: `() =>\n\tx;\n\t/* c */` (strips parens, block comment after `;`)

At prettier's **fixed point** (second pass) the comment floats *past the `;`* in
every case but the call arg — `() => x; /* c */`, `(a) => (b) => z; /* c */`,
`(a) => (b) => (c) => w; /* c1 */ /* c2 */`, `() => x; // c` — detaching it from
the body entirely (pass 1 still showed `x /* c */;`). This past-`;` relocation,
not just the paren strip, is the strongest reason for the divergence; tsv keeps
every comment attached to the body where the author wrote it.

Further cases:
- Stacked line comments (`g`): each is kept on its own line inside the parens.
  tsv previously **swallowed** the second one (`x // c1 // c2`, dropping `// c2`
  into `// c1`'s text) — a content-loss bug, now fixed. A line comment forces a
  break before any following comment regardless of kind.
- Same-line comment group after a body break (`i`): a block + line comment the
  author wrote together (`/* i1 */ // i2`) stay together on one line.
- Leading + trailing (`h`): the leading comment hugs `=>` (both formatters keep
  it there) while the trailing comment keeps the parens.
- Object-literal body (`j`): here the parens are **required** (object/block
  disambiguation), not redundant grouping parens. tsv keeps the comment inside
  (`({ k: 1 } /* c */)`); prettier moves it outside the required paren
  (`({ k: 1 }) /* c */`), changing its association from the object to the whole
  expression.

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Arrow body stripped parens) and §Comment Position
Philosophy.
