# template_tail_form_feed_prettier_divergence

Template text ending in a form feed, authored ahead of the `<script>`
(`unformatted_ours_before_script.svelte`). A form feed is rendered content, not collapsible
whitespace, so tsv keeps it wherever it appears in template text
([text_form_feed](../../../elements/text_form_feed_prettier_divergence/)). The canonical reorder
prints the template after the script, where it **ends the document** — and Svelte's `parse` trims
the end of the document with JavaScript's `trimEnd()`, which removes U+000C (ECMAScript
`WhiteSpace`). Printed raw there, the character is gone on the next read.

tsv spells the document's last character as a character reference, `&#xC;`, which Svelte decodes
to the same text; the `;` now ends the document. The form is its own fixed point under both
formatters.

Prettier respells the form feed as collapsible whitespace and trims it, landing on
`variant_form_feed_dropped.svelte` in one pass — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and [§Whitespace: Form feed](../../../../../../docs/conformance_prettier_svelte.md#whitespace-form-feed).
