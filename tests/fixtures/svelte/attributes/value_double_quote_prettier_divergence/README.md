# value_double_quote_prettier_divergence

An attribute value containing a literal `"` (authored with single-quote
delimiters, `attr='a"b'`). tsv keeps the single-quote delimiters so the value
round-trips; prettier rewrites the delimiters to double quotes **without escaping
the interior `"`**, corrupting the markup.

tsv (idempotent):

```svelte
<div data-attr='a"b'></div>
<a data-title='say "hi"'></a>
```

Prettier re-quotes with `"` and the interior `"` prematurely terminates the value:

```svelte
<div data-attr="a"b"></div>
<a data-title="say "hi""></a>
```

Prettier's output is **broken** — it does not re-parse (`prettier(output)` throws
`Expected token =`), so the transform is non-idempotent and changes the document's
meaning. tsv defaults attribute delimiters to double quotes (`'value'` → `"value"`,
matching prettier-plugin-svelte) but switches to single quotes for the one value
shape double quotes cannot hold: one containing a literal `"`. In valid Svelte
source a quoted value's raw text can contain at most one of the two quote chars
(the delimiter quote can't appear literally inside), so this rule is total and
lossless — no entity-encoding is ever needed, and the author's exact value bytes
are preserved.

This is the same escape-minimizing quote choice tsv already applies to JS string
literals (prefer one quote, switch to the other rather than escape) and that
prettier's own **HTML** formatter applies to attributes; only prettier-plugin-svelte
regresses it.

## Reason

A formatter must never emit output that changes the document's meaning or fails to
re-parse. Preserving the author's `"` by delimiting with single quotes is the
minimal, lossless fix. See
[conformance_prettier.md §Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [modifier_preservation](../../directives/modifier_preservation_prettier_divergence/) — prettier silently drops directive modifier text; tsv preserves it (adjacent content-loss class)
- [single_quotes](../single_quotes/) — the ordinary `'value'` → `"value"` delimiter normalization (no interior `"`, both formatters agree)
