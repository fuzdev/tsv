# expression_statement_prettier_ignore_head_prettier_divergence

An own-line directive after the `(` of an expression statement whose parens are **required** —
an object / class / function expression at statement start — freezes the expression. The parens
are the printer's, so they stay outside the frozen slice and the directive keeps its place
inside them:

```js
(
	// prettier-ignore
	{ aaa:  1 }
);
```

Prettier hoists the directive out before the `(` and glues the frozen slice back inside parens
on one line (`// prettier-ignore⏎({ aaa:  1 });`), the same relocation it applies to an ordinary
comment here
([expression_statement_paren_kept_comment](../expression_statement_paren_kept_comment_prettier_divergence/)).
When the parens are **redundant** tsv drops them and the directive leads the statement, matching
prettier ([paren_dropped head](../expression_statement_paren_dropped_prettier_ignore_head/)).

The last case is the one where those two regimes meet: the parens are redundant around the
*printed* expression but the frozen slice's leftmost token (`{`) would reparse as a block, and a
verbatim slice has no interior for the printer to wrap. tsv therefore keeps the author's parens
around the whole slice. Prettier drops them and emits `{ bbb:  2 }.ccc;`, which does not reparse.

## Reason

Comment position is authorship signal, and a shell a verbatim slice needs must go around the
whole slice; ◆comment_preservation, ◆prettier_bug. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
