# comment_same_line_long_prettier_divergence

The width-forced half of
[comment_same_line](../comment_same_line_prettier_divergence/): a same-line **block** comment
in the attribute list trails the token the author wrote it after, exactly as a same-line `//`
does. Prettier relocates it to its own line.

While the list fits on one line the two formatters agree — a block comment is
self-delimiting, so nothing has to move. The rule only becomes visible once the list wraps,
which is why this needs its own `long` fixture:

```svelte
<!-- tsv -->                     <!-- prettier -->
<div /* c */                     <div
	data-attr1="value1"          	/* c */
	data-attr2="value2"          	data-attr1="value1"
>                                	data-attr2="value2"
                                 >

<div                             <div
	data-attr1="value1" /* c */  	data-attr1="value1"
	data-attr2="value2"          	/* c */
>                                	data-attr2="value2"
                                 >
```

Both gaps take the same rule the `//` fixture pins for the same two positions — the tag-name
gap and a between-attribute gap. The **trailing** gap (after the last attribute, before
`>`/`/>`) is not a divergence: there both formatters already keep the comment inline, which is
this fixture's fourth case and the discriminator showing the rule is about which gap the
comment sits in, not about the comment.

The 100/101 pair pins the boundary: at exactly 100 the list stays inline and the question does
not arise; one character more and it wraps.

Three stable forms, and the fixture holds all of them, because the position is authored:
`input.svelte` is the comment trailing its token, `divergent_variant_compact.svelte` is what
prettier reaches from a **flat** authoring (the comment glued to the head of the next
attribute's line), and `output_prettier.svelte` is the comment on a line of its own. Handed the
glued form tsv produces the third one — the comment now *starts* its line, so it reads as an
own-line comment and gets one, which is the same authoring-follows-placement rule seen from the
other side. Both formatters keep that third form.

## Reason

A same-line comment's position records what the author was annotating, and the token it was
written after is that record. tsv applies one rule to both comment kinds — applying prettier's
relocation to `/* */` while keeping the authored position for `//` would make the binding depend
on the comment's spelling. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [comment_same_line](../comment_same_line_prettier_divergence/) — the `//` half, where the comment forces the wrap itself
- [comment_trailing_same_line](../comment_trailing_same_line/) — the trailing gap, where both formatters keep the comment inline
- [comment](../comment/) — own-line comments in the list, preserved as-written by both
