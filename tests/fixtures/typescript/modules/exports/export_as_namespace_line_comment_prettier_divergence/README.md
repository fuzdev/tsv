# Divergence: `export as namespace` line comments (preserve, one indent level)

A *line* comment in every gap of the three-word `export as namespace` keyword. tsv keeps each where
the author wrote it; prettier **relocates** all of them past the whole keyword and stacks them flush
before the name.

```ts
// tsv (preserve)          // prettier (relocate past the keyword, stack flush)
export // c1               export as namespace // c1
	as // c2               // c2
	namespace // c3        // c3
	Foo;                   Foo;
```

The line-comment counterpart of
[export_as_namespace_keyword_comment](../export_as_namespace_keyword_comment_prettier_divergence/)
(the block-comment form), and it pins the **rendering**, which is the reason it exists separately:
a header with several broken gaps continues at **one** indent level, not a staircase.

Each gap is emitted without its own `indent`, and the whole tail is wrapped once — so N line
comments indent the rest of the header once rather than N times. Nesting per gap would read as
`namespace` deeper than the `Foo` that follows it. One level is also what the
[uniform forced-continuation indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
already specifies for the single-gap case; this keeps a multi-gap header saying the same thing.

Only a three-word keyword can show the difference — `export as namespace` and the import-equals
header are the only ones — so a one- or two-gap header renders identically either way.

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
