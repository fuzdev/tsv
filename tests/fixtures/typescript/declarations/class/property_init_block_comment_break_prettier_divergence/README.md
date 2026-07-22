# property_init_block_comment_break_prettier_divergence

A block comment glued to a class property's `=`, with the value authored on the next line.

**tsv** keeps the comment where the author glued it and reflows the value onto the
operator's line — the fixed point both formatters agree on:

```
p = /* c */ 1;
```

**Prettier** reflows too, but relocates the comment to before the `=`:

```
p /* c */ = 1;
```

So `input.svelte` itself is byte-identical in both formatters; the divergence shows only
on the authored-break variant (`unformatted_ours_break.svelte`), which tsv normalizes back
to input and prettier normalizes to its relocated form.

That relocated form is pinned as `variant_relocated.svelte` — **dual-stable**: once the
comment sits there, *both* formatters keep it, because tsv preserves a comment wherever the
author put it and prettier is already at its own fixed point. So it is a `variant_*`, not a
`prettier_variant_*` (which would mean tsv normalizes it back to input). Pinning it is what
makes the round trip explicit: the authored-break form is the only unstable one, and the two
formatters carry it to two different stable places.

The **own-line** authoring (`p =⏎/* c */⏎1`) is pinned separately as
`variant_own_line.svelte` — also dual-stable, and a *different* fixed point from input: with a
newline before it the comment leads its own line, so nothing glues it to the `=` and both
formatters break after the `=` and keep it there. Carrying the whole authoring axis (glued /
own-line) is deliberate: the two take different paths through the leading-comment run, so a
change that only exercises the glued one can silently break the other.

The **blank** authoring (`p = /* c */⏎⏎1`, `unformatted_ours_blank.svelte`) is the third point on
that axis. The blank yields with the break — it sits inside a break already judged unforced, and
a blank line is a property of a line break — so tsv normalizes it to input, same as the plain
newline; prettier relocates to `variant_relocated.svelte` as it does for the newline authoring.
See
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).

## Reason

**Design choice.** The break is unforced — a block comment does not run to end-of-line, so
nothing pushes the value off the operator's line — and tsv reflows an unforced break at
every value position (see
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).
Where tsv and prettier part is the comment's **position**: tsv preserves it, prettier moves
it, which is the standing
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
divergence.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
