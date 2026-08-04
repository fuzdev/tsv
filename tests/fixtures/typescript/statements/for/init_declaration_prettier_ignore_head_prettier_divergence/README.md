# init_declaration_prettier_ignore_head_prettier_divergence

A `for` header's **init clause that is a declaration**, frozen by an own-line directive in the
`(`→init gap. The frozen slice is the declaration's own node span; the header's `;` is
parent-owned and stays outside it:

```ts
for (
	// prettier-ignore
	let i  =  0, j = 1;
	i < 10;
	i++
) {
	fn();
}
```

Prettier's slice **swallows the `;`** and then emits the header's own separator after it,
producing `let i  =  0, j = 1;;` — a four-clause `for` header that **does not parse**
(`js_parse_error: Unexpected token`). Prettier is stable on that output only because it never
re-reads it. tsv keeps the separator parent-owned, exactly as it does at every other frozen
list item, so the output stays valid.

The expression-init, test and update forms of the same rule match prettier — see the ordinary
sibling [clauses_prettier_ignore_head](../clauses_prettier_ignore_head/).

## Reason

A formatter must not emit code that fails to parse; ◆prettier_bug. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
