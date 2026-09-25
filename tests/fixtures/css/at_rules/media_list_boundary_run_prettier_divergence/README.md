# media_list_boundary_run_prettier_divergence

A non-ASCII space (U+00A0, U+FEFF) in a `<media-query-list>`: at the list's head
(`@media <NBSP> screen, print`), either side of a comma (`screen <NBSP>, print`,
`screen, <ZWNBSP> print`), at its tail (`print <NBSP> {`, `print <NBSP>;`), and glued to the
name before a comma or at the tail (`screen<NBSP>, print`, `print<ZWNBSP> {`). Every entry
keeps the run as its own content; prettier drops it from every `@media` entry and from the
last `@import` entry (`@media screen, print {`, `@import 'a.css' screen, print;`). The
`@import` entries prettier keeps the run in are pinned by the plain
[import_media_list_boundary_run](../import_media_list_boundary_run/).

## Reason

Content preservation. Dropping a character the author wrote is content loss the corpus
SAFETY check reads as `content_lost` — here even one glued to a name, which is that name's
own content to css-syntax-3. See
[conformance_prettier_css.md §CSS: Selectors](../../../../../docs/conformance_prettier_css.md#css-selectors).
