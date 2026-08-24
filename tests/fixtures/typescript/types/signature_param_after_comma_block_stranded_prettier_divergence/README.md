# signature_param_after_comma_block_stranded_prettier_divergence

The type-**member signature** member of the `is_stranded_after_comma_block` family — method
signatures, construct signatures, and bodyless (`declare` / overload) function signatures,
which share one parameter printer (`build_signature_params_doc`) with each other and with
nothing else.

A block comment the author **stranded** after the comma (a newline before the next param)
keeps the comma's line; prettier attaches it to the preceding param and relocates it
**before** the comma. Same single rule as the value-level
[param](../../syntax/comments/param_after_comma_block_stranded_prettier_divergence/) and the
function/constructor-**type**
[param](../function_type/param_after_comma_block_stranded_prettier_divergence/) siblings —
the stranded form is stable only once the params wrap, since a list that fits collapses and
the block hugs the next param in both formatters.

A **third** answer, neither of the other two, is emitting
the whole inter-param gap as the previous param's trailing run, so the comment comes
out *before* the comma — the very relocation this family declines to make. The
seam is now the shared one ([`Printer::push_item_trailing_run`] for the claimed trailing
prefix, [`Printer::push_stranded_after_comma_blocks`] for the stranded remainder, and the
shared leading emitter for what neither claimed), so the three parameter families answer it
identically. Its hugging counterpart is
[signature_param_after_comma_block](../signature_param_after_comma_block/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
