# inline_component_wide_multi_long_prettier_divergence

Two wide inline components in a single inline run. Each is too wide to share a line,
so each drops to its own line; the words around them (`and`, `mid words and`) stay
hugged on their text lines — none is stranded. Pins that a run with **repeated** flow
boundaries stays idempotent (a fix that handles one boundary but mishandles the second
would fail here).

tsv: words stay hugged, each component moves to its own line (every line ≤100)
Prettier: keeps `and <Comp` on the text line at each component (101) and breaks the
components internally — see `prettier_variant_attrs_hug.svelte` (prettier's stable
form, which tsv normalizes back to `input.svelte`).

## Reason

Print width. tsv treats printWidth as a hard limit and keeps each component intact rather than
splitting its attributes/closing `>`, so an over-wide component goes to its own line.
The boundary before each component is a collapsible space, so the word before it stays
on the text line. See
[conformance_prettier.md §Svelte: Elements (Wide inline child own-line)](../../../../../docs/conformance_prettier.md#svelte-elements).
