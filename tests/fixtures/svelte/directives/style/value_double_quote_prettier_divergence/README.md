# value_double_quote_prettier_divergence

A `style:` directive with a quoted string value containing a literal `"`
(`style:color='a"b'`, `style:content='"x"'`). Like a plain attribute value, tsv
keeps single-quote delimiters so the value round-trips; prettier re-quotes with
`"` and the interior `"` corrupts the markup.

tsv (idempotent):

```svelte
<div style:color='a"b'></div>
<div style:content='"x"'></div>
```

Prettier corrupts (the interior `"` terminates the value early, output does not
re-parse):

```svelte
<div style:color="a"b"></div>
<div style:content=""x""></div>
```

Same root cause and fix as the plain-attribute case — a `style:` directive's
`Parts` value goes through the same quoted-text emission. See the sibling
[attributes/value_double_quote](../../../attributes/value_double_quote_prettier_divergence/)
for the full rationale.

## Reason

A formatter must never emit output that changes the document's meaning or fails to
re-parse. When a `style:` value's raw text contains a literal `"`, single-quote
delimiters are the only spec-conformant quoted form (HTML §13.1.2.3). See
[conformance_prettier.md §Svelte: Attributes](../../../../../../docs/conformance_prettier.md#svelte-attributes).
