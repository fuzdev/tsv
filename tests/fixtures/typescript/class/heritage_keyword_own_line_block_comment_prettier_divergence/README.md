# heritage_keyword_own_line_block_comment_prettier_divergence

An **own-line block** comment after a heritage keyword (`extends`/`implements`),
before the first heritage type, with the type authored on a later line
(`class A extends⏎/* c */⏎B`).

**tsv** keeps the comment where the author wrote it — on its own line after the
keyword, with the heritage type on the next line:

```
class P extends
	/* c */
	Q {}
```

**Prettier** relocates the comment up *before* the keyword, breaking the keyword
+ type onto the next line:

```
class P
	/* c */
	extends Q {}
```

This is the own-line-block sibling of the
[line-comment form](../extends_keyword_line_comment_prettier_divergence/)
(prettier floats either up before the keyword); tsv preserves the author's
keyword-associated placement, the same as the `as`/`satisfies` keyword→type gap
([as_satisfies_value_own_line_block_comment](../../expressions/as_satisfies_value_own_line_block_comment_prettier_divergence/)).
A **same-line** block comment glued to the type (`class A extends /* c */ B`)
stays inline in both formatters and is *not* a divergence (the regular
[extends_keyword_comment](../extends_keyword_comment/) fixture) — only the
own-line authoring diverges. Covers class `extends`, class `implements`, and
interface `extends`.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
