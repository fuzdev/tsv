# property_init_block_comment_break_hang_prettier_divergence

A block comment glued to a class property's `=` with the value authored on the next line
(`p = /* c */⏎value`), where the value is **too long to sit behind the comment** — the
width half of [property_init_block_comment_break](../property_init_block_comment_break_prettier_divergence/),
whose value fits and reflows onto the `=` line.

**tsv** keeps the comment after the `=`, gives it a line of its own, and hangs the value
below it at one indent — the declarator's and the assignment's answer to the same authoring
(`Printer::broke_after_operator_rhs_doc`), so the three `=` seams agree:

```
p =
	/* c */
	fn(…);
```

That form is `input.svelte`, and prettier keeps it too: written on a line of its own the
comment is a leading own-line comment, which prettier does not move. The divergence rides
the authored-break variant (`unformatted_ours_break.svelte`), which tsv normalizes to input
and prettier **relocates** to before the `=` (`p /* c */ =⏎value`).
That relocated form is pinned as `variant_relocated.svelte` — dual-stable, since tsv keeps a
comment wherever the author put it.

The curried chain (`q`) is the case the shared arm exists for here: left to the assignment
layout, the chain's own break-after-`=` fired under a run that had already broken, and the
output carried a blank line the author never wrote — a form that is its own fixed point.

## Reason

Per Comment Position Philosophy tsv preserves the comment's place after the `=` where
prettier moves it across the operator; the break after the comment is unforced, so tsv
re-decides it by width (see
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
