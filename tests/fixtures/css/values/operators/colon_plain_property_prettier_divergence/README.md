# colon_plain_property_prettier_divergence

A **non-custom** property whose value carries a top-level `:` — `b: c:1.50`, `d: e : f`.

tsv: parses the declaration and keeps the value exactly as authored, glued or spaced
(`c:1.50` and `e : f` both survive, and the `1.50` is not normalized).
Prettier: **rejects** the input — `CssSyntaxError: Missed semicolon`. postcss's
`checkMissedSemicolon` reads a top-level `:` inside a declaration value as a second
declaration's colon; its one exemption is the lowercase word `progid` ahead of it.
Svelte's `parseCss` accepts the declaration (the AST is `expected.json`), and so does tsv.

## Reason

No oracle. A **custom** property's value is exempt from that check, so postcss parses it,
gives its top-level `:` a `colon` node and prints it `key: value`
([colon_custom_property](../colon_custom_property/)) — which is why that spelling has an
oracle and this one does not. Splitting the colon here would be tsv inventing a form no
other formatter emits, on a value it has no reading for; keeping the author's bytes is the
same answer tsv gives every other value it cannot grade.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Plain-property value colon").

## Related

- [colon_custom_property](../colon_custom_property/) — the custom-property spelling, which both
  formatters split
- [colon](../colon/) — a `:` inside a function or a group, where both formatters agree at every
  property
- [progid_opaque_case_prettier_divergence](../../progid_opaque_case_prettier_divergence/) — the
  same postcss check, reached through the `progid:` spelling
