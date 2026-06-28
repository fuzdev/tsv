# Await grouped-operand comment divergence

When a parenthesized operand of `await` carries a trailing (or leading)
comment inside its **required** parens (`await (x + y /* c */)`), tsv keeps
the comment where the author wrote it — inside the parens. The parens are
required: `await` binds tighter than `+`/`||`/`? :`, so `await x + y` means
`(await x) + y`. Both formatters keep the parens; only the comment position
differs.

Prettier relocates the comment **outside** the parens, and is non-idempotent
doing so: a trailing comment first moves after the parens (`await (x + y) /* c
*/`) and then, on a second pass, floats past the `;` (`await (x + y); /* c */`
— see `audit_signature.txt`); a leading comment moves before the parens
(`await /* c */ (x + y)`).

`yield` is **not** here: `yield` binds looser than `+`, so its parens are
redundant and both formatters strip them (`yield x + y /* c */`) — a match,
not a divergence.

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Await grouped operand) and §Comment Position Philosophy.
