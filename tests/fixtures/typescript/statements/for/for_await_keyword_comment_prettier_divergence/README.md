# for_await_keyword_comment_prettier_divergence

A comment between `await` and `(` in a `for await` header is preserved in place.
Prettier absorbs it inside the parens (before the binding).

- Input: `for await /* c */ (const x of y) {}`
- Prettier: `for await (/* c */ const x of y) {}` (absorbs into parens)
- Ours: `for await /* c */ (const x of y) {}` (preserves between keyword and paren)

Per comment placement policy, the user's chosen position is preserved. Both
positions are dual-stable in tsv (the inside-parens form `output_prettier`
round-trips unchanged here too); prettier keeps only the inside-parens form
stable, relocating the between-keyword form into it.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
