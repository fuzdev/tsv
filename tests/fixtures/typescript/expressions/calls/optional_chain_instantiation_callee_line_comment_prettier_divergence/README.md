# optional_chain_instantiation_callee_line_comment_prettier_divergence

A line comment between an instantiation callee's type arguments and an optional call's `?.`,
with arguments present and a chain continuing past the call. The comment trails the type
arguments in both formatters, and both keep the type arguments ahead of the `?.` (behind it,
`T?.<U>(a)` is a different program: the list moves from the instantiation to the call).

```ts
// tsv                          // prettier
T<U> // c1                      T<U> // c1
	?.(a).b;                      ?.(a).b;
```

## Reason

**Continuation indent.** A line comment runs to end of line, so the argument list cannot stay
on it. tsv drops it to a continuation line **indented one level**; prettier keeps it flush.
This is the callee→`?.` gap of
[callee_comment_optional_args](../callee_comment_optional_args_prettier_divergence/), reached
through an instantiation callee; the placement matches. See
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
