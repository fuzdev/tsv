# inline_element_long_prettier_divergence

A `{#snippet}` glued after text inside a component (`<A>text{#snippet name()}body{/snippet}</A>`),
across the print-width boundary.

tsv has **one form at every width**: the snippet is a declaration and takes its **own line**
([blocks/snippet/own_line](../own_line_prettier_divergence/)), so the component lays out
block-style — both tags intact, text and snippet on their own indented lines. Prettier instead
keys its layout on the compact spelling's width: it keeps the one-liner up to 100 chars, dangles
the closing `>` at 101 (`…{/snippet}</A⏎>`), and dangles both tags once the opening tag also
overflows (`<A href={x}⏎\t>…`).

```svelte
<!-- tsv (one form, any width) -->
<A>
	text
	{#snippet aaaa…()}x{/snippet}
</A>
```

`prettier_variant_compact.svelte` carries prettier's stable form of every case — the ≤100 cases
one-lined, the >100 cases in their dangled forms; tsv normalizes them all to `input`.
`unformatted_ours_compact.svelte` authors every case flat on one line: tsv normalizes it to
`input`, while prettier dangles the overflowing ones (landing on the variant's forms, not
`input`). The 100/101(/102) pairs pin exactly where **prettier's** form changes — tsv's does not.

## Reason

The break between the text and the snippet is render-free (the snippet hoists out of the
fragment before the whitespace rules run), so the layout question is free and tsv answers it
with the declaration's own line — uniform at every width, render-safe under Svelte 5. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

Note: when the snippet *name* alone overshoots printWidth (it is unbreakable), the snippet's own
line overshoots regardless of layout; that body-drop convergence is folded into the deferred
trailing/between-text hug-convergence follow-up and is not covered here.

## Related

- [blocks/snippet/own_line](../own_line_prettier_divergence/) — the own-line rule itself, at
  short widths
- [blocks/if/inline_element_long_prettier_divergence](../../if/inline_element_long_prettier_divergence/)
  — the block-style layout inside an inline element for `{#if}`, which is **not** a declaration
  and keeps the width-keyed behavior
