# computed_leading_line_comment_prettier_divergence

A line comment before a computed member-access index, inside the brackets
(`arr[// c⏎i]`).

**tsv** keeps the comment inside the brackets, breaking so the index and `]`
drop to their own lines — the `//` can't swallow the index. The broken lookup then
hangs below the `=`, as any value that breaks does. **Prettier** relocates the comment
to lead the whole expression, before the assignment RHS.

```
// tsv                     // prettier
const a =                  const a =
	arr[                        // c1
		// c1                   arr[i];
		i
	];
```

A comment written on the `[` line itself (`arr[ // c`, not its own line) is kept
trailing the `[` — the open-delimiter-trailing rule
([conformance_prettier_ts_comments.md §Object/array/block open-delimiter trailing](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)) —
while prettier relocates it to trail the object (`arr // c⏎[i]`).

The rule holds **inside a call chain** too (`arr.foo()[ // c4`, a numeric index). A comment
inside the brackets is the lookup's own — printed by the brackets around the index — so it
starts no chain group and breaks nothing before the `[`: the lookup stays in the group
prettier's member-chain grouping puts it in, and the open-delimiter-trailing rule then
applies unchanged. A `//` in the gap BEFORE the `[` is a chain gap comment and does start a
group; that half has its own fixture
([computed_numeric_index_pre_bracket_line_comment](../computed_numeric_index_pre_bracket_line_comment_prettier_divergence/)).
An **own-line** comment in the brackets of a chained lookup keeps the `[` on the object's
line, as prettier does — pinned, with the stripped-shell spelling that reaches the same
layout, by
[computed_index_shell_leading_line_comment](../computed_index_shell_leading_line_comment/).

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(before the index, inside the brackets) rather than hoisting it out to the whole
expression. This is the same in-place preservation the computed-key bracket
family uses (`[/* c */ key]`); prettier canonicalizes the position by relocating.
Optional chains (`obj?.[…]`) behave the same way. Without the break the `//`
would swallow the index and `]` — a content-loss bug this fixture also guards.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
