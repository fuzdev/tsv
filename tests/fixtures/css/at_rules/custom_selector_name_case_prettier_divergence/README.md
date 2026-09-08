# custom_selector_name_case_prettier_divergence

An at-rule name is ASCII case-insensitive (CSS Syntax 3), so `@CUSTOM-SELECTOR` and
`@Custom-Selector` are `@custom-selector` and their prelude routes through the selector
printer like the lowercase spelling's: `h1,h2` → `h1, h2`, `.class1>.class2` →
`.class1 > .class2`.

Prettier's routing test is **case-sensitive** (`node.name === "custom-selector"`, where its
`@media` / `@custom-media` arm compares the lowercased name), so an uppercase spelling is
not routed on the first pass: the name lowercases (`maybeToLowerCase`) but the prelude
stays verbatim — `@custom-selector :--a h1,h2;` — and only the **second** pass, now
seeing the lowercase name, formats the list. `prettier_intermediate_uppercase.svelte`
pins that unstable first pass; both formatters converge on `input.svelte`.

## Reason

◆prettier_bug (non-idempotent). tsv dispatches every at-rule on its lowercased name, so
one form for every spelling in one pass. See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(**The two prelude readers**, `@custom-selector`).

## Related

- [custom_selector](../custom_selector/) — the lowercase spelling (a match)
- [custom_media_prelude](../custom_media_prelude/) — `@custom-media`'s uppercase spelling, which prettier does route case-insensitively (a match)
