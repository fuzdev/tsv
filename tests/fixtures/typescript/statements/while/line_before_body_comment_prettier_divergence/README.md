# line_before_body_comment_prettier_divergence

Line comments between a `while (…)` header's `)` and the body `{`.

A trailing comment (`while (a) // c`) and an own-line comment before `{`
(`while (a)\n// c\n{`) stay where the author wrote them in both formatters. The
divergence is the **blank line**: when the author leaves a blank line between `)`
and an own-line comment, tsv preserves it while prettier drops it.

## Reason

tsv treats the author's vertical spacing as intentional, preserving the blank
line before the comment. Consistent with tsv's comment-position handling across
control-flow statements.

`divergent_variant_spaces.svelte` is prettier's stable form reached from the
extra-whitespace `unformatted_ours_spaces.svelte` (it keeps a blank line *after*
the comment instead). It is a divergent variant: prettier keeps it, but tsv drops
the blank and settles on a distinct third stable form (neither prettier's nor the
input). `unformatted_ours_*` variants normalize back to input under tsv only.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
