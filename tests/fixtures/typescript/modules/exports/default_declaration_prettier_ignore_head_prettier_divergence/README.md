# default_declaration_prettier_ignore_head_prettier_divergence

The `export default`→value gap is the same head as the named
[`export`→declaration](../named_declaration_prettier_ignore_head_prettier_divergence/) one: an
own-line directive there freezes the value it precedes, whether that value is an expression, a
declaration, or a decorated class:

```ts
export default
	// prettier-ignore
	fn  (  aaa  );
```

Prettier **relocates** the directive flush onto the `export default` line and freezes anyway;
tsv keeps the author's line, since a head-trailing directive is inert under the placement floor
and the relocated form would lose the freeze on the second pass. **Both spellings** behave
alike — placement keys the freeze, not the comment's spelling.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation. See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).
