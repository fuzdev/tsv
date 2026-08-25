# inline_sibling_newline_label_hold_tag_pair_space_prettier_divergence

The sibling-newline flow rule's prose gate
([inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/))
is a **hold**, never a forced break: it decides whether an authored newline may reflow, and says
nothing about an authored space. A one-word run holds its authored lines, but its SPACE-spelled
tag pair — `text1 {expr1} {expr2}`, `{expr1} {expr2} {expr3} text1`, and the escaped-angle-bracket
pair at the root — is a fill like a prose run's and packs per width. Prettier's bare `line`
between two tags breaks with the multiline container whatever the run holds, so it splits every
one of them (`output_prettier.svelte`). The control is a prose-free pair (`{expr1} {expr2}`, no
content text at all), which breaks with the container under both formatters
(`unformatted_ours_space.svelte`: tsv normalizes its space spelling to the split `input.svelte`
form, prettier to `output_prettier.svelte`).

## Reason

Design choice. The whitespace-only separator before a tag asks two different run gates for its two
spellings. The NEWLINE spelling asks the prose gate — is there a phrase to reflow into — and a
one-word run says no, so the newline is held as the author's structure. The SPACE spelling asks
only whether the run holds a content text at all — is there a fill for this separator to sit in —
so it defers to the next tag's per-width group and the pair packs, exactly as a prose run's does
([inline_adjacent_sibling_newline_flow](../inline_adjacent_sibling_newline_flow_prettier_divergence/),
the prose tag pair prettier also splits). Gating the space on the prose count would turn the hold
into a forced break — `Price: {currency} {amount}` authored on one line coming out on two — a
layout keyed on nothing the author wrote. Only a run with no content text keeps the bare `line`
that breaks with the container: with no fill at all, its authored spacing is the only structure it
has, and both formatters agree (the tag-separator rule of
[inline_content_spaced_tags_long](../inline_content_spaced_tags_long_prettier_divergence/)).
Both arms of the separator site read the same run fact, so a width-broken container and a
newline-authored one lay the pair out identically.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
