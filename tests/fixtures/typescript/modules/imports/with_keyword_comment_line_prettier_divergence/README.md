# with_keyword_comment_line_prettier_divergence

A **line comment between an import's source and its `with` attributes keyword**.
Prettier collapses `with {…}` back onto the source line and floats the line
comment past the `;`. tsv keeps the comment between the source and `with`,
forcing `with {…}` onto the next line and indenting the continuation one level.

The block-comment forms (source→`with` and `with`→`{`, plus the line comment
*after* `with`) are covered in the sibling
`with_keyword_comment_prettier_divergence` fixture.

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
rather than relocating it past the `with` clause and `;`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
