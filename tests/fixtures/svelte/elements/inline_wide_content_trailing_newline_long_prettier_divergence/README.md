# inline_wide_content_trailing_newline_long_prettier_divergence

The newline-authored companion of `inline_wide_content_trailing_long`. A wide inline element whose
**own content** (not its attributes) overflows, followed by trailing text the author placed on its
**own line** (a newline boundary). tsv wraps the over-wide content **inside** the element to honor
printWidth and keeps the trailing text on its own line — **respecting the authored newline**. The
no-attribute `<strong>` keeps its **opening tag attached** (`<strong>word…`); the attributed `<a>`
still dangles its opening `>` for now (the with-attrs opening-attach is a pending follow-up).

The space-authored counterpart (`inline_wide_content_trailing_long`) instead hugs the trailing text
onto the dangled closing `>` (`</tag⏎> tail`). So the two boundary authorings reach two different,
each-stable forms — tsv treats the boundary whitespace before trailing text as a meaningful authoring
choice, the same way a _short_ inline element keeps `<el>x</el> tail` inline for a space and breaks
for a newline.

Prettier keeps the content on a single over-width line (`output_prettier.svelte`) but agrees on the
tail (own line), so the **content wrap is the sole divergence** here.

## Reason

tsv treats printWidth as a hard limit and wraps over-wide inline content rather than emitting
prettier's single over-width line; trailing text authored on its own line stays there. See
[conformance_prettier.md §Wide inline content + trailing text](../../../../../docs/conformance_prettier.md#svelte-elements).
