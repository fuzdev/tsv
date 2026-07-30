# supports_selector_name_case_prettier_divergence

Prettier bug: a CSS function name is ASCII case-insensitive (css-values-4 §"Functional Notations": "like keywords, function names are ASCII case-insensitive"), so `SELECTOR(…)` is the same function as `selector(…)`. tsv recognizes it either way; prettier's check is case-sensitive, so an uppercase spelling falls through to its declaration path.

tsv: `SELECTOR(div:hover)` (the argument is a selector)
Prettier: `SELECTOR(div: hover)` (the argument is treated as a declaration)

Prettier's output is not a stylistic difference — `div: hover` is not a selector, so the condition it produces is false where the author's was true. Once prettier has written that form, neither formatter can recover it: `div: hover` no longer parses as a selector, so tsv keeps it opaque too (see [supports_selector_argument](../supports_selector_argument_prettier_divergence/)).

The name's own case is preserved by both formatters, matching how tsv preserves every function name (`URL(`, `RGB(`) — see [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) "CSS keyword case".

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) — "`selector()` argument".

## Related

- [supports_selector_interior](../supports_selector_interior/) — the lowercase spelling, where both formatters agree
- [supports_selector_argument](../supports_selector_argument_prettier_divergence/) — an argument that is not a selector, and a selector list
