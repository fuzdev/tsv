# discriminant_trailing_comment_prettier_divergence

Prettier moves trailing comments from switch discriminant parens into the switch body (`switch (x) {\n\t// comment` instead of `switch (\n\tx\n\t// comment\n) {`).

tsv: preserves comments where the user placed them
Prettier: relocates comments to a different position

Both authorings of the gap reach the same tsv fixed point — the own-line form and the closing-paren-line form (`switch (y\n/* trailing */) {`, pinned by `unformatted_ours_close_paren_line.svelte`; a `//` has no such spelling, since it would swallow the `) {`). Prettier's landing differs between them: own-line leads the case on its own line (`output_prettier.svelte`), closing-paren-line glues to the label (`variant_close_paren_line.svelte`, which both formatters keep stable).

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, for, while, do-while, labeled statements, and call chains.

The same gap in `if` / `while` / `do…while` / `catch` is not a divergence at all — prettier preserves the comment there too, and tsv matches it (`syntax/comments/condition_paren_comment_on_close_line`). Switch is the one member of the family whose comment prettier moves.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
