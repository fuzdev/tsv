# supports_escaped_selector_name_prettier_divergence

An `@supports` condition whose `selector()` function name is **escape-spelled** —
`s\65 lector(#ABC)`.

`\65 ` is a valid escape (CSS Syntax 3 §4.3.7) and §"Consume an ident sequence" takes its
payload as ident content, so `s\65 lector` is the single ident `selector` — the same
function name as `selector`, which css-values-4 §"Functional Notations" makes ASCII
case-insensitive. tsv resolves the escape when it recognizes the name
(`printer::values::function_name_is`), so the argument is parsed and printed as a selector
and the ID keeps its case: an ID selector is case-sensitive (selectors-4 §"ID selectors"),
so `#ABC` and `#abc` match different elements.

Prettier's `selector()` check reads the name's bytes verbatim, so an escape-spelled name
falls through to its declaration path, where the argument is a value and `#ABC` is a hex
colour to lowercase. Its `#abc` selects a different element than the author wrote, so this
is a content change rather than a spacing choice — and prettier's form is its own fixed
point, so a second pass raises nothing.

This is the escape-spelled twin of the uppercase spelling in
[supports_selector_name_case](../supports_selector_name_case_prettier_divergence/): one
divergence, two ways of spelling a name prettier's literal check misses. The lowercase
spelling, where both formatters agree, is [supports_selector](../supports_selector/).

## Reason

See [conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules) — "`selector()` name case".

## Related

- [supports_selector_name_case](../supports_selector_name_case_prettier_divergence/) — the uppercase spelling of the same miss
- [supports_hex_case_sensitive_preserved](../supports_hex_case_sensitive_preserved/) — where both formatters keep an ID's case
