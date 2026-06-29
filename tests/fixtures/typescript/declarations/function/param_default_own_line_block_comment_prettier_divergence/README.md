# param_default_own_line_block_comment_prettier_divergence

A block comment in a parameter's (or destructuring binding's) `=` default gap —
between `=` and the default value — has two authored-intentional inline
positions, and both formatters preserve each when given as input:

- Form A (inline, canonical input): `a = /* c */ b` — comment after `=`, on the value
- Form B (before `=`, dual-stable): `a /* c */ = b` — comment on the name (`variant_before_equals.svelte`)

The divergence is in how the formatters **normalize an own-line layout** — when
the author breaks the line after the comment so the value sits on its own line
(`a = /* c */⏎b`, `unformatted_ours_own_line.svelte`):

- **tsv** collapses to Form A (`a = /* c */ b`) — the comment stays on the value side.
- **Prettier** collapses to Form B (`a /* c */ = b`) — it relocates the comment
  across `=` onto the name.

Either way the comment ends up inline (block comments inline losslessly), so
neither formatter wraps here; they just pick different canonical sides of `=` for
the ambiguous own-line input. Per [Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the value (after `=`)
rather than floating it onto the name. The line-comment form is a separate
divergence — there prettier floats the comment out to *trail* the parameter
(`a = b // c`); see
[param_default_line_comment](../param_default_line_comment_prettier_divergence/).
Covers function parameters and object/array destructuring binding defaults.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
