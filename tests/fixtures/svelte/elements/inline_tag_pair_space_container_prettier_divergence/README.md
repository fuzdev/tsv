# inline_tag_pair_space_container_prettier_divergence

The container holding a space-spelled tag pair is not an axis. `{expr1} {expr2}` packs per width
in every multiline container — the root, a block element, an inline element authored with
boundary air, a list item, a table cell, a component's content, and an `{#if}` / `{#each}` /
`{#snippet}` body — where prettier's bare `line` between two tags breaks with the container and
splits the pair in every one of them (`output_prettier.svelte`).

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the rule of
[inline_tag_pair_space](../inline_tag_pair_space_prettier_divergence/), varied over the container
alone. The whitespace-only separator before a tag defers to the next tag's per-width group in
both arms of the separator site, and neither arm asks what container the run sits in, so a
block, an inline element made multiline by its own air, a table cell, a component and a block
body all reach one answer. The holding side of the same sweep — the NEWLINE spelling of a
prose-free or one-word run, held in every container — is
[inline_sibling_newline_label_hold_container](../inline_sibling_newline_label_hold_container_prettier_divergence/).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
