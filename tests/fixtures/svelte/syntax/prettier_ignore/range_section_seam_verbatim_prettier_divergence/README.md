# range_section_seam_verbatim_prettier_divergence

When a hoisted `<script>` / `<style>` is cut out of an ignore range, the whitespace run that
survives beside it — the stronger of the two, the one after the section on a tie (see
[range_section_seam](../range_section_seam_prettier_divergence/)) — survives **as the author
spelled it**: a tab stays a tab and three spaces stay three spaces. The run is kept whole and the
other one goes whole, so the indentation in front of the next node survives only when the run
after the section is the one kept.

```svelte
<!-- prettier-ignore-start -->
text1	<script>
	let a = 1;
</script>text2
<!-- prettier-ignore-end -->
```

tsv keeps the tab:

```svelte
<!-- prettier-ignore-start -->
text1	text2
<!-- prettier-ignore-end -->
```

- `unformatted_ours_before_kept` — the run before the section is the stronger one (a tab, three
  spaces, with nothing after); it survives as written
- `unformatted_ours_after_kept` — a tie (a space before a tab; a line break before a line break
  and its indentation); the run after the section survives as written, indentation included
- `unformatted_ours_blank_before_kept` — a blank line before the section and an indented line
  after it: the blank line is the stronger run and survives, and the indentation goes with the
  weaker run (`text1⏎⏎<style>…</style>⏎⇥⇥text2` → `text1⏎⏎text2`)

Prettier emits an invalid component on every variant: it hoists the section and leaves a copy
of it in the range, which Svelte rejects as a duplicate (`script_duplicate`, `style_duplicate`);
the copy also carries the placeholder prettier holds the section's code in
(`<script ✂prettier:content✂="…">{}</script>`), and prettier's own second pass throws on it
(`Attributes need to be unique`). `input.svelte` holds no section in a range, and both tools
keep it.

## Reason

A range's promise is that the bytes between its markers are the author's (see
[range_glued](../range_glued_prettier_divergence/)). Lifting a section out has to remove some
whitespace along with it, but where the kept run is one written before or after the section it
chooses which of the author's runs to keep, never a spelling of its own: rewriting the kept run
to a canonical space or line break would reformat the one part of the range the section left
behind, and splicing the two runs together would write a run the author never wrote. The one
respelling is a run written only BETWEEN two sections (see
[range_section_seam_edges](../range_section_seam_edges_prettier_divergence/)): it has no
neighbour on either side to stay with, so it prints as one of its line breaks, or as a single
space when it holds none, which renders the same.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
