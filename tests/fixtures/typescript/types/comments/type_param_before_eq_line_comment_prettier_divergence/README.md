# Divergence: type-parameter before-`=` default line comment (preserve, one indent level)

A line comment before a type parameter default's `=` (`<T extends A // c⏎= B>`, or
default-only `<U // c⏎= B>`). A `//` runs to end-of-line, so the tail cannot stay
on the comment's line — inlining would swallow it (`T extends A // c = B`: the
default becomes comment text). tsv keeps the comment where the author wrote it and
drops the `= B` tail to a continuation line at **one** indent level (uniform
forced-continuation indent). Prettier instead **relocates** the comment past `=`
to lead the default value.

```ts
// tsv (preserve + continuation)   // prettier (relocate past `=`)
function fn<                       function fn<
	T extends A // c                  T extends A = // c
		= B                              B
>(): void {}                       >(): void {}
```

The type-parameter face of the cross-construct before-`=` initializer line
comment — the same preserve + continuation rule as variable declarators
([declarator_before_eq_line_comment](../../../declarations/variable/declarator_before_eq_line_comment_prettier_divergence/)),
enum members, and class properties.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
