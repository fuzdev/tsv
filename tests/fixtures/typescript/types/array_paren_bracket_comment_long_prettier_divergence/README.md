# array_paren_bracket_comment_long_prettier_divergence

The width boundary of the break point the bracket comment introduces. At exactly 100
the comment stays inline in the brackets; at 101 the **brackets** are what give — the
suffix owns that break, so the element keeps its own line unchanged. The position rule
itself is
[array_paren_bracket_comment](../array_paren_bracket_comment_prettier_divergence/);
this fixture pins only where the layout flips.

tsv: the brackets break, the element stays put
Prettier: the element breaks, or the comment leaves the declaration

```
// tsv (101)                        // prettier (101)
type B = (AAA… & BBB…)[             type B = (AAA… & BBB…)[];
	/* c */                           /* c */
];
```

## Reason

Prettier has no in-declaration position left once its relocated `(…) /* c */[]` no
longer fits, so at 101 it strands the comment on its own line **after** the `;` — the
block-comment face of the line-comment relocation the position fixture already
catalogs, and the same information loss: the comment now reads as leading whatever
statement follows. At 100 it stays in-declaration but pays for the room by breaking the
intersection that tsv leaves intact.

Both element kinds flip at the same width (`type C` / `type D`), even though prettier's
in-declaration destination differs between them (before the `[` for the intersection,
inside the parens for the union) — the boundary belongs to the suffix, not to the
element.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
