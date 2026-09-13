# retained_paren_shell_trailing_multiline_block_comment_prettier_divergence

A **multiline** block comment at the end of a redundant paren shell around a type
(`(B /* m1⏎m2 */)`), where the token after the shell may not follow a line break: a
conditional type's `extends`, an optional tuple element's `?`, an array type's `[]`, an
indexed access's `[`.

**tsv**: keeps the shell and opens it, the form a trailing `//` already takes there:

```
type A = (
	B /* m1
m2 */
) extends C
	? D
	: E;
```

**Prettier**: strips the shell and inlines the comment:

```
type A = B /* m1
m2 */ extends C
	? D
	: E;
```

## Reason

Each of those tokens is `[no LineTerminator here]` — TypeScript's
`parsePostfixTypeOrHigher` stops at `scanner.hasPrecedingLineBreak()` before a `[` or a
tuple `?`, and `parseType` before `extends`; acorn-typescript and tsv read the same rule —
and the comment's interior newline is a line terminator. Prettier's form therefore
**changes the program**: `B /* m1⏎m2 */ extends C ? D : E` is a syntax error, the tuple
`?` cannot follow, and at the top of a type alias `R /* m9⏎m10 */[]` parses as
`type Q = R;` followed by an empty array expression statement. A comment that spans a
line can only have reached this gap from inside a shell, and the shell has to survive
for the output to mean what the input did — a prettier bug at every one of these
positions (see
[conformance_prettier.md §Prettier bug index](../../../../../docs/conformance_prettier.md#prettier-bug-index)),
and the type-side twin of the
[non-null operand](../../expressions/non_null/operand_paren_multiline_block_comment_prettier_divergence/)
rule.

The retention is keyed on the **following token**, not on the comment alone: where
nothing restricted follows — a `;`, a union's `|` — the shell strips and the comment
trails inline, matching prettier (`type Y = Z /* m15⏎m16 */;`). The trailing `//` rule
([type_suffix_trailing_comment_union_member](../type_suffix_trailing_comment_union_member_prettier_divergence/))
retains everywhere but before a `|`/`&`, because a deferred `//` is lossy; a multiline
block inlines losslessly, so it keeps the shell only where inlining is not an option. A
**single-line** block strips everywhere, as before (the two controls at the end).

The retained shell opens at every position the way its `//` sibling does — the
top-level and nested conditional check type, the tuple element (plain and object), the
array element, the indexed-access object, and a mapped type's value — so the two
comment kinds share one form per position.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
