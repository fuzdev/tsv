# template_tail_line_separator_prettier_divergence

Template text ending in U+2028 LINE SEPARATOR, authored ahead of the `<script>`
(`unformatted_ours_before_script.svelte`). U+2028 is not whitespace to Svelte's compiler
(`clean_nodes` trims only `[ \t\r\n]`), so it renders. The canonical reorder prints the template
after the script, where it **ends the document** — and Svelte's `parse` trims the end of the
document with JavaScript's `trimEnd()`, which removes U+2028 (ECMAScript `LineTerminator`, the
other half of the class beside `WhiteSpace`). Printed raw there, the character is gone on the
next read.

tsv spells the document's last character as a character reference, `&#x2028;`, which Svelte
decodes to the same text; the `;` now ends the document. The form is its own fixed point under
both formatters.

Prettier keeps the raw character on its first pass
(`prettier_intermediate_to_variant_before_script.svelte`) and loses it on its second, landing on
`variant_line_separator_dropped.svelte` — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
