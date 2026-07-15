# Divergence: `export`→`default` keyword-interior line comment (preserve + continuation indent)

A line comment inside the `export default` keyword (`export // c⏎default 1;`). tsv keeps it after
`export` and drops `default 1;` to a continuation line **indented one level** (the uniform
forced-continuation indent). Prettier **relocates** the comment past `default` and pulls the value
back flush.

```ts
// tsv (preserve + continuation indent)   // prettier (relocate, value flush)
export // c                               export default // c
	default 1;                            1;
```

The line form of [default_keyword_comment](../default_keyword_comment_prettier_divergence/) — same
gap, same reason to preserve; only the forced break differs. Consistent with every other
module-header line comment, which tsv indents uniformly.

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
