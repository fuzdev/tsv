# atrule_in_prelude_prettier_divergence

Prettier preserves missing spaces around `/* */` in the middle of at-rule preludes.

tsv: `@media screen /* comment */ and (min-width: 500px) {` (normalized)
Prettier: `@media screen/* comment */ and (min-width: 500px) {` (no space before `/*`)
Prettier: `@media screen/* comment */and (min-width: 500px) {` (no space on either side)

Both compact forms are prettier-stable; tsv normalizes both to the spaced form.

Same quirk as `atrule_before_opening_brace`, but mid-prelude rather than before `{`.

## Reason

tsv normalizes comment spacing consistently across all CSS contexts.

## Related

- [atrule_before_opening_brace](../atrule_before_opening_brace_prettier_divergence/) — same quirk before `{`
- [media_list](../media_list_prettier_divergence/) — comment spacing across media query operators
