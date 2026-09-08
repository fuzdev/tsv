# space_separated_wide_first_item_long_prettier_divergence

A space-separated value whose **first** item does not fit on the colon's line — `prop: '<item>'`
past 100 columns — with and without a comment in the value.

tsv: the item drops to a fresh continuation line and the rest of the value fills beneath it
(`prop:⏎\t\t\t'<item>'⏎\t\t\t2px;`); the colon's line ends at the `:`
Prettier: the first item stays on the colon's line, overrunning by as much as the item is wide
(101 here), and only the tail wraps

## Reason

Print width. A fill item that does not fit mid-line goes to a fresh line — the wide-element drop
every tsv fill makes (see the Svelte text fill's [fill_spaced_tag_travel_long](../../../../svelte/elements/fill_spaced_tag_travel_long_prettier_divergence/)) —
and the first item of a declaration value is no exception: the fresh line reclaims the property's
width, and a value that cannot fit anywhere overruns by less there. Prettier's `fill` never breaks
ahead of its first content, so the head is unbreakable. At exactly 100 columns the item fits and
the two agree — the control cells. The comment-bearing cells take the same fill and the same
boundary, with the `: ` inside the doc so nothing trails on the colon's line.
See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Wide first item").

## Related

- [space_separated_long_wrap](../space_separated_long_wrap_prettier_divergence/) — the `;`-overage
  boundary of the same fill
- [space_separated_leading_comment_long](../space_separated_leading_comment_long_prettier_divergence/) —
  a leading comment run ahead of an over-long first word
