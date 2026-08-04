# comma_trailing_empty_element_prettier_divergence

A comma list whose **last element is empty** (`transition: a,,`,
`linear-gradient(red,,)`, `--x: a,,`) needs one more comma than it has separators to
spell itself: joining `["a", ""]` with `,` writes `a,`, which re-splits to `["a"]` —
the element silently gone. tsv writes the extra comma; prettier does not, and loses
the element.

tsv: `transition:⏎\ta,⏎\t,;` · `linear-gradient(red, ,)` · `--x: a, ,`
Prettier: `transition:⏎\ta,⏎\t;` · `linear-gradient(red, )` · `--x: a, `

Prettier's form is not a fixed point — `audit_signature.txt` pins the chain: its
**second** pass drops the element outright (`transition: a;`,
`linear-gradient(red)`, `--x: a;`). That is a render change, not a cosmetic one:
`linear-gradient(red,,)` is an invalid gradient the UA discards, so the element has
no background; `linear-gradient(red)` is valid and paints.

The **custom property** is the case with nothing to hide behind. The other two are
invalid declarations the UA drops, so the loss only shows once the shortened form
becomes *valid*; `--x`'s value is a verbatim token sequence with no grammar to fail
(css-variables-1 §"Custom Property Value Syntax"), so the deleted element changes
what every `var(--x)` substitutes regardless of where it is substituted. It is also
the one shape here that stays inline — a custom property is exempt from the
one-per-line comma break.

CSS Syntax 3 §"parse a comma-separated list of component values" is what makes the
last element real *and* the extra comma necessary: the loop consumes up to each
separator, discards it, and stops once the input is empty — so the stretch after a
*final* comma produces no group (`a,` is the one-element list `[a]`), while `a,,`
produces two, the second empty. Both readings are needed at once, which is why the
spelling takes N commas for N elements when the last is empty and N-1 otherwise.

Prettier keeps the comma in the one place tsv also does — `var()`'s empty fallback
(`var(--a,)`) — so the divergence is the general list, not the idea.

## Reason

**Spec precedence** (and content preservation). See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Trailing empty comma-list element").

## Related

- [comma_empty_element](../comma_empty_element/) — a leading or interior empty element, where tsv and prettier agree
- [var_comma_fallback](../../variables/var_comma_fallback/) — `var()`'s fallback is a comma token, and both formatters keep it
- [media_query_empty](../../../at_rules/media_query_empty_prettier_divergence/) — the same construct in a `<media-query-list>`
