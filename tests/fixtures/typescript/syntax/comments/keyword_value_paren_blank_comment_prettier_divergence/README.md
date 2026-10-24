# keyword_value_paren_blank_comment_prettier_divergence

A block comment on a keyword's line, then a `//` behind it and an author blank line between the
two — at `await`, `new`, `case`, the prefix type operators (`keyof`, `readonly`) and a function
type's `=>`. At the type positions the comments sit inside the operand's redundant grouping
parens; both formatters strip them and pull the `//` onto the keyword's line behind the block
comment, so the blank, which sat between the two comments, has no line left between them; both
erase it. The value continues on the next line.

```ts
// tsv                               // prettier
type K = keyof /* c10 */ // c11      type K = keyof /* c10 */ // c11
	T;                                 T;
```

`unformatted_ours_paren_blank.svelte` carries the parenthesized type authorings and the
expression cells written without parens. At an expression position the parenthesized authoring
keeps its pair instead — the block is glued to the `(` and binds to the value — see
[glued_block_paren_line_comment](../glued_block_paren_line_comment_prettier_divergence/).

## Reason

**Continuation indent.** The `//` forces the value onto a continuation line; tsv indents it one
level, prettier keeps it flush. The comments and the erased blank match. See
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment).

**A second stripped paren with its own blank** (`unformatted_ours_nested_paren_blank.svelte`:
`keyof /* c19 */ (⏎⏎// c20⏎(⏎⏎// c21⏎T))`). The first blank erases as above. The second sits
inside the inner pair, above `// c21`, which keeps its own line: tsv keeps it, the stripped
paren shell's interior blank ahead of an own-line comment that the condition-shell entry of
§Comment relocation keeps
([condition_shell_blank_before_comment](../../../statements/if/condition_shell_blank_before_comment_prettier_divergence/)).
Prettier erases it, and keeps its own output
(`divergent_variant_nested_paren_blank.svelte`) stable, which tsv formats with the
continuation indent.
