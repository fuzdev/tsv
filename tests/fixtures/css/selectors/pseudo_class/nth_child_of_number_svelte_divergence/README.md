# nth_child_of_number_svelte_divergence

The `of S` selector list in `:nth-child(An+B of S)` accepts a bare `<number>`/`<an+b>`
term as `S` (`2n of 123`, `2n of 2n`, `3 of 7`) — the same `in_pseudo_args` production a
direct functional-pseudo argument uses (Svelte's `read_selector_list(inside_pseudo_class)`),
where a bare number/An+B reads as an `Nth` simple selector rather than a type selector. tsv
previously hard-rejected these (`Unexpected token in selector: number`) because the of-list
parse ran without `in_pseudo_args` set; both prettier and Svelte's `parseCss` accept them.

`S` need not be a bare term — it is a full `<complex-real-selector-list>`, so `2n of .a 123` nests
a two-hop complex selector (`.a` descendant a terminal `Nth "123"`; the descendant-position
`Nth` is the same `in_pseudo_args` production, recognized after an implicit combinator).

tsv nests `S` under `Nth.selector` (spec-compliant, the same structural correction as
[nth_child_of](../nth_child_of_svelte_prettier_divergence/)); Svelte flattens ` of ` into
`Nth.value` and reads `S` as a sibling `Nth`.

tsv: `Nth.value = "2n"`, with `Nth.selector` holding the nested `S` (here a single `Nth "123"`)
Svelte: `Nth.value = "2n of "`, with `S`'s `Nth "123"` as a sibling of the outer `Nth`

Formatting matches prettier (`2n of 123` — single spaces around `of`), so this is a pure AST
divergence (`_svelte_divergence`, no `output_prettier.svelte`).

## Reason

Same structural correction as [nth_child_of](../nth_child_of_svelte_prettier_divergence/):
CSS Selectors 4 (`#the-nth-child-pseudo`) defines `:nth-child(An+B [of S]?)` where `S` is a
nested `<complex-real-selector-list>`. Svelte's parser folds the `of` keyword into the An+B value
and flattens `S` as siblings. `S` here is itself the `in_pseudo_args` bare-`<an+b>`
over-acceptance (both parsers read `123`/`2n` as an `Nth`) — invalid as a real selector, but
tsv matches Svelte's acceptance and applies its principled `of S` nesting uniformly.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Fixture Structure

- `expected_ours.json` — tsv's spec-compliant nested output (source of truth)
- `expected_svelte.json` — documents Svelte's flattened output
