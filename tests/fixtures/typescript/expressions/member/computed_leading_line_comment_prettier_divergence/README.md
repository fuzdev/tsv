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
([conformance_prettier_ts_comments.md §Object/array/block open-delimiter trailing](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)) —
while prettier relocates it to trail the object (`arr // c⏎[i]`).

The rule holds **inside a call chain** too, on the one accessor kind the chain
grouping otherwise glues into the preceding group: a numeric index (`arr.foo()[ // c4`).
A `//` anywhere in that node's gap — the pre-bracket region *or* the bracket interior —
makes the accessor start its own chain group (`group_chain_nodes`), and the
open-delimiter-trailing rule then applies unchanged. The pre-bracket half of that gap has
its own fixture ([computed_numeric_index_pre_bracket_line_comment](../computed_numeric_index_pre_bracket_line_comment_prettier_divergence/));
this case is the interior half. An **own-line** comment in the brackets of a *chained*
accessor is left uncovered on purpose — it breaks the chain, and prettier is
non-idempotent on its own output there, so pinning it means an `audit_signature` chain
claim that has nothing to do with this fixture's subject.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(before the index, inside the brackets) rather than hoisting it out to the whole
expression. This is the same in-place preservation the computed-key bracket
family uses (`[/* c */ key]`); prettier canonicalizes the position by relocating.
Optional chains (`obj?.[…]`) behave the same way. Without the break the `//`
would swallow the index and `]` — a content-loss bug this fixture also guards.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
