# Divergence: named-export-specifier rename `as` gaps (preserve, one indent level)

The export-side twin of
[imports/specifier_as_gap_line_comment](../../imports/specifier_as_gap_line_comment_prettier_divergence/).
A *line* comment in a renamed export specifier's `local`→`as` gap (`a //c⏎as b`) or `as`→`exported`
gap (`c as //c1⏎d`) stays where the author wrote it, with the tail continued at **one** indent
level; prettier relocates every such comment to lead the whole specifier.

```ts
// tsv (preserve)          // prettier (relocate to lead the specifier)
export {                   export {
	a //c                     //c
		as b                  a as b
};                         };
```

Import and export named specifiers share one renamed-specifier printer
(`build_renamed_specifier_doc`), which routes both `as` gaps through the shared header-gap
continuation helpers — so a line comment can't swallow the `as` or the renamed binding (the
before-`as` case was a **content-loss** bug). A same-line block comment trails inline in both.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
