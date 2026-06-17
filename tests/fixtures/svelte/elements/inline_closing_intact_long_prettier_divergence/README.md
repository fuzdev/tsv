# inline_closing_intact_long_prettier_divergence

When an inline element sits at the fill boundary and only a trailing word pushes the line past
printWidth, tsv breaks at the whitespace before that word — keeping the closing tag intact — while
Prettier leaves the line long.

tsv: breaks at the whitespace before the trailing word (closing tag stays intact, ≤100)
Prettier: keeps the trailing word on the line (101, 1 over printWidth)

At 100 chars both formatters keep it inline.

## Reason

tsv treats printWidth as a hard limit. The break falls at the collapsible whitespace between the
element and the trailing word — a newline there is equivalent to the space, so the closing `>` is
never split off on its own. Prettier's fill allows the line to exceed printWidth here, the same
emergent fill-boundary behavior documented across the family.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

## Related

- [inline_element_fill_long](../inline_element_fill_long_prettier_divergence/) — same fill boundary with an attributed inline element (`<a href>`)
- [inline_component_fill_long](../inline_component_fill_long_prettier_divergence/) — same boundary with a component (`<Comp>`)
