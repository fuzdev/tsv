# Divergence: before-`:` line comment indents the `: type` continuation

A line comment between an index-signature key and its `:`
(`[key // c⏎: type]`). The `//` forces the bracket to break; tsv keeps the comment
after the key and drops the `: type` to a continuation line **indented one level**
so it reads as part of the `key: type` parameter (uniform forced-continuation
indent). Prettier keeps the `: type` flush with the key.

```ts
// tsv (continuation indents one level)   // prettier (flush)
[                                         [
	key // c                              	key // c
		: string                          	: string
]: number;                                ]: number;
```

`interface B` shows two comments in the gap: each keeps its own line and the
`: type` follows, all on the continuation indent.

This applies the same indent tsv already gives every other forced continuation —
the after-`:` type (`x: // c⏎T`), the `]`→value-`:` gap, prefix operators — now
also to the **before-`:`** comment, uniformly across the constructs that have it:
index signatures (here), property signatures and class properties — key→`:`
([key_colon_line_comment](../../../syntax/comments/key_colon_line_comment_prettier_divergence/))
and `?`→`:`
([optional_marker_line_comment](../../../syntax/comments/optional_marker_line_comment_prettier_divergence/)),
variable bindings
([binding_key_colon_line_comment](../../../declarations/variable/binding_key_colon_line_comment_prettier_divergence/)),
and function parameters
([param_key_colon_line_comment](../../../declarations/function/param_key_colon_line_comment_prettier_divergence/)).
Prettier keeps the before-`:` continuation flush (and for property signatures /
class properties relocates the comment to end-of-line). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent and §Comment Position Philosophy.
