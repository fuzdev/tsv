# yield_open_paren_line_comment_prettier_divergence

A `//` line comment in the grouping `(`→argument gap of a `yield` / `yield*` keeps the line
the author wrote it on; prettier moves it to trail the `yield` keyword with the argument on
the next line, from **either** authoring. The yield analog of the return/throw
[keyword_open_paren_line_comment](../keyword_open_paren_line_comment_prettier_divergence/) —
the third restricted production.

tsv, from the two authorings:

```ts
yield ( // c
	a = b
);

yield (
	// c
	a = b
);
```

Prettier trails the comment on the keyword and drops the argument below — the same output
from both — and, since the comment no longer sits inside the parens, strips a now-redundant
grouping pair around a plain identifier:

```ts
yield // c
(a = b);

yield // c
a;
```

## Reason: prettier's output is ASI-UNSOUND, not merely relocated

`yield` is a restricted production — `yield [no LineTerminator here] AssignmentExpression`
(ECMA-262 §15.5) — so the parens are what keep the author's break legal. Strip them and put a
newline after the keyword and **the meaning changes**: ASI ends the statement at the newline,
the `yield` loses its argument, and the argument becomes a separate expression statement.
`yield // c⏎a;` parses as `YieldExpression { argument: null }` followed by `a;` under acorn
and tsv alike, and prettier's own second pass writes the split out — `yield; // c⏎a = b;`,
pinned in `audit_signature.txt`. So this is not a placement tsv declines to follow but an
output it cannot follow.

Prettier applies exactly this retention to `return` / `throw`, the other two restricted
productions, but scopes the check to `ReturnStatement` / `ThrowStatement`
(`parent-needs-parentheses.js`), so it never reaches `YieldExpression`. tsv uses one rule and
one gate for all three; the block-comment spelling of the same defect is
[yield_hanging_comment_parens](../../../expressions/await_yield/yield_hanging_comment_parens_prettier_divergence/).

Where the parens survive, which line the comment keeps is the ordinary preservation rule —
comment placement is a deliberate authoring choice. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
cataloged in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

The rule is independent of the argument kind — an assignment (`a = b`), a sequence (`a, b`),
a plain expression (`a`), and the delegating `yield*` all behave alike — and each appears in
both authorings.

- **`assign` / `seq` / `ident` / `delegate`** — the comment authored on the `(` line.
- **`assignOwnLine` / `seqOwnLine` / `identOwnLine` / `delegateOwnLine`** — the comment
  authored on its own line below the `(`. Two authorings, two tsv fixed points, one prettier
  form. This half is where `yield` parts from the return/throw sibling a second time: there
  prettier relocates the same-line comment to its own line *inside* the parens, so the
  own-line authoring matches and only the same-line one diverges.

`yield*` diverges in **layout only**: the `[no LineTerminator here]` sits between `yield` and
`*`, so a break after the `*` is legal and ASI cannot silently split it — a bare `yield*` is a
syntax error.

As in the return/throw sibling, tsv renders a sequence operand **bare** inside the hanging
parens (`( // c⏎a, b⏎)`) rather than double-wrapping it (`( // c⏎(a, b)⏎)`) — the hanging
parens are the grouping. A same-line **block** comment is not covered here.
