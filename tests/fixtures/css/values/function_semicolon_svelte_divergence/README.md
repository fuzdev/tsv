# function_semicolon_svelte_divergence

A `;` inside a function's parentheses in a declaration value — `prop: fn(a; b)`.
Per CSS Syntax 3, a declaration's value is a list of component values and a
`(…)`/`fn(…)` is consumed as a **simple block**; a `;` *inside* that block is
block content, **not** the declaration terminator (only a top-level `;`/`}`
ends the declaration). tsv follows the spec — it keeps consuming to the matching
`)` — and prettier agrees (formats `fn(a; b)` stable).

Svelte's `parseCss` instead truncates the declaration at the inner `;`, leaving
an empty declaration and reporting:

```
css_empty_declaration
```

This is the canonical-fails-tsv-ok pattern — tsv follows the CSS spec where
Svelte's parser is incomplete, the same shape as
[block_value](../variables/block_value_svelte_prettier_divergence/) and the
comment-position corrections. `expected_ours.json` pins tsv's AST;
`expected_svelte.json` records the Svelte rejection.

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).
