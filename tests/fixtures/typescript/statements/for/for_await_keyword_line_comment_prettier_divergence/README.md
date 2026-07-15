# for_await_keyword_line_comment_prettier_divergence

A **line** comment in a `for await` header — the line-comment counterpart of the
block-comment `for_await_keyword_comment_prettier_divergence`. Both `for await`
keyword gaps are covered: `for`→`await` (`for // c⏎await`) and `await`→`(`
(`for await // c⏎(`).

- **tsv**: keeps the comment trailing the preceding keyword, with the next token
  (`await`, then `(`) broken to the next line so the `//` can't swallow it.
- **prettier**: relocates the comment *inside* the condition parens, before the
  binding (`for await (// c⏎const a of x)`).

Emitting the comment inline (the previous behavior in the `for`→`await` gap) let
the `//` run to end-of-line and swallow `await (…)` — non-idempotent content loss
that failed to reparse. Per comment placement policy, the authored position is
preserved. The uniform keyword→`(` line rule is
[keyword_paren_line_comment](../../../syntax/comments/keyword_paren_line_comment_prettier_divergence/);
this fixture pins the two extra keyword gaps `for await` adds.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-relocation) §Comment relocation.
