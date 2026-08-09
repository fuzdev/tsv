# param_after_comma_block_stranded_prettier_divergence

A type-parameter block comment **stranded** after the comma — the author left a newline
before the next parameter (`<T, /* c */⏎ U>`). tsv respects that newline and keeps the
comment where it was written (trailing the comma line); prettier attaches it to the
preceding parameter and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)       // prettier (relocate)
function fn1<                   function fn1<            function fn1<
	T, /* c */                      T, /* c */               T /* c */,
	U                               U                        U
>() {}                          >() {}                   >() {}
```

The `fn2` case pairs a **before-comma** block with a stranded after-comma block in the same
gap (`T /* c1 */, /* c2 */⏎ U`): each stays on its own side of the comma while prettier
relocates **both** before it, merging them onto one trailing run. The type-**argument**
list shares the printer and so answers identically.

A block **hugging** the next parameter (`<T, /* c */ U>`, no newline between them) leads
that parameter and both formatters agree — [param_after_comma_block](../param_after_comma_block/).
The stranded form is stable only once the parameters wrap; a list that fits collapses
inline, where the block hugs and both formatters agree again — that flat-layout normalization
is [end_of_line_block_comment](../end_of_line_block_comment_prettier_divergence/). The comma
pushed onto its own line with the comment (`<T⏎, /* c */⏎ U>`) is the same authoring one
notch further — the comma is re-emitted structure, outside every parameter span, so the
comment still sits after it — and takes the same normalization
(`unformatted_ours_comma_own_line`).

The type-parameter member of the `is_stranded_after_comma_block` family — see the
[tuple](../../tuple/element_after_comma_block_stranded_prettier_divergence/),
[function/constructor-type](../../function_type/param_after_comma_block_stranded_prettier_divergence/)
and value-level
[declarator](../../../declarations/variable/multiple/after_comma_block_stranded_prettier_divergence/)
siblings.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
