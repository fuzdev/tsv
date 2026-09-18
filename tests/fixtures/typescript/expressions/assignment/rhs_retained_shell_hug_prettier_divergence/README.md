# rhs_retained_shell_hug_prettier_divergence

The retained-shell rule
([expression_statement_paren_line_comment](../../../statements/expression_statement_paren_line_comment_prettier_divergence/))
at the assignment family, plus the **layout** that rule owes the operator.

A comment with no inline placement — a `//`, or one the author gave a line of its own —
written between the value and the `)` of its own grouping parens **retains** those parens.
Stripping them would defer the comment past the `;` onto a line that may already hold one,
and two `//`s sharing an output line merge into a single comment.

tsv (`input.svelte`):

```ts
x = (
	a.b // c1
);
```

The break the shell takes is the **comment's**, not a break point inside the value, so the
operator must not take one as well: the assignment **hugs** the shell. That is tsv's own
layout rule, not a reading of prettier — it is the shape the values that never reach a hang
already had (an object literal, a fluid call, a ternary) and the one `return` / `throw` /
`export default` / an arrow body / a bare expression statement give at this authoring, so
one rule now covers every value kind at every seam that hangs a value under an operator.

Prettier's **fixed point** is the fully collapsed line — no shell, no operator break:

```ts
x = a.b; // c1
```

so the divergence is the shell **retention** alone. (`output_prettier.svelte` is prettier's
first pass, which still hangs the value; `audit_signature.txt` pins the second, where the
hang collapses.) The flat authoring reaches tsv's form in one pass
(`unformatted_ours_flat.svelte`), where prettier takes two passes of its own to the same
fixed point — `audit_signature_flat.txt` pins that chain.

Cases: a member chain and a binary, which reach the hang by different arms; the two-segment
`x = y =` chain and a compound operator; a declarator, whose own layout cascade reads the
same rule; an own-line `//` and an own-line block, which open the shell for the same reason
a trailing `//` does. The first control is a trailing block **sharing the value's line** —
it does not end its line, so deferring it past the `;` is lossless and both formatters do
it.

## The sequence carve-out

A **sequence value is outside the retention**, and the last two cells are its controls. A
sequence supplies its own paren pair on every path, so the author's shell is never the pair
that would be kept — nothing is retained, nothing is forced open, and at a statement the
comment defers past the `;` exactly as prettier does it (a `for` header has no `;` of the
value's own to defer past, so there the comment stays inside the sequence's pair). The rule
is therefore not allowed to read such a value as a retained shell and change the operator's
layout for it.

The second control is where that matters: inside a **sequence container** the deferred run
rides the operand's own indentation (`// c11` under `// c10`), a layout the shell rule must
not reach for. Its flat authoring is the one the variant pins — it reaches that form in a
single pass, which is the property a shell reading would take away.

The `for`-header spellings of the same rule are pinned by
[init_assignment_prettier_ignore_paren_comment](../../../statements/for/init_assignment_prettier_ignore_paren_comment_prettier_divergence/).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value paren, retained shell: the operator HUGS it) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
