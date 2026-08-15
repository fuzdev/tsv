# head_body_block_comment_broke_after_relocated_prettier_divergence

The same authoring as
[head_body_block_comment_broke_after](../head_body_block_comment_broke_after_prettier_divergence/)
— a block comment trailing a header→body anchor with the body's `{` dropped to the next
line — at the five anchors where Prettier answers a different question entirely.

| anchor | tsv | prettier |
| --- | --- | --- |
| `try` / bare `catch` / `finally` | comment stays put, `{` keeps its line | comment moves **into the body** |
| `catch (e)` | the same | comment moves **into the `catch` parens** |
| `switch (a)` | the same | comment moves **into the discriminant parens** |

## Reason

There is no oracle here: Prettier relocates the comment out of the gap before any
line-keeping question is asked, so it never answers this authoring at all. The
relocation is the standing divergence — see §Comment relocation ("Try/catch/finally
head→body" and the `switch` entry) — and this fixture only records that tsv's own
answer at these anchors is the same one it gives everywhere else: the comment keeps
the position the author wrote it in, and a `{` the author dropped below it keeps the
line the author gave it.

Keeping the answer uniform is the point. Reading Prettier's relocated form as
guidance would mean each of these five anchors deciding the gap for itself, and the
comment's authored position is what would pay for it.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
