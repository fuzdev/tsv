# operator_before_group_prettier_divergence

An operator's **authored glue against a following parenthesized group** is preserved.
Prettier drops it — `1.5/(2.5)` → `1.5/ (2.5)` — while keeping the identical glue on the
group's other side (`(1.5)/2.5` is untouched).

`◆design_choice`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Operator glued to a following group".

## Why prettier's asymmetry is not a rule

Prettier decides "was this gap authored empty?" with `hasEmptyRawBefore`, which reads the
node's `raws.before`. A `value-paren_group` is **synthesized** by `parse-value.js` rather
than produced by the tokenizer, so it carries no `raws` at all and the question answers
`undefined` — never "glued". Three observations follow, and none of them is a rule prettier
states anywhere:

- the asymmetry itself: every operator sheds its glue *before* a group (`1.5/ (2.5)`,
  `1.5+ (2.5)`, `1.5- (2.5)`) and keeps it *after* one (`(1.5)/2.5`, `(1.5)+2.5`,
  `(1.5)-2.5`);
- it fires inside `calc()`, whose own documented rule is to print a `+` / `-` gap "as is …
  due to the fact that it is not valid syntax" to change it — so `calc(1.5+(2.5))` becomes
  `calc(1.5+ (2.5))`, contradicting the rule one line above it in prettier's own source;
- the `font` shorthand carve-out short-circuits ahead of it, so `font: 1.5/(2.5)` keeps the
  glue prettier drops for every other property — the same authoring, two answers, chosen by
  the property name.

## Why tsv keeps the glue

css-values-4 `<calc-sum>`: *"whitespace is required on both sides of the `+` and `-`
operators. (The `*` and `/` operators can be used without white space around them.)"*

- For `*` and `/` the glue is spec-permitted either way, so it is the author's to keep —
  and tsv already keeps it in every other position (`1.5/2.5`, `(1.5)/2.5`).
- For `+` and `-` the gap is load-bearing, which is exactly why both formatters otherwise
  print it as authored. Adding a space on **one** side of an invalid `calc(1.5+(2.5))`
  leaves it invalid while making it look corrected; preserving the authoring reports it
  unchanged.

Prettier is idempotent on its own output here, so nothing but this oracle reveals it.

## Cells

| authoring | tsv | prettier |
| --- | --- | --- |
| `1.5/(2.5)` | `1.5/(2.5)` | `1.5/ (2.5)` |
| `1.5+(2.5)` | `1.5+(2.5)` | `1.5+ (2.5)` |
| `1.5-(2.5)` | `1.5-(2.5)` | `1.5- (2.5)` |
| `(1.5)/(2.5)` | `(1.5)/(2.5)` | `(1.5)/ (2.5)` |
| `calc(1.5+(2.5))` | `calc(1.5+(2.5))` | `calc(1.5+ (2.5))` |

The last three declarations are the controls, where the two agree: an authored gap
(`1.5 / (2.5)`, `calc(1.5 + (2.5))`) and glue on the group's other side (`(1.5)/2.5`).
The agreeing cells that bound the rule the other way — the member gap between two adjacent
groups, `*` beside a group, and the `font` carve-out — are in
[paren_group_boundary](../paren_group_boundary/).
