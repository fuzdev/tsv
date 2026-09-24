# leading_zwnbsp_prettier_divergence

A document whose formatted output **begins with a U+FEFF that is content** — here template
text, authored behind a space (`unformatted_ours_leading_space.svelte`: ` ﻿<div></div>`).
U+FEFF is not HTML whitespace, so Svelte keeps it in the DOM text and it renders.

tsv writes a byte-order mark ahead of it: `input.svelte` is `BOM` + `U+FEFF` + the formatted
document. A UTF-8 decode (WHATWG Encoding, and Svelte's own `parse`) strips exactly one leading
BOM, so a text that begins with U+FEFF has no other lossless spelling — without the BOM, the
next read takes the content character for a BOM and the text node is gone. The form is its own
fixed point: the read strips the BOM, the content U+FEFF is again output byte 0, and the BOM is
written again. Only a load-bearing BOM is written; a BOM with no content U+FEFF behind it is
still stripped (see [bom](../bom_prettier_divergence/README.md)).

`expected.json` pins Svelte's reading of `input.svelte`: one BOM stripped, then a `Text` node
holding the U+FEFF.

Prettier trims a leading U+FEFF from the template as whitespace (JavaScript `\s` includes it),
so it drops the text node: `output_prettier.svelte` keeps only the input's BOM, and the authored
form lands on the BOM-less `variant_text_dropped.svelte`. Both change the render.

See [conformance_prettier.md §Whitespace: BOM Handling](../../../../../../docs/conformance_prettier.md#whitespace-bom-handling).
