# key_colon_blank_block_comment_prettier_divergence

An authored **blank line** after a single-line block comment in a head→`:` gap — a
parameter's or binding's name (or its `?` marker) and its type annotation, a member key
and its `:` — `a /* c */⏎⏎: number`.

**tsv** collapses the gap to the inline form both formatters hold stable when the comment
is authored inline, one pass at every site:

```ts
function f1(a /* c */ : number) {}
let v /* c */ : number;
```

**Prettier** keeps the blank — and the break it sits in — at eight of the ten sites here
(every parameter context, the `?`→`:` gap, a variable binding, a two-block run either
side of the blank), flush at the head's indent with each parameter list expanded around
it; it collapses the blank at a class property and a type-literal member,
where the comment attaches as the key's trailing comment. That blank-keeping form is
`prettier_variant_blank.svelte`, prettier's own fixed point, reached from the authoring in
`unformatted_ours_blank.svelte` through one unstable pass
(`prettier_intermediate_to_variant_blank.svelte`: the class property first prints
`p /* c */: number`, then re-spaces to `p /* c */ : number`). tsv normalizes both
prettier forms back to `input.svelte`.

## Reason

**Design choice — the blank yields with the break.** A single-line block forces nothing,
and a comment in this gap *trails the head* (the trailing-position corollary of
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)),
so the break after it is unforced layout tsv reflows — the own-line authoring
(`a⏎/* c */⏎: number`) already pulls up to the same inline form
([param_key_colon_own_line_block_comment](../../../declarations/function/param_key_colon_own_line_block_comment_prettier_divergence/)).
A blank line is a property of a line break: once the break is reflowed there are no two
lines left for it to separate. This is the pre-separator reading of the rule the value gaps
already follow (`A = /* c */⏎⏎1` → `A = /* c */ 1`, [§Authored breaks in value
position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)),
and it is one answer across all ten sites where prettier's is split — its class property
and type-literal member collapse the same blank, so prettier is not the tiebreak.

The **line**-comment sibling is the other side of the same sentence: a `//` forces the
break, so an authored blank after it survives at every one of these sites, inside the
uniform forced-continuation indent
([continuation_blank_between_comments](../continuation_blank_between_comments_prettier_divergence/)).
An author who wants the blank writes that form.

See [conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position);
cataloged in [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
