# keyword_line_comment_prettier_divergence

A **line** comment in the `infer`→inferred-name gap, the name on a later line —
trailing `infer` (`infer // a⏎R`) or on its own line (`infer⏎// b⏎R`).

**tsv** keeps the comment where the author wrote it and hangs the name indented
one level (the shared keyword→value layout, `append_keyword_value_line_comments`):

```
type A = X extends infer // a
	R
	? R
	: never;
```

**Prettier** keeps the comment trailing `infer` but drops the name **flush** at the
conditional's base indent (`infer // a⏎R`), and pulls an own-line comment up onto
the `infer` line.

`infer` hangs the name like every other forced keyword→value continuation — the
prefix type-operator and type-parameter constraint/default gaps — rather than the
flush layout prettier uses here; the divergence is the one-level indent (and, for
the own-line form, keeping the comment on its own line). The block-comment sibling
is [keyword_own_line_block_comment](../keyword_own_line_block_comment_prettier_divergence/).
Per [§How tsv treats keyword→value block
comments](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation),
tsv keeps the comment associated with `infer` and indents the continuation
uniformly.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
