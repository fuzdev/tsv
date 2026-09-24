# leading_zwnbsp_prettier_divergence

A stylesheet whose formatted output **begins with a U+FEFF that tsv prints as content**,
authored behind a space (`unformatted_ours_leading_space.css`: ` ﻿a{}`). U+FEFF is not CSS
whitespace (css-syntax-3 §4.2) but an ident code point, so to a browser `﻿a` is not the type
selector `a`.

tsv writes a byte-order mark ahead of it: `input.css` is `BOM` + `U+FEFF` + the formatted
stylesheet. A UTF-8 decode strips exactly one leading BOM, so this is the only lossless spelling
of a text that begins with U+FEFF, and it is its own fixed point. A BOM with no content U+FEFF
behind it is still stripped (see [bom](../bom_prettier_divergence/README.md)).

`expected.json` pins `parseCss`'s reading of `input.css`: it strips one BOM, then steps over
the content U+FEFF as the selector list's leading boundary whitespace, so the `TypeSelector`
is `a` at offset 1. tsv's parse matches it. That U+FEFF is the non-ASCII boundary run tsv
already keeps at a selector-list start while prettier drops it
([conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors),
the non-ASCII boundary whitespace entry; the parse model is
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md)'s "Boundary whitespace is
JS `\s`"). What is new here is only where the run lands: at output byte 0, where it needs the
BOM to survive a read.

Prettier (postcss) strips the input's BOM and then the content U+FEFF too, restoring one:
`output_prettier.css` is `BOM` + `a {}`, the same bytes it gives the authored form.

See [Svelte leading_zwnbsp fixture](../../../../svelte/syntax/whitespace/leading_zwnbsp_prettier_divergence/README.md) and [conformance_prettier.md §Whitespace: BOM Handling](../../../../../../docs/conformance_prettier.md#whitespace-bom-handling).
