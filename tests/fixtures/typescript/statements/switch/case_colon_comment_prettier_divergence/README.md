# Switch case colon comment divergence

Prettier relocates comments near the colon in switch cases:

1. `case 1: /* c */` → `case 1 /* c */:` (moves from after colon to before colon)
2. `default /* c */:` → `default: /* c */ break;` (moves from before colon to body)

tsv preserves comment placement per the comment position philosophy.

The relocated forms prettier produces are dual-stable (both formatters keep them
as-is, so they are `variant_*`, not the canonical input):

- `variant_relocated.svelte` — `case 1 /* c */:` (before colon) + `default: /* c */ break;` (body); identical to `output_prettier.svelte`.
- `variant_body.svelte` — both comments in the case body (`case 1:\n\t/* c */ break;`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
