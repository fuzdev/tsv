# param_default_division_header_comment_prettier_divergence

A statement header inside a parameter default, with a comment between the keyword and its
`(` and another after its `)`: the `<` after the `)` opens an angle-bracket assertion, not a
type-argument list, so the `/` after the assertion's `>` opens a regex — whose `)` must not
close the parameter list. The scan that finds the list's `)` reads the header through both
comments.

The divergence is the comment's position, not the scan:

- Input / tsv: `if /* x */ (c) /* y */ <RegExp>/[)]/;` (kept between the keyword and `(`)
- Prettier: `if (/* x */ c) /* y */ <RegExp>/[)]/;` (relocated inside the parens)

The same relocation as
[keyword_paren_comment](../../../statements/if/keyword_paren_comment_prettier_divergence/),
here in a TypeScript script so the assertion parses.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
