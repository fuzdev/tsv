# case_before_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a switch case's head→`:` gap
(`case x⏎/* c */⏎:`, `default⏎/* c2 */⏎:`). A single-line block forces
nothing, and a comment in this gap trails the head (a trailing position —
the `:` is a pure separator with no operand of its own on the line), so tsv
collapses the authored breaks and keeps the comment inline in its authored
syntactic slot (`case x /* c */:` — for a `case` with a test, the very form
prettier itself produces from the after-`:` authoring, see
[case_colon_comment](../case_colon_comment_prettier_divergence/)).

Prettier instead relocates the comment **across the `:` into the body**,
re-binding it from the case head to the first consequent statement — from the
inline authoring glued to the statement (`default:⏎\t\t\t/* c2 */ b();` —
`output_prettier.svelte`; the `case x` form it leaves alone, as its own
preferred slot), from the own-line authoring on its own line leading it
(`case x:⏎\t\t\t/* c */⏎\t\t\tb();` — `variant_own_line.svelte`, one pass,
dual-stable). The two authorings keep two fixed points under tsv-plus-variant;
only prettier moves a comment between them.

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes
  it to input in one pass; prettier takes it into the body instead.
- `variant_own_line.svelte` — prettier's own-line landing, dual-stable.

The inline authoring's divergence (`default /* c */:` → into the body) is
pinned by [case_colon_comment](../case_colon_comment_prettier_divergence/);
the same-gap **line** comment (which forces the break → continuation indent)
is
[case_before_colon_line_comment](../case_before_colon_line_comment_prettier_divergence/).
A **multiline** block in this gap stays glued to the `:` in both formatters —
the broke-after continuation rule is scoped to value-separator gaps, and
prettier does not distinguish the broke-after authoring here either.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
