# in_of_left_prettier_ignore_head_prettier_divergence

A for-in / for-of header's **left clause**, frozen by an own-line directive in the `(`→left
gap. The slice is the left's own node span, so the `in` / `of` keyword and the iterable stay
parent-owned and normalize:

```ts
for (
	// prettier-ignore
	const  xxx
		of
		yyy
) {
	fn();
}
```

Prettier diverges twice over. It **relocates** the directive flush against the `(`
(`for (// prettier-ignore⏎const  xxx …`), a placement tsv never writes; and where the left is a
**declaration** its frozen slice is followed by a `;`, producing `for (const  xxx; of yyy)` —
a header that **does not parse** (`js_parse_error`). Prettier is stable on that output only
because it never re-reads it. A **pattern** left (`[ aaa , bbb ]`) freezes correctly for both
tools, so there only the relocation differs.

The header's own broken layout — binding, keyword and iterable each on their own line — is the
standing for-in/for-of line-comment layout, shared with
[of_in_keyword_binding_line_comment](../of_in_keyword_binding_line_comment_prettier_divergence/).
**Both spellings** hold the header open, unlike an ordinary block comment, which still rides
inline: the directive's own line is what makes it honored, so the inline layout would glue it
to the following token and the freeze would be lost on the second pass.

That placement rule holds in the keyword→binding gap too, where nothing freezes today — the
last case pins the layout. (Prettier freezes the binding there, but only by relocating the
directive up beside `const`, the placement tsv treats as inert.)

The C-style `for` clauses are the ordinary sibling
[clauses_prettier_ignore_head](../clauses_prettier_ignore_head/); the declaration-init form has
the same prettier separator bug, pinned in
[init_declaration_prettier_ignore_head](../init_declaration_prettier_ignore_head_prettier_divergence/).

## Reason

tsv never relocates a directive — the placement the author wrote is the placement that decides
the freeze — and a formatter must not emit code that fails to parse; ◆comment_preservation
◆prettier_bug. See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).
