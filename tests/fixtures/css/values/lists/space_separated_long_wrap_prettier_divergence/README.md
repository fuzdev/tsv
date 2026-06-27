# space_separated_long_wrap_prettier_divergence

When a CSS space-separated value's content fills exactly to printWidth and the declaration's
trailing `;` becomes the 101st char, Prettier keeps the line at 101; tsv wraps to stay ≤100.

tsv: wraps to respect printWidth (every line ≤100)
Prettier: tolerates a **1-char** overage — the value content stays ≤100, the trailing `;`/`,`
terminator is the lone 101st char (Prettier's `fill()` doesn't count the parent's trailing
punctuation; it never exceeds by more — a value content of 101 wraps for both)

## Reason

Print width. tsv treats printWidth as a hard limit, so it breaks one item early rather than let the
trailing terminator push the line to 101; Prettier leaves it on one line. Same precise 1-char
trailing-punctuation overage documented at [comma_separated_greedy_fill](../../../comma_separated_greedy_fill_prettier_divergence/).
See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values) ("Space-separated value wrap").

## Related

- [transform_long](../../functions/transform_long_prettier_divergence/) — same pattern for function-heavy values
- [comma_space_separated_long](../comma_space_separated_long_prettier_divergence/) — comma + space variant
