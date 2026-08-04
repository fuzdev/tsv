# static_attribute_value_both_quotes_prettier_divergence

A top-level `<script>` / `<style>` attribute value that contains **both** quote
characters (`a=x'y"z`). No delimiter can hold it, so tsv emits it unquoted —
the form it was authored in, and the only lossless one. Prettier wraps it in
`"` regardless, and the interior `"` terminates the value early.

tsv (idempotent):

```svelte
<script a=x'y"z b='c"d' e="f'g"></script>
```

Prettier:

```svelte
<script a="x'y"z" b="c"d" e="f'g"></script>
```

Prettier's output is **broken markup**: `a` closes at the interior `"` with the
value `x'y`, the `z` after it reads as a stray attribute name, and the `"` that
follows raises `Expected token =`. Neither Svelte nor prettier itself can re-parse
it, so the transform is not idempotent and it changes the document's meaning.

Only a static tag head can reach this. Everywhere else the element attribute
reader bounds a raw value to at most one quote kind — its unquoted value stops
at either quote, and a quoted value cannot contain its own delimiter — which is
why the sibling [value_double_quote](../../attributes/value_double_quote_prettier_divergence/)
can state single quotes as a total rule. The static reader's value alternative
is `[^>\s]+` (Svelte's `read_static_attribute`), which admits both quotes at
once; because such a value came from that alternative it holds no whitespace and
no `>`, so leaving it unquoted always round-trips.

The other two rows are the one-kind contrast cases at the same position, where
the single/double rule applies unchanged: `b='c"d'` keeps single quotes,
`e="f'g"` takes the default double.

## Reason

A formatter must never emit output that changes the document's meaning or fails
to re-parse. See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [value_double_quote](../../attributes/value_double_quote_prettier_divergence/) — the one-literal-`"` case in ordinary elements; same rule, same prettier bug
- [static_attribute_grammar](../static_attribute_grammar/) — the static reader's grammar, including the `[^>\s]+` value alternative that makes this shape parse
