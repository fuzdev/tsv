# param_default_own_line_block_comment_prettier_divergence

A block comment in a parameter's (or destructuring binding's) `=` default gap —
between `=` and the default value — has two authored-intentional inline
positions, and both formatters preserve each when given as input:

- Form A (inline, canonical input): `a = /* c */ b` — comment after `=`, on the value
- Form B (before `=`, dual-stable): `a /* c */ = b` — comment on the name (`variant_before_equals.svelte`)

The divergence is in how the formatters **normalize an own-line layout** — when
the author breaks the line inside the default gap. There are two such authorings,
and **tsv collapses both to Form A** (`a = /* c */ b`) — the comment stays inline
on the value side (block comments inline losslessly). Prettier also keeps the
comment inline but on a different side of `=`, splitting by *where* the break falls:

- Value on its own line, comment still trailing `=` (`a = /* c */⏎b`,
  `unformatted_ours_own_line.svelte`) → prettier collapses to **Form B**
  (`a /* c */ = b`) — the comment relocates across `=` onto the name.
- Comment on its own line, `=` left bare (`a =⏎/* c */⏎b`,
  `unformatted_ours_comment_own_line.svelte`) → prettier floats the comment out to
  **lead the whole parameter** on its own line (`/* c */⏎a = b`,
  `variant_lead_own_line.svelte`, dual-stable) — the parameter list breaks.

Per [Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the value (after `=`)
rather than floating it onto the name or out past the binding. The line-comment
form is a separate
divergence — there prettier floats the comment out to *trail* the parameter
(`a = b // c`); see
[param_default_line_comment](../param_default_line_comment_prettier_divergence/).
Covers function parameters and object/array destructuring binding defaults.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
