# inline_element_wide_long_prettier_divergence

Parity guard for the wide-inline-element case: a wide HTML inline element (`<a>`) at
the same boundary as `inline_component_wide_long`. tsv drops the whole element to its
own line and keeps the preceding word (`and`) hugged — **identical in shape** to the
component fixtures.

Inline HTML elements already behave correctly here (unlike components, which strand the
preceding word — see `inline_component_wide_long`). This fixture exists so that:

1. the wide-element behavior is not regressed by the component fix, and
2. element and component output stay byte-identical in shape (the fix should make
   components match elements, not introduce a second shape).

tsv: word stays hugged, the whole element moves to its own line (each line ≤100)
Prettier: keeps `and <a` on the text line (101) and breaks the element internally —
see `prettier_variant_attrs_hug.svelte` (prettier's stable form, which tsv normalizes
back to `input.svelte`).

## Reason

tsv treats printWidth as a hard limit and keeps the element intact rather than splitting
its attributes/closing `>`, so an over-wide element goes to its own line. The boundary
before the element is a collapsible space, so the word before it stays on the text line.
See [conformance_prettier.md §Inline content hug](../../../../../docs/conformance_prettier.md#svelte-elements).
