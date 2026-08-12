# callback_multiline_param_comment_prettier_divergence

A multiline block comment in a callback's parameter list carries a real line break, so the
parameter list cannot stay on one line. Both formatters break it and both keep the comment
exactly where it was written — they disagree only about **which group pays for the break**.
Prettier expands the enclosing call as well, dropping the callback to its own indented line;
tsv breaks only the signature and keeps the call hugging its callback, the narrower break.

```
// tsv                            // prettier
fn((                              fn(
	a /* multi                      (
line */,                              a /* multi
	b                             line */,
) => ({ x: a }));                     b
                                    ) => ({ x: a })
                                  );
```

Hugging the callback is the whole point of the expand-last-argument layout, and prettier's
extra level buys nothing here: the comment's own continuation line is verbatim author text in
both outputs, so it lands at the same column either way and the added indent only pushes the
signature away from the callee it belongs to. Same principle as the member-chain half of
[callee_line_comment_empty_args](../callee_line_comment_empty_args_prettier_divergence/) — a
comment makes prettier expand the enclosing construct, while tsv breaks only where the comment
requires it.

The **call-body** shapes (`c1`, `c2`) are the exception, and they converge with prettier: that
hug renders the whole signature on one line, which this comment makes impossible, so it is
refused outright and the call expands. Its line-comment sibling is
[arrow_callback_trailing_param_comment](../arrow_callback_trailing_param_comment/) and
[chain_arrow_callback_trailing_param_comment](../chain_arrow_callback_trailing_param_comment/),
where every shape converges. They are pinned here as the boundary of this rule: the hug tsv
keeps is only the one it can honor.

A **single-line** block comment forces nothing and hugs everywhere, in both formatters — the
control for this rule lives in
[arrow_callback_param_comment](../arrow_callback_param_comment/).

Covers: call body and object body, block body plain and in a member chain, and a `function`
expression argument. The flat authoring normalizes to `input` under tsv only
(`unformatted_ours_flat`) — prettier expands it to `output_prettier`.

Reason: `◆design_choice`. See
[conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
