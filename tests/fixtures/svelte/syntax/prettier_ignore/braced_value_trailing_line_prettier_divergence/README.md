# braced_value_trailing_line_prettier_divergence

A frozen `{…}` value whose trailing run ends in a **line** comment. The comment's own break
already ends the content, so the closing `}` reuses it — the value's block form must not add
a second one, which would leave a blank line above the `}`.

tsv:

```svelte
<p>
	{
		// prettier-ignore
		aaa  +  bbb // c
	}
</p>
```

Prettier freezes nothing here and strips the comments (`{aaa + bbb}`), the content loss
[expr_trailing_line](../../comments/expr_trailing_line_prettier_divergence/) and
[braced_value_own_line](../braced_value_own_line_prettier_divergence/) already catalog
between them; this fixture's subject is the closer.

The rule is the value's, not the head shape's: the prefixed heads answer it in
`build_prefixed_head_doc` and the block heads in their own assembler, and the unprefixed
`{…}` values — an expression tag, an attribute value, a `bind:` value — owe the identical
shape. The `{@html}` case sits in the same input so the parity is the assertion rather than
a claim in prose.

## Reason

The freeze decides what the value's *bytes* are; it never decides how the delimiter around
them is placed, so a frozen value's closer follows the same rule as an ordinary one. See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).

## Related

- [braced_value_own_line](../braced_value_own_line_prettier_divergence/) — the directive's own line in the same value gap
- [expr_trailing_run](../../comments/expr_trailing_run_prettier_divergence/) — the same closer rule, unfrozen, for a run of two
