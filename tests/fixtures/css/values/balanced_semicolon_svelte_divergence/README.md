# balanced_semicolon_svelte_divergence

A `;` inside a balanced construct in a declaration value — a simple block
`(x;y)` / `[x;y]` or a function fallback `var(--d, ;)`. Per CSS Syntax 3 a
declaration's value is a list of component values, and `()` / `[]` / `{}` simple
blocks (and `fn(…)` functions) are consumed as **balanced** units; a `;` *inside*
one is block content, **not** the declaration terminator (only a top-level
`;` / `}` ends the declaration). tsv follows the spec — it keeps consuming to the
matching close — and prettier agrees (formats each stable). This rounds out the
balanced-construct family alongside
[function_semicolon](../function_semicolon_svelte_divergence/) (the `fn(…)` case)
and [block_value](../variables/block_value_svelte_prettier_divergence/) (the `{…}` case).

Svelte's `parseCss` instead truncates the declaration at the inner `;`, leaving
an empty declaration and reporting:

```
css_empty_declaration
```

This is the canonical-fails-tsv-ok pattern — tsv follows the CSS spec where
Svelte's parser is incomplete. `expected_ours.json` pins tsv's AST;
`expected_svelte.json` records the Svelte rejection.

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).
