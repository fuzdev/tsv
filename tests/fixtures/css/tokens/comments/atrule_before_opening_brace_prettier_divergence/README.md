# atrule_before_opening_brace_prettier_divergence

Prettier preserves missing spaces before `/*` in at-rule preludes before the opening brace.

tsv: `@media screen /* comment */ {` (normalized spacing)
Prettier: `@media screen/* comment */ {` (no space before `/*`)

## Reason

Stable quirk. tsv normalizes comment spacing in the contexts whose grammar it parses (`@media`/`@supports` preludes, selectors, declaration values). Prettier adds space after `*/` but not before `/*` in this position, despite normalizing both in other contexts. (Raw at-rule preludes — `@layer`/`@keyframes`/`@namespace`/… — are kept verbatim instead, matching prettier, so they are *not* a divergence.) See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [atrule_in_prelude](../atrule_in_prelude_prettier_divergence/) — same quirk mid-prelude
- [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/) — same quirk for selectors
