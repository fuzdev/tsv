# progid_opaque_case_prettier_divergence

The opaque `progid:` value ([progid_opaque](../progid_opaque/)) spelled in another case —
`PROGID:` / `Progid:`.

tsv: reads the trigger ASCII-case-insensitively, so the value freezes verbatim exactly as the
lowercase one does (`1.50`, `"#FF0000"`, `1.50PX`, the four interior spaces all kept)
Prettier: **rejects** the input — `CssSyntaxError: Missed semicolon`. postcss's `checkMissedSemicolon`
reads a top-level `:` inside a declaration value as a second declaration's colon, and its one
exemption is the word `progid` spelled in lowercase immediately ahead of it; prettier's own
`progid:` arm never runs. Svelte's `parseCss` accepts the value (the AST is `expected.json`), and
so does tsv.

## Reason

No oracle, and the same reasoning as the lowercase class: IE's filter syntax is case-insensitive,
tsv's parser accepts every spelling, and a value the formatter does not understand is frozen
rather than normalized — otherwise `Progid:` would normalize numbers where `progid:` keeps them, a
cliff keyed on letter case. The prettier verdict is a parser limitation of postcss, not a
formatting opinion. See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values)
("`progid:` opaque value").

## Related

- [progid_opaque](../progid_opaque/) — the lowercase spelling, the agreeing cells
- [progid_opaque_prettier_divergence](../progid_opaque_prettier_divergence/) — the comment and
  mid-value cells
