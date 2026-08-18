# cast_target_leading_line_comment_prettier_divergence

A line comment in the leading gap of the required pair a type-assertion assignment
target prints — between the `(` and the target (`( // c⏎x as T) = 1;`).

**tsv**: keeps the comment inside the pair, on its authored line — glued to the `(`
it stays on the `(` line, own-line it keeps its own line — and the shell expands:

```
( // c1
	x as T
) = 1;
```

**Prettier**: hoists the whole run out in front of the pair, re-binding it from the
target to the whole statement:

```
// c1
(x as T) = 1;
```

At a destructuring default (`{ a: ( // c⏎b as T) = 1 }`) prettier instead hangs the
comment in the property's key→value gap; tsv's answer is unchanged.

## Reason

The pair is **required** — a bare `x as T = 1;` is a parse error — so it prints
whatever the comment does, and a comment inside it comments the *target*, not the
statement; hoisting re-binds it. tsv keeps a comment inside a pair the position
requires everywhere the family reaches it: the non-null grouped operand
(`(/* b */ x + y)!`), the update operand's shell
([update_postfix_paren_line_comment](../../unary/update_postfix_paren_line_comment_prettier_divergence/) —
"retaining on either gap is one rule for one shell"), and the optional tuple
element's pair. A target whose pair is **redundant** strips it and the run leads
the statement, matching prettier — the plain
[redundant_target_paren_comment](../redundant_target_paren_comment/) is that
control.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
