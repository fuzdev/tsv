# region_end_trail_blank_lines_prettier_divergence

The whitespace between a hoisted section's closing tag and its `<!-- #endregion -->` trail.
Both formatters carry the marker below the section it closes and keep one authored blank
line there; they part on the *other* spellings of that gap. tsv normalizes them the way it
normalizes every gap — a same-line marker goes to the next line with no blank, three blank
lines collapse to one. prettier-plugin-svelte prints the trail's whitespace text with only its
first newline removed, so the leftover spelling comes out as is: a same-line marker gains a
blank line (`</script> <!-- #endregion -->` → `</script>⏎⏎<!-- #endregion -->`), and every
extra blank line survives (`</style>⏎⏎⏎⏎<!-- #endregion -->` keeps two).

tsv: one form per gap — glued (`</script>⏎<!-- #endregion -->`) or one blank
Prettier: one form per authoring — a fabricated blank from the same-line spelling, and as many
blank lines as were written

## Files

- `unformatted_ours_same_line.svelte` — the marker on the `</script>` line; tsv normalizes it
  to `input.svelte` (glued). Prettier does not (N6): it prints a blank line there instead.
- `variant_same_line.svelte` — prettier's output from that source, a form **both** formatters
  keep stable (a blank between a section and its marker is an authored blank to tsv as well).
- `unformatted_ours_extra_blank_lines.svelte` — three blank lines before the style's marker;
  tsv collapses them to the one blank `input.svelte` carries. Prettier does not.
- `prettier_variant_extra_blank_lines.svelte` — prettier's output from that source (two blank
  lines kept), stable under prettier; tsv normalizes it to `input.svelte`.

## Reason

`◆stable_quirk`. The gap between a section and its marker is a gap like any other: tsv holds
one blank at most and never manufactures one, so a document has one form there however it
was authored. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
