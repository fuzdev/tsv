# Divergence: named-export-specifier `type` modifier gap (preserve, one indent level)

The export-side twin of
[imports/specifier_type_gap_line_comment](../../imports/specifier_type_gap_line_comment_prettier_divergence/).
A *line* comment between a specifier's own `type` modifier and the name it modifies
(`{ type //c⏎A }`) stays where the author wrote it, with the tail continued at **one** indent
level; prettier relocates every such comment to lead the whole specifier.

```ts
// tsv (preserve)          // prettier (relocate to lead the specifier)
export {                   export {
	type //c                  //c
		A                     type A
};                         };
```

Import and export named specifiers share one renamed-specifier printer
(`build_renamed_specifier_doc`), so the `type`→name gap routes through the same header-gap
continuation helpers as the `as` gaps beside it — a `//` can't swallow the name (a gap
with no emitter **drops** every comment in it). A same-line block
comment trails inline in both.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
