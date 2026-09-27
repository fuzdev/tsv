# range_section_seam_prettier_divergence

A `<script>` / `<style>` / `<svelte:options>` written **inside** an ignore range is lifted out of
it ([range_section_hoist](../range_section_hoist/)): its bytes leave the frozen slice, and the
nodes written on either side of it become neighbours. The cut takes the section together with
the **weaker** of the two whitespace runs beside it; the **stronger** one stays, byte for byte.
Strength is the order the template join reads everywhere else — a blank line, then a line
break, then any other whitespace, then none — and on a tie the run after the section stays.

```svelte
<!-- prettier-ignore-start -->
{a} <script>
	let a = 1;
</script>text2
<!-- prettier-ignore-end -->
```

tsv keeps the space the author wrote before the section, which is the only thing separating
`{a}` from `text2`:

```svelte
<!-- prettier-ignore-start -->
{a} text2
<!-- prettier-ignore-end -->
```

The variants cover every pairing of the two runs, one range per strength (a space, a line
break, a blank line), and one range in the `format-ignore` spelling:

- `unformatted_ours_whitespace_before` — whitespace before the section only; it survives
- `unformatted_ours_whitespace_after` — whitespace after the section only; it survives
- `unformatted_ours_whitespace_both` — the same run on both sides; one survives (a blank line
  on each side is one blank line)
- `unformatted_ours_before_stronger` — the stronger run before the section (a line break
  before a space, a blank line before a line break); the stronger survives
- `unformatted_ours_after_stronger` — the stronger run after the section; the stronger survives

The blank-line range has text on both sides: outside a range the join collapses a blank line
between two text neighbours to a space, as it does `text3⏎⏎text4`, and inside a range the
blank line the author wrote stays.

Prettier re-lays out the whitespace between the nodes of a range like any other (see
[range_glued](../range_glued_prettier_divergence/)), and on every `unformatted_ours_*` variant
here it emits an invalid component instead: it hoists each section and leaves a copy of it in
the range, which Svelte rejects as a duplicate (`script_duplicate`, `svelte_meta_duplicate`).
The copy of a `<script>` / `<style>` also carries the placeholder prettier holds the section's
code in (`<script ✂prettier:content✂="…">{}</script>`), and prettier's own second pass throws on
it (`Attributes need to be unique`). `input.svelte` holds no section in a range, and both tools
keep it.

## Reason

A hoisted section renders nothing where it was written, so the whitespace around it is all
that separates its neighbours. Cutting the only run the author wrote welds them — `{a} text2`
renders `1 text2`, the welded `{a}text2` renders `1text2`. Of two runs, the one kept is the one
carrying the layout the neighbours would have had with no section between them, read the way
the join outside a range reads it (see
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)),
and it is kept as written, since every byte inside the range is the author's (the verbatim
spelling is pinned by [range_section_seam_verbatim](../range_section_seam_verbatim_prettier_divergence/)).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
