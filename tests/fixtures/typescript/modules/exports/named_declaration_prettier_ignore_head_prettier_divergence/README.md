# named_declaration_prettier_ignore_head_prettier_divergence

An own-line directive in an `export`→declaration gap freezes the declaration — the node that
follows the directive, over its own span. The `export` keyword is parent-owned and stays
outside the slice; decorators written *after* `export` belong to the declaration, so they
ride inside it:

```ts
export
	// prettier-ignore
	const  aaa  =  1;
```

Prettier **relocates** the directive flush onto the `export` line (`export // prettier-ignore`)
and freezes anyway. tsv keeps the line the author gave it and indents the continuation, the
same uniform header rule an ordinary comment takes at this gap
([export_declaration_line_comment](../../../syntax/comments/export_declaration_line_comment_prettier_divergence/)) —
and here it is load-bearing rather than cosmetic: a head-trailing directive is inert under the
placement floor, so following the relocation would lose the freeze on tsv's own second pass.

**Both spellings** behave alike — placement keys the freeze, not the comment's spelling.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation. See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).
