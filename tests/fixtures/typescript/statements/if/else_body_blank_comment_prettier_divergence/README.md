# else_body_blank_comment_prettier_divergence

An author blank line between the `else`→body gap's comment run and the body.

Prettier keeps the blank for every body kind. tsv keeps it too — except where the
body opens a `{`, which it must not sit below:

| body | tsv | prettier |
| --- | --- | --- |
| non-block (`fn();`) | blank kept | blank kept |
| else-if | blank kept | blank kept |
| else-if, comment already trailing `else` | blank kept | blank kept |
| **block** (`{ … }`) | **blank dropped** | blank kept |

The third row is the same answer reached through a different layout. A comment
trailing `else` is a trailing comment of the keyword, printed outside the clause, so
prettier keeps that run flush with `else` (`if/else_consecutive_comment`) where a
run owning its own lines takes the continuation indent. Flush or indented, the body
opens no `{`, so the blank survives either way — the layout and the blank are
independent questions about the same gap.

## Reason

The drop is the brace rule, not a run rule: a body block's `{` never sits below a
blank, the same sanction the `while (a)⏎// c⏎⏎{` header→body gap carries
([while/line_before_body_comment](../../while/line_before_body_comment_prettier_divergence/)).
The licence stops where its argument stops — a brace-less body has nothing to
protect, so the blank survives there as it does between two comments in the same
run.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §"No blank above a body block's `{`".
