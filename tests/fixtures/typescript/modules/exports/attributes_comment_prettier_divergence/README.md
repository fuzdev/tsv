# attributes_comment_prettier_divergence

Comments in a re-export's attributes clause (`export … from … with {…}`, across
both the `export { … } from` and `export * from` hosts) are preserved where the
author placed them — the re-export analog of the import `with_keyword_comment`
and `source_trailing_comment` divergences (one shared attribute-clause printer
handles all three hosts).

A source→`with` block comment (c1) and an after-`}` comment (c3, c5) agree
between both formatters: c1 stays between the source and `with`, and the
after-`}` comment trails past the `;` — the lossless trail-past-a-separator
carve-out. Only the `with`→`{` comments (c2, c4) diverge: prettier relocates them
to before `with`, tsv keeps them after `with`.

Per Comment Position Philosophy, tsv keeps each comment where the author wrote it
rather than floating it to a canonical position.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
