# No-namespace Selector (Svelte Divergence)

## Feature
The no-namespace selector `|element` selects elements that belong to no namespace.
This is distinct from `*|element` (any namespace) and `ns|element` (specific namespace).

## CSS Specification
Per [CSS Selectors Level 4](https://www.w3.org/TR/selectors-4/#type-nmsp):
- `|E` - represents an element E with no namespace
- `*|E` - represents an element E in any namespace (including no namespace)

## Svelte Behavior
Svelte's CSS parser does not support the no-namespace selector syntax (`|div`, `|*`).
It reports: "Expected a valid CSS identifier"

## tsv Behavior
tsv correctly parses no-namespace selectors per the CSS spec:
- `|div` - type selector with explicit no namespace
- `|*` - universal selector with explicit no namespace

## Reason for Divergence
Svelte's CSS parser has incomplete namespace selector support. tsv follows the CSS spec.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).
