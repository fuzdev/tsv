# head_body_blank_comment_prettier_divergence

An author blank line between the `do`→body gap's comment run and the body.

| body | tsv | prettier |
| --- | --- | --- |
| **block** (`{ … }`) | **blank dropped** | blank kept |
| non-block (`fn();`) | blank kept | blank kept |

## Reason

The drop is the brace rule, not a `do` rule: a body block's `{` never sits below a
blank, the same sanction the `while (a)⏎// c⏎⏎{` header→body gap carries
([while/line_before_body_comment](../../while/line_before_body_comment_prettier_divergence/))
and the `else`→body gap makes visible against its own non-block arm
([if/else_body_blank_comment](../../if/else_body_blank_comment_prettier_divergence/)).
`do` is the one anchor of that rule that is neither a `)` nor `else`, so it is pinned
here rather than inferred.

The non-block row is a control, not a divergence: it reaches the same answer through
the leading-run emitter (`Printer::push_indented_header_to_body_gap`) rather than
through the gap's tail, and both formatters keep the blank there.

`prettier_variant_blank.svelte` pins prettier's stable form — the block case with the
blank — which tsv normalizes back to `input`.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §"No blank above a body block's `{`".
