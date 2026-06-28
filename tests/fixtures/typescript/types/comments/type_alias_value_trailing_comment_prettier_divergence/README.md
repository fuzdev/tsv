# type_alias_value_trailing_comment_prettier_divergence

A comment between a type-alias value and its `;`. A **block** comment stays before
the `;` (`type A = B /* c */;`); a **line** comment moves after the `;`
(`type D = E; // c`) — both formatters agree on these, and `input.svelte` is stable
in both.

The divergence is in **normalization**: when the line-comment form is authored on
its own line before the `;` (`type W = A | B // c\n;`), tsv collapses it to the
inline `type W = A | B; // c` in a single pass, but prettier takes two. Its first
pass spuriously breaks the union value onto its own line
(`prettier_intermediate_before_semi`: `type W =\n\tA | B; // c`), and only a second
pass collapses it back to `input`. A non-union value (`type D = E`) doesn't trigger
the break; only a union/intersection value does.

Chain: `unformatted_ours_before_semi` → prettier → `prettier_intermediate_before_semi`
(unstable) → prettier → `input`. tsv reaches `input` in one pass.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
