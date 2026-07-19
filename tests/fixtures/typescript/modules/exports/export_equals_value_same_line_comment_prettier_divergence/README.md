# export_equals_value_same_line_comment_prettier_divergence

A **same-line** block comment after `export =`, with the value authored on the next line
(`export = /* c */⏎value`). The `export =` twin of
[default_value_same_line_comment](../default_value_same_line_comment_prettier_divergence/).

**tsv** reflows the value inline, keeping the comment where the author wrote it:

```
export = /* c */ value;
```

**Prettier** preserves the author's break:

```
export = /* c */
value;
```

`prettier_variant_authored_break.svelte` carries that broken form: prettier keeps it
stable, tsv normalizes it back to `input.svelte`.

## Reason

**Design choice — uniformity.** Same rule as the `export default` twin: a block comment
does not run to end-of-line, so nothing forces the value off the `=` line, and tsv's TS
printer reflows an unforced break at every value position.

This fixture exists because the pair is the tightest possible test of that uniformity —
`export =` and `export default` are the same shape in the same position, so a formatter
treating them differently is inconsistent with itself regardless of what prettier does.
The two gaps disagreed before this rule was stated: `export =` reflowed while
`export default` preserved and indented, and no fixture covered `export =`, so nothing
caught it.

See [conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).
