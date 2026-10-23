# Divergence: `export`→`=` line comment (preserve + continuation indent)

A line comment between `export` and the `=` of an export-assignment (`export // c⏎= value;`).
tsv keeps it after `export` and drops `= value;` to a continuation line **indented one level**
(the uniform forced-continuation indent). Prettier **relocates** the comment past the `=` and
pulls the value back flush.

```ts
// tsv (preserve + continuation indent)   // prettier (relocate, value flush)
export // c                               export = // c
	= value;                              value;
```

The line form of [export_equals_keyword_comment](../export_equals_keyword_comment_prettier_divergence/)
— same gap, same reason to preserve; only the forced break differs. The `export default` twin is
[default_keyword_line_comment](../default_keyword_line_comment_prettier_divergence/).

The comment's text holds an `=` of its own, which is not the export assignment's.
`unformatted_ours_glued.svelte` writes the `=` glued to the value (`=value;`); it keeps its comment
and normalizes to input.

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword),
[§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation),
and [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
