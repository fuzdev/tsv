# self_closing_nonvoid_prettier_divergence

Svelte warns against self-closing syntax for non-void HTML elements (`<div />`).

tsv: normalizes to `<div></div>` (Svelte-recommended)
Prettier: preserves whichever form is used (both `<div />` and `<div></div>` are stable)

## Reason

tsv follows Svelte's recommendation. See https://svelte.dev/e/element_invalid_self_closing_tag.
