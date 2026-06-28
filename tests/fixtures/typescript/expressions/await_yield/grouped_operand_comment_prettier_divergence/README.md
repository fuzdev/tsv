# Await grouped-operand comment divergence

When a parenthesized operand of `await` carries a trailing (or leading)
comment inside its **required** parens (`await (x + y /* c */)`), tsv keeps
the comment where the author wrote it — inside the parens. The parens are
required: `await` binds tighter than `+`/`||`/`? :`, so `await x + y` means
`(await x) + y`. Both formatters keep the parens; only the comment position
differs.

Prettier 3.9 relocates the comment **outside** the parens, and is
non-idempotent doing so:

- Trailing: `await (x + y) /* c */` (pass 1), then `await (x + y); /* c */`
  (pass 2, floats the comment past `;`) — see `audit_signature.txt`
- Leading: `await /* c */ (x + y)` (comment moved before the parens)

`yield` is **not** here: `yield` binds looser than `+`, so its parens are
redundant and both formatters strip them (`yield x + y /* c */`) — a match,
not a divergence.

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Await grouped operand) and §Comment Position Philosophy.
