# supports_trailing_comma_prettier_divergence

A comma **closing** the value of an `@supports` declaration (`@supports (x: a,)`) is a
separator with no element after it. tsv keeps it, glued to the `)`; prettier deletes it.

tsv: `@supports (x: a,)` · `@supports (x: 'b',)`
Prettier: `@supports (x: a)` · `@supports (x: 'b')`

The rule is **token preservation**, the one a declaration's own value takes
([comma_closing](../../values/lists/comma_closing_prettier_divergence/)): a
`<supports-decl>` is `( <declaration> )` (css-conditional-3 §"@supports"), so its value is
a declaration value, and a comma the author wrote is one of its tokens. The condition asks
whether the UA supports that declaration, and css-values-4 §"Component value combinators"
requires a comma to be omitted when "all items following the comma have been omitted" —
so `(x: a,)` and `(x: a)` are two different questions, and deleting the comma can flip a
false condition true.

The comma takes no space after it: the space a value comma is given separates it from the
element that follows, and here the `)` closes the group. An authored run between the two
(`(x: a , )`, `unformatted_ours_spaces`) normalizes away, as whitespace ahead of a `)`
does everywhere in a condition, so the glued form is the one fixed point of every
authoring.

## Reason

**Content preservation.** Prettier is idempotent here and its output is valid CSS; tsv
emits the authored tokens rather than a repaired declaration. See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
("@supports closing comma").

## Related

- [comma_closing](../../values/lists/comma_closing_prettier_divergence/) — the same comma in a declaration's value
- [media_query_closing_comma](../media_query_closing_comma_prettier_divergence/) — the one construct whose closing comma tsv deletes
- [prelude_separator_spacing](../prelude_separator_spacing/) — a comma with an element after it, where the two agree on `, `
