# Divergence: variable-binding before-`:` line comment indents the continuation

A line comment between a binding name and its `:` (`let x // c⏎: T`). tsv keeps the
comment after the name and drops the `: type` to a continuation line **indented one
level** (uniform forced-continuation indent). Prettier keeps the `: type` **flush**
with the name.

```ts
// tsv (continuation indents)   // prettier (flush)
let x // c                      let x // c
	: string;                   : string;
```

The **own-line** authoring (`let x⏎// c⏎: string`) pulls up to trail the name
and reaches input under tsv — one pass (`unformatted_ours_own_line.svelte`);
prettier reaches its flush form (`output_prettier.svelte`) in one pass too. The
own-line **block** sibling is
[binding_key_colon_own_line_block_comment](../binding_key_colon_own_line_block_comment_prettier_divergence/).

The variable-binding face of the cross-construct
[before-`:` continuation indent](../../../types/type_members/index_signature_key_colon_line_comment_prettier_divergence/)
(index signatures, property signatures, class properties, function parameters).
See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent and §Comment Position Philosophy.
