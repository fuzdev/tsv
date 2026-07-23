# yield_open_paren_line_comment_prettier_divergence

A `//` line comment the author placed on the grouping `(` line of a `yield` / `yield*`
argument stays trailing the `(`; prettier moves it to trail the `yield` keyword with the
argument on the next line. The yield analog of the return/throw
[keyword_open_paren_line_comment](../keyword_open_paren_line_comment_prettier_divergence/) —
the third restricted production.

tsv:

```ts
yield ( // c
	a = b
);
```

Prettier trails the comment on the keyword and drops the argument below (and, since the
comment no longer sits inside the parens, strips a now-redundant grouping pair around a
plain identifier — `yield // c⏎ a`):

```ts
yield // c
(a = b);
```

A leading comment before the argument forces the grouping parens to break either way (the
argument is a restricted-production operand — `yield [no LineTerminator here] operand`, so
without the parens ASI would end the `yield` and silently drop its argument). The only
question is where the comment lands: tsv preserves where the author wrote it (trailing the
`(`); prettier relocates it after the keyword. The rule is independent of the argument
kind — an assignment (`a = b`), a sequence (`a, b`), a plain expression (`a`), and the
delegating `yield*` all behave alike. As in the return/throw sibling, tsv renders a
sequence operand **bare** inside the hanging parens (`( // c⏎ a, b⏎)`) rather than
double-wrapping it (`( // c⏎ (a, b)⏎)`) — the hanging parens are the grouping.

The prettier form differs from the return/throw sibling (there prettier relocates the
comment to its **own line** *inside* the parens, keeping them), which is why `yield` is
covered by its own fixture rather than folded into `keyword_open_paren_line_comment`.

Only the **same-line `//`** authoring diverges; a comment on its **own line** below the
`(` keeps that line in both formatters, and a same-line **block** comment is not covered
here.

## Reason

Comment placement is a deliberate authoring choice and tsv preserves it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the return/throw sibling
[keyword_open_paren_line_comment](../keyword_open_paren_line_comment_prettier_divergence/).
