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

**Keeping the comment inside is a claim about position, never a licence to lose
it.** A lone block comment that fits stays inline and delimiter-tight (c3,
`{/* c3 */}`), but a **line** comment cannot be inlined — the `}` and the `;`
would land inside it — so it breaks the braces open (c5), and a **run** takes one
line per comment in source order (c6/c7), including two blocks (c8/c9), since the
dangling separator is unconditional. That is the empty tuple type's rule
(`type A = [/* c */]` vs `[⏎ // c⏎]`), reached through the same shared emitter, so
the two empty containers answer one question one way. Prettier relocates every one
of these out past the `;` instead.

Per Comment Position Philosophy, tsv keeps each comment where the author wrote it
rather than relocating it to a canonical position.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
