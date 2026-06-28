# attributes_empty_comment_prettier_divergence

Comments around an **empty** import-attributes clause (`with {}`) are preserved
where the author placed them. The empty `with {}` itself is kept by both
formatters; only the comment position diverges.

A source→`with` block comment (c1) and an after-`}` comment (c4) land the same
way under both formatters: c1 stays between the source and `with`, and the
after-`}` comment trails past the `;` — the lossless trail-past-a-separator
carve-out. The `with`→`{` comment (c2) and the inside-braces comment (c3) diverge:
prettier relocates both to before `with`, tsv keeps them where they were written.
This is the empty-`with` analog of the `with_keyword_comment` and
`source_trailing_comment` import divergences.

Per Comment Position Philosophy, tsv keeps each comment where the author wrote it
rather than relocating it to a canonical position.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
