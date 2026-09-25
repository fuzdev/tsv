# template_tail_zwnbsp_prettier_divergence

Template text that is a single U+FEFF, authored ahead of the `<script>`
(`unformatted_ours_leading_space.svelte`: ` ﻿<script>let a;</script>`). U+FEFF is not whitespace
to Svelte's compiler (`clean_nodes` trims only `[ \t\r\n]`), so the text renders. The canonical
reorder prints the template after the script, where it **ends the document** — and Svelte's
`parse` trims the end of the document with JavaScript's `trimEnd()`, whose class (ECMAScript
`WhiteSpace` + `LineTerminator`) includes U+FEFF. Printed raw there, the character is gone on the
next read.

tsv spells the document's last character as a character reference, `&#xFEFF;`: Svelte decodes it
to the same text (the compiled component is byte-identical), and the `;` now ends the document,
so `trimEnd()` stops short of it. The form is its own fixed point under both formatters.

Prettier trims the U+FEFF as template whitespace (JavaScript `\s` includes it) and drops the text
node, so every BOM-less authoring lands on `variant_text_dropped.svelte` — a render change.

## Cases

- `unformatted_ours_leading_space.svelte` — the U+FEFF behind a space.
- `unformatted_ours_leading_newline.svelte` — the U+FEFF behind a newline.
- `unformatted_ours_bom.svelte` — a BOM, then the U+FEFF at the start of the text. The file's
  BOM is stripped by the read; after the reorder the content U+FEFF is no longer output byte 0,
  so no BOM is written ([leading_zwnbsp](../../../syntax/whitespace/leading_zwnbsp_prettier_divergence/)
  covers the text that stays first) — the reference at the tail is what keeps it. Prettier keeps
  the BOM and drops the text (`divergent_variant_bom.svelte`), which tsv strips of its BOM.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and, for the BOM, the shared frame's
[§Whitespace: BOM Handling](../../../../../../docs/conformance_prettier.md#whitespace-bom-handling).
