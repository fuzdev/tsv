# media_list_prettier_divergence

Prettier preserves whatever comment spacing the input has in @media preludes. tsv normalizes spaces around comments.

tsv: `screen /* c */ and`, `not /* c */ screen`, `(min-width: /* c */ 500px)`
Prettier: `screen/* c */and`, `not/* c */screen`, `(min-width: /* c */500px)`

## Reason

Stable quirk. tsv normalizes comment spacing consistently across all CSS contexts. This covers comments between boolean operators, before commas, and inside media features. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).
