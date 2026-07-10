# computed_leading_line_comment_prettier_divergence

A line comment before a computed member-access index, inside the brackets
(`arr[// c⏎i]`).

**tsv** keeps the comment inside the brackets, breaking so the index and `]`
drop to their own lines — the `//` can't swallow the index. **Prettier**
relocates the comment to lead the whole expression, before the assignment RHS.

```
// tsv                     // prettier
const a = arr[             const a =
	// c1                       // c1
	i                           arr[i];
];
```

A comment written on the `[` line itself (`arr[ // c`, not its own line) is kept
trailing the `[` — the open-delimiter-trailing rule
([conformance_prettier.md §Object/array/block open-delimiter trailing](../../../../../../docs/conformance_prettier.md)) —
while prettier relocates it to trail the object (`arr // c⏎[i]`).

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(before the index, inside the brackets) rather than hoisting it out to the whole
expression. This is the same in-place preservation the computed-key bracket
family uses (`[/* c */ key]`); prettier canonicalizes the position by relocating.
Optional chains (`obj?.[…]`) behave the same way. Without the break the `//`
would swallow the index and `]` — a content-loss bug this fixture also guards.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
