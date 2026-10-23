# keyword_value_paren_blank_comment_prettier_divergence

A block comment on a keyword's line, then the value's redundant grouping parens with an author
blank line and a `//` inside them — at `await`, `new`, `case`, the prefix type operators
(`keyof`, `readonly`) and a function type's `=>`. Both formatters strip the parens and pull the
`//` onto the keyword's line behind the block comment, so the blank, which sat between the two
comments, has no line left between them; both erase it. The value continues on the next line.

```ts
// tsv                          // prettier
await /* c1 */ // c2            await /* c1 */ // c2
	a;                            a;
```

`unformatted_ours_paren_blank.svelte` carries the parenthesized authorings.

## Reason

**Continuation indent.** The `//` forces the value onto a continuation line; tsv indents it one
level, prettier keeps it flush. The comments and the erased blank match. See
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment).

**A second stripped paren with its own blank** (`unformatted_ours_nested_paren_blank.svelte`:
`await /* c5 */ (⏎⏎// c6⏎(⏎⏎// c7⏎b))`). The first blank erases as above. The second sits
inside the inner pair, above `// c7`, which keeps its own line: tsv keeps it, the stripped
paren shell's interior blank ahead of an own-line comment that the condition-shell entry of
§Comment relocation keeps
([condition_shell_blank_before_comment](../../../statements/if/condition_shell_blank_before_comment_prettier_divergence/)).
Prettier erases it, and keeps its own output
(`divergent_variant_nested_paren_blank.svelte`) stable, which tsv formats with the
continuation indent.
