# import_supports_long_prettier_divergence

Print width: an `@import` prelude whose `supports()` condition pushes the line past 100 chars. tsv breaks the condition onto its own indented line; Prettier tolerates 101 and, past that, breaks the *prelude* instead — putting `supports(…)` on a second line at the rule's own indent.

tsv: `@import url('a.css') supports(⏎\t(display: grid) and …⏎);`
Prettier: `@import url('a.css')⏎supports((display: grid) and …);`

The two forms differ on both axes this fixture pins:

**Where the break goes.** A `supports()` argument is the condition it says it is, so it takes the same break point every other function value takes — after the `(`, with the argument indented. Prettier's `@import` prelude is a value fill, so its break lands between prelude components, at the rule's indent rather than one level in.

**When it happens.** At exactly 101 chars Prettier stays inline; tsv treats print width as a hard limit and breaks. This is the same one-char boundary as [import_media_query](../import_media_query_long_prettier_divergence/) — the sibling divergence for the other wrappable `@import` component, where Prettier waits until 102 as well.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) — "@import `supports()` line wrap".

## Related

- [import_media_query_long](../import_media_query_long_prettier_divergence/) — the same boundary for an `@import` media query
- [supports_long](../supports_long_prettier_divergence/) — the same condition wrapping in `@supports` position, where the break point is the `and`/`or` fill
- [import_supports_condition](../import_supports_condition/) — the condition normalization that this position inherits
