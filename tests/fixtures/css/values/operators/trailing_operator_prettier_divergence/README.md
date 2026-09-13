# trailing_operator_prettier_divergence

A declaration value ending in an operator that has **no operand after it** — `1.50*-`,
`1.50*+`, `1.5 / -`, and the two-operator runs `+-`, `++`, `-+`, `/+`.

tsv: reads the run as members like any other — a number, an operator and a trailing
operator member — so the number normalizes and each gap is decided by the two members
beside it (`1.5 * -`, `+-`)
Prettier: **freezes the whole declaration verbatim** — every authoring of it is a prettier
fixed point, `1.50` and the authored gaps included

## Reason

No oracle. postcss-values-parser fails on a value whose last token is an operator with
nothing to bind, and prettier's value layer answers a failed parse by printing postcss's own
`value` string — so `1.50*-`, `1.5 *-`, `1.5* -` and `1.5 * -` are four different stable
outputs of one value, and the `1.50` in each is left exactly as authored. The freeze is the
same one [line_comment_arg](../../functions/line_comment_arg_prettier_divergence/) and
[stray_close_paren](../../stray_close_paren_prettier_divergence/) reach by unbalancing a
paren, one construct over.

tsv normalizes to the form prettier's own separator rule produces where its parse *does*
succeed. ⚠️ **That is a function's argument list, and only it.** A custom property is
frozen exactly as a plain property is — `--x: 1.50 /-`, `--x: 1.50*-` and `--x: + -` all
come back as authored — so `f(1.5 * -)`, `f(1.5 /-)` and `f(+-)` are the bounding cells
that carry an oracle, while `--n: +-` and `--o: a +-` agree only because tsv's output *is*
the frozen bytes. The `prettier_variant_spaced_head` file pins that freeze rather than
asserting it: its `--n: + -` is a form prettier keeps and tsv normalizes onto input's
`--n: +-`. So the divergence is a convergence-count one — prettier holds one stable form
per authoring of a value the language reads one way — and the representative tsv picks is
prettier's own.

⚠️ **Which gaps that leaves free is the separator rule's answer, not a normalization to one
form per value.** A trailing operator is no word, so a `*` beside it always spaces
(`1.50*-`, `1.5 *-`, `1.5* -` all → `1.5 * -`) while a `/` beside it takes the **author's**
gap on both of its sides: `1.5/-`, `1.5 /-`, `1.5/ -` and `1.5 / -` are four fixed points
at a plain property, and prettier reproduces them inside the parens, where its parse
succeeds (`f(1.50 /-)` → `f(1.5 /-)`, `f(1.50/ -)` → `f(1.5/ -)`). Same rule for the
two-operator runs: a head operator binds to the member after it, so `+-`, `++`, `-+` and
`/+` glue, and the spaced authorings (`+ -`, `+ +`, `- +`, `/ +`) normalize onto them — the
`prettier_variant_spaced_head` file.

⚠️ **A custom property is not a fifth spelling of the plain property's four.** Its `/` is
the `font` shorthand's (`ValueScope::font_shorthand` in
[printer/values.rs](../../../../../../crates/tsv_css/src/printer/values.rs): "The `font`
shorthand and a custom property keep a glued `/` beside a font-size operand"), and the arm
that reads the gap on the operator's **own** left carries that glue across a gap the author
left open. So `--p: 1.50/ -` closes to `1.5/-` where the plain-property `f: 1.50/ -` keeps
`1.5/ -`, while `--q: 1.5 / -` (nothing glued for the arm to read) and `--r: a/ -` (no
font-size operand) take the ordinary rule. Prettier freezes all three, so the rows are tsv
fixed points pinned against a freeze, not against a parse.

The other bounding cells are in the fixture, and these prettier really does parse at a
plain property: the run with an operand after it (`1.50 * -a` → `1.5 * -a`), and a trailing
operator with no operator *before* it (`1.50 -` → `1.5 -`, `1.50 *` → `1.5 *`), where the
whole value is one number and one operator. Both formatters agree on all three.

⚠️ "tsv normalizes to a single form" is true of the **AST path** only. A value carrying a
comment is normalized as text instead (`normalize_value_text`; see the `tsv_css` crate doc's
value-run bullet, "It reaches only the AST path"), where the separator rule does not run — so
`1.50*- /* c */` prints as `1.5*- /* c */`, a second tsv form for the same run. Prettier's
answer to that authoring is a third one, `1.50*-; /* c */`, which moves the comment out past
the declaration's `;`.

No `output_prettier.*`: on tsv's canonical form prettier agrees, so the divergence lives in
the `prettier_variant_*` files — forms prettier keeps stable that tsv normalizes to `input`.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Trailing operator in a value").
