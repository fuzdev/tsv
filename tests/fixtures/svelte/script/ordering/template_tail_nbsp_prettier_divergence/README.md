# template_tail_nbsp_prettier_divergence

Template text ending in a non-breaking space, authored ahead of a hoisted section. U+00A0 is not
whitespace to Svelte's compiler (`clean_nodes` trims only `[ \t\r\n]`), so it renders. The
canonical reorder prints `<svelte:options>` and both scripts above the template, so the text
**ends the document** — and Svelte's `parse` trims the end of the document with JavaScript's
`trimEnd()`, which removes U+00A0 (ECMAScript `WhiteSpace`). Printed raw there, the character is
gone on the next read.

tsv spells the document's last character as a character reference, `&#xA0;`: Svelte decodes it
to the same text (the compiled component is byte-identical), and the `;` now ends the document,
so `trimEnd()` stops short of it. The form is its own fixed point under both formatters.

Prettier keeps the raw character on its first pass (`prettier_intermediate_to_variant_before_instance.svelte`)
and loses it on its second, landing on `variant_nbsp_dropped.svelte` — a render change.

## Cases

The text is authored ahead of each section the reorder hoists, since whichever one follows it,
the template ends up last:

- `unformatted_ours_before_options.svelte` — ahead of `<svelte:options>`.
- `unformatted_ours_before_module.svelte` — ahead of the module script.
- `unformatted_ours_before_instance.svelte` — ahead of the instance script.
- `unformatted_ours_before_instance_trailing_space.svelte` — the same, with a collapsible run
  after the U+00A0. The run is render-free and normalizes away; the U+00A0 is what the reference
  spells.
- `unformatted_ours_before_instance_crlf.svelte` — the same, with CRLF line endings. The format
  path folds them ahead of the parse, so the `<CR>` never reaches the text's tail.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
