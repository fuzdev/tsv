# Switch case colon comment divergence

Prettier relocates comments near the colon in switch cases:

1. `case 1: /* c */` → `case 1 /* c */:` (moves from after colon to before colon)
2. `default /* c */:` → `default: /* c */ break;` (moves from before colon to body)

tsv preserves comment placement per the comment position philosophy.
Both positions are dual-stable in our formatter (`variant_relocated.svelte`).
