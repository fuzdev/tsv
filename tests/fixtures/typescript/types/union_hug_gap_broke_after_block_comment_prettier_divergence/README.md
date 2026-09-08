# union_hug_gap_broke_after_block_comment_prettier_divergence

A single-line block comment the author **broke after**, ahead of a should-hug union
with an expanded member, at the value seams (`let a: /* c */⏎{ … } | null`, a
property `:`, a function type's `=>`, a type alias `=`).

Both formatters bind such a comment to the **union**, not its first member — only a
block glued to the member declines the hug
([union_hug_gap_block_comment](../union_hug_gap_block_comment/)) — so the union keeps
hugging and the object member owns its expansion. The divergence is where the
comment lands:

- **tsv** hangs the union after the operator with the comment on its own line and the
  hugged union below it, one form at every value seam:

  ```ts
  let a:
  	/* c */
  	{
  		aaaa: Aaaaaaaaa;
  	} | null;
  ```

- **Prettier** prints that form at the alias `=` (its break-after-`=` layout for a
  leading own-line comment, where the two agree) but at `:` and `=>` keeps the
  comment trailing the operator and drops the type **flush** at the statement's own
  indent (`let a: /* c */⏎{⏎\taaaa: Aaaaaaaaa;⏎} | null;`) — the hardline its
  leading-comment printer emits after a block followed by a newline escapes the
  annotation's indent.

tsv's uniform hang is the [§Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
family: an own-line comment forces the value below the operator, indented one
level, at every operator alike. `unformatted_ours_broke_after.svelte` is the
broke-after authoring itself, which tsv normalizes to `input.svelte` in one pass
(prettier keeps it as `output_prettier.svelte`). A short member collapses the
comment back onto the operator in both formatters (the last case).

**An own-line block after an authored leading pipe** (`: |⏎/* c */⏎{ … } | null`,
`unformatted_ours_pipe_own_line.svelte`; `: | /* c */⏎{ … }`,
`unformatted_ours_pipe_glued_own_line.svelte`) is the same case for tsv: a block
separated from the member by a newline binds to the union whether or not a pipe
was authored, so both normalize to `input.svelte`. Prettier's union span starts at
the authored pipe, so the comment sits inside the union and binds to the first
member instead: it declines the hug and prints the comment own-line after the
pipe with the member aligned beneath it — a stable form of its own
(`prettier_variant_pipe_own_line.svelte`, which tsv normalizes back to input):

```ts
let a:
	| /* c */
	  {
			aaaa: Aaaaaaaaa;
	  }
	| null;
```

A block **glued** to the pipe (`/* c */ | {`, `/* c */ |⏎{`) is not this case:
both formatters bind it to the member
([union_hug_gap_block_comment_leading_pipe](../union_hug_gap_block_comment_leading_pipe/)).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
