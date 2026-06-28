# optional_paren_non_null_boundary_prettier_divergence

A non-null assertion at the boundary between a parenthesized optional chain and a
non-optional access. The parens **seal** the chain so the trailing access is not
short-circuited: `(a?.b)!.c` reads `.c` on the asserted result, whereas `a?.b!.c`
would short-circuit `.c` when `a` is null — a different expression. So the parens
are required and both formatters keep them; `input.svelte` (the `!`-outside form
`(a?.b)!.c`) is stable in both.

The divergence is in **canonicalization** of the equivalent `!`-inside-the-parens
authoring (`unformatted_ours_nonnull_in_parens`, `(a?.b!).c`):

- **tsv**: canonicalizes the assertion to the `!`-outside form `(a?.b)!.c` — the
  two are semantically identical (the `!` asserts the sealed chain either way), and
  tsv normalizes to the one form.
- **Prettier** (3.9, [#18661](https://github.com/prettier/prettier/pull/18661)):
  preserves the `!`-inside form `(a?.b!).c` as written.

tsv normalizes the two equivalent placements to a single canonical form; prettier
preserves the author's placement. The `unformatted_compact` / `unformatted_spaces`
variants (already `!`-outside) normalize to input under both formatters.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript.
