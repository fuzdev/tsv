# svelte_element_this_double_quote_prettier_divergence

A `<svelte:element>` plain-string `this=` value containing a literal `"`. Like a
quoted attribute value, tsv keeps single-quote delimiters so the value
round-trips; prettier re-quotes with `"` and the interior `"` corrupts the markup.

tsv (idempotent):

```svelte
<svelte:element this='a"b'></svelte:element>
<svelte:element this='say "hi"'></svelte:element>
```

Prettier corrupts (interior `"` terminates the value early; output does not
re-parse):

```svelte
<svelte:element this="a"b"></svelte:element>
<svelte:element this="say "hi""></svelte:element>
```

Same root cause and fix as the plain-attribute case (the plain-string `this=`
form emits its content between hardcoded double quotes). The braced form
`this={"a'b"}` is unaffected — it prints as a `{expr}`. See the sibling
[attributes/value_double_quote](../../attributes/value_double_quote_prettier_divergence/)
for the full rationale.

## Reason

A formatter must never emit output that changes the document's meaning or fails to
re-parse. Single-quote delimiters are the only spec-conformant quoted form for a
value holding a literal `"` (HTML §13.1.2.3). See
[conformance_prettier.md §Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).
