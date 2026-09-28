# range_end_marker_verbatim_prettier_divergence

A root `prettier-ignore-start` / `-end` range whose content ends in a **space** or a **tab**
before the end marker, with text glued after the marker. tsv prints the range byte for byte
**through the end marker**: the whitespace in front of the marker is inside the range, so the
marker stays on it, and the text glued to the marker stays glued and wraps at its first space
when the line runs past the print width. Prettier moves the end marker to a fresh line in every
case, the short one included (`output_prettier.svelte`).

- `variant_own_line.svelte` — prettier's form, the marker on its own line: tsv keeps it too,
  since the line break is then the range's own byte.
- `unformatted_ours_joined.svelte` — the wrapped runs on one line: tsv wraps them back to
  `input.svelte`, prettier moves the marker.

Both forms render identically.

## Reason

Design choice — the range's promise is "these bytes are mine", and the end marker is the last of
them. The whitespace between the frozen content and the marker is the author's, as much as the
inter-node whitespace inside the range
([range_glued](../range_glued_prettier_divergence/)), so a break in front of the marker would
rewrite the range; the boundary that is the printer's begins after the marker, where it is laid
out as after any comment ([range_end_glued_follower](../range_end_glued_follower/),
[range_end_space_follower](../range_end_space_follower_prettier_divergence/)).

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
