# value_leading_comment_blank_width_prettier_divergence

A block comment (or a glued run) the author broke after, leading a value that breaks by
**width** alone, with an authored **blank line** between the comment and the value.

**tsv** reflows the blank away and lands on the same fixed point as the single-newline
authoring — the comment on its own line under the operator, the value re-fitting below:

```
const u =
	/* c1 */
	aaa + bbb + ccc;
```

**Prettier** keeps the blank:

```
const u =
	/* c1 */

	aaa + bbb + ccc;
```

`unformatted_comment_on_head.svelte` is the single-newline authoring, which both formatters
normalize to input; `unformatted_ours_blank.svelte` adds the blank, which only tsv normalizes
to input. The type-alias union is the same rule at a seam whose union arm already reached it.

The blank may also sit **between** two comments, the last glued to the value
(`= /* c1 */⏎⏎/* c2 */ v`): the break is then the first comment's, and it yields the same way —
own-line when the value breaks by width (`q`), one line when the value fits (`t`, and the type
alias `U`, where nothing owns the glued comment and the run carries it itself).

Prettier's blank-keeping output is pinned as `divergent_variant_blank.svelte`, and tsv does
not return it to input: once a lone comment sits on its own line its break is forced, so
tsv keeps that blank (as prettier does), while the glued run (`/* c1 */ /* c2 */`) given its
own line still owns none of it, so that blank yields. Three stable forms, which is what a
`divergent_variant_*` records.

## Reason

**Design choice.** The break after the comment is unforced — a block comment does not run to
end-of-line, and nothing in the value forces a hard break — so it is width layout, and a blank
line is a property of a line break: it yields with the break, exactly as the fitting-value
authoring (`A = /* c */⏎⏎1` → `A = /* c */ 1`) does. A width-broken value does not change that;
it only decides that the break renders. See
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
