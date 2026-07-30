# supports_selector_argument_prettier_divergence

tsv parses a `selector()` argument as the selector it is and prints it with the selector printer — the one a rule's own selector uses — so the same selector formats the same way in both positions ([supports_selector_interior](../supports_selector_interior/)). Two cases fall outside that and diverge from prettier.

**A selector list stays on the prelude's line.** Prettier breaks it one selector per line, hanging `selector(` and closing on its own `)`. A selector's break points belong to a rule's selector list; a condition's own wrapping is the `and`/`or` fill, so tsv keeps the argument inline.

tsv: `selector(.class1, .class2)`
Prettier: `selector(⏎.class1,⏎.class2⏎)`

**An argument that is not a selector keeps its tokens.** `<supports-selector-fn> = selector( <complex-selector> )` (css-conditional-4 §"Extensions to the @supports rule") — when the argument doesn't parse as a selector, `<supports-in-parens>` falls through to its third arm, `<general-enclosed>` (css-conditional-3), whose production is `[ <function-token> <any-value>? ) ]` (mediaqueries-4 §"Syntax"). That arm is *grammatically valid* — "the result is false", not invalid — so a formatter must keep the rule and has no grammar to normalize its contents against. An attribute value must be an identifier or a string (selectors-4 §"Attribute selectors"), so `1.50` is not one, and tsv's selector parser rejects it in rule position too (`div[data-attr=1.50] {}` is a parse error). tsv therefore leaves it as authored; prettier quotes it, which is a repair rather than a normalization — and one that can flip a false condition true.

tsv: `selector([data-attr=1.50])`
Prettier: `selector([data-attr='1.50'])`

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) — "`selector()` argument".

## Related

- [supports_selector_interior](../supports_selector_interior/) — a valid argument, formatted identically by both
- [supports_selector_name_case](../supports_selector_name_case_prettier_divergence/) — the same function, recognized case-insensitively
- [supports_long](../supports_long_prettier_divergence/) — where a condition does wrap
