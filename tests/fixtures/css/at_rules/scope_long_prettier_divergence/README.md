# scope_long_prettier_divergence

Prettier does not implement line-width wrapping for an `@scope` prelude. tsv wraps at 101+ chars:
the clause that does not fit opens its parens, and a selector list inside it breaks one selector
per line.

tsv: wraps at 101 chars (>100)
Prettier: never wraps from inline input; preserves the wrapping if the input is already wrapped

| Line width | tsv    | Prettier (from inline) |
| ---------- | ------ | ---------------------- |
| 100 chars  | inline | inline                 |
| 101+ chars | wraps  | inline                 |

Which clause opens is decided clause by clause, left to right, each measured on its line through
whatever follows it up to the next place a break can land. The root is measured through its
`) to (`, so it stays on the at-rule's line when that line fits and the limit clause opens instead
— its selectors drop inside its parens. A root whose line would pass the print width with the
`) to (` behind it opens itself, and the limit is then measured on the `) to (…) {` line. A root
clause with no limit, and a limit clause with no root, are measured through their `) {` — or
through the `);` of a rule with no block.

Prettier has several stable forms, all idempotent under Prettier: the wrapped form, the inline form
(`prettier_variant_inline`), and the forms it gives the unspaced and the multi-spaced authorings
(`prettier_variant_compact`, `prettier_variant_spaces` — it keeps a glued `)to(` and the runs
inside the parens). tsv normalizes each to the wrapped one.

## Reason

See [conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules) (`@scope line wrap`, Print width). tsv enforces printWidth consistently across all CSS at-rules; Prettier never implemented wrapping for an `@scope` prelude.

## Related

- [container_long](../container_long_prettier_divergence/) · [media_long](../media_long_prettier_divergence/) · [supports_long](../supports_long_prettier_divergence/) — the same rule on the other conditional at-rules
- [scope_selector](../scope_selector_prettier_divergence/) — authored newlines inside an `@scope` prelude
