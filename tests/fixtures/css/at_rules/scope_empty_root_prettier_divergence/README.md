# scope_empty_root_prettier_divergence

An `@scope` whose **root clause is an empty `<forgiving-selector-list>` and there is
no limit** — `@scope () { … }`. tsv keeps a single space before the clause paren
(`@scope ()`), the same spacing it uses for every other clause paren (`@scope (.a)`,
`@scope () to (.b)`). Prettier collapses the space **only** in this empty-root-only
case (`@scope()`), while keeping it everywhere else (`@scope () to (.b)` stays
spaced) — an inconsistency tsv declines to reproduce.

## Reason

Design choice — consistent spacing. tsv writes ` (` before each `@scope` clause
paren unconditionally, so an empty root renders `@scope ()`. Prettier special-cases
the fully-empty-root prelude to `@scope()` but not the empty root of a
root-plus-limit prelude, so tsv's uniform rule is the more defensible one. parseCss
accepts the input and captures the prelude raw (`()`), so this is prettier-only.

The empty/forgiving parse itself (accepting `@scope ()`, `@scope (.a, , .b)`,
`@scope (.)`) is covered by
[scope_forgiving_selector_list](../scope_forgiving_selector_list/) — those cases
match prettier; only the empty-root **spacing** here diverges. See
[conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules).

## Related

- [scope_forgiving_selector_list](../scope_forgiving_selector_list/) — the forgiving-list acceptance (empty / empty-item / invalid-item), matching prettier
- [scope_selector](../scope_selector_prettier_divergence/) — `@scope` selector-list whitespace normalization
