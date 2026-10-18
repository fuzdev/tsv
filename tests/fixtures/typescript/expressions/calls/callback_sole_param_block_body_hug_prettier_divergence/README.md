# callback_sole_param_block_body_hug_prettier_divergence

A call argument that is a block-bodied arrow whose sole parameter carries an authored-expanded
object type. tsv keeps the expansion, and the arrow keeps the hug — as the last argument of a
plain call, a `new` expression and a member chain, after a preceding argument, with a return
type, and as the first argument (`reduce(…, '')`). The same holds for the object type annotating
a sole destructured parameter, and for a `function` callback's hugged sole parameter.

Prettier drops the authored break of an object type annotating a hugged sole parameter, so its
object collapses and the arrow hugs. tsv keeps the break and, for a block body, the hug too: the
object breaks inside the hugged parameter list, the shape a function declaration with the same
parameter prints. That hug is tsv's choice, not prettier's rule read on the kept break:
prettier's `ArgExpansionBailout` (`function-parameters.js`) asks only whether the signature will
break, never what the body is, so on that break it would break these calls out too — as tsv does
for an arrow with an expression body
([callback_signature_break_expands_call](../callback_signature_break_expands_call_prettier_divergence/),
[callback_signature_break_hugged_body_expands_call](../callback_signature_break_hugged_body_expands_call_prettier_divergence/)).

- **tsv**: `fn((a: {⏎\tb: C;⏎}) => {⏎\tg(a);⏎});`
- **Prettier**: `fn((a: { b: C }) => {⏎\tg(a);⏎});`

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Hugged sole-parameter object type.
