# inline_tag_pair_space_welded_prettier_divergence

A **welded** tag unit beside a **spaced** separator. The glued boundary inside the unit —
`{expr1}{expr2}`, an `&nbsp;`, a word glued to both tags — is never split, since breaking there
would inject a rendered space; the spaced boundary beside it packs per width exactly as it does
between two lone tags, so the whole run sits on one content line. Prettier's bare `line` between
a tag and its spaced neighbour breaks with the multiline container, so it splits at every spaced
boundary (`output_prettier.svelte`) while keeping the welds.

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the rule of
[inline_tag_pair_space](../inline_tag_pair_space_prettier_divergence/) meeting the welded-unit
rule of [§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style):
a separator's *presence* carries signal (a glued boundary is never split) and its *spelling* does
not (a space before a tag defers to the tag's per-width group). The welded unit is one movable
item and the space in front of or behind it is the fill boundary that moves it.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
