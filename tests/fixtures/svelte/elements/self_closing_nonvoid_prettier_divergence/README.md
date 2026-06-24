# self_closing_nonvoid_prettier_divergence

Svelte warns against self-closing syntax for non-void HTML elements (`<div />`).

tsv: normalizes to `<div></div>` (Svelte-recommended)
Prettier: preserves whichever form is used (both `<div />` and `<div></div>` are stable)

## Reason

**Design choice.** tsv follows Svelte's recommendation, normalizing non-void self-closing
elements to the long form; prettier keeps both forms stable. The `<div />` form still parses
(Svelte only *warns*), so this is tsv's normalization choice, not a hard spec violation. See
https://svelte.dev/e/element_invalid_self_closing_tag and
[conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
