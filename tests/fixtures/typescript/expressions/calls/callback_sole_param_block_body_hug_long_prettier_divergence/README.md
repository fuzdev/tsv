# callback_sole_param_block_body_hug_long_prettier_divergence

A call argument that is a block-bodied arrow (or `function`) whose sole parameter carries an
object type authored flat but too wide to fit flat on the broken-out argument line. tsv counts
that width break as authored, so the object type expands inside the hug the block body keeps —
the form [callback_sole_param_block_body_hug](../callback_sole_param_block_body_hug_prettier_divergence/)
pins for an authored expansion — as the last argument of a plain call, a `new` expression and a
member chain of one or two calls, after a preceding argument, as the first argument
(`reduce(…, '')`), with a return type, for a sole destructured parameter, holding an empty
nested object type (it has nothing to break), and in a template expression.

The window is exact and the fixture pins its boundary on that line wherever it ends or sits — after
a preceding argument, at an empty body's `}`, at the first argument's comma, at a `function`
callback's object return type's `{` (the group around its lone parameter measures the line only
that far), and a level deeper in an expanded member chain: at 100 the broken-out argument line
holds the flat object type, so both formatters break the arguments out and keep it flat (the
`function`'s return type breaks instead); at 101 tsv hugs. Controls outside it: an expression body
breaks every argument out in both formatters, a lone `function` callback hugs in both, so does one
under `new` (whose hug keeps the parameters breakable, so the object type breaks inside it), and an
object type holding a nested object or mapped type — in a member, or annotating a nested function
type's parameter — breaks every argument out in both: the nested type breaks for width there, and
that break, read back as authored, refuses the hug.

Prettier prints the hug's parameters only when the flat signature fits on the callee line, so
here it breaks every argument out and the object type, inside the broken-out argument, breaks for
width. Read back by tsv, that break is an authored expansion and takes the hug — so prettier's
form is not a tsv fixed point, and printing it from the flat authoring would give one document
two tsv forms. tsv prints the hug in one pass instead. `unformatted_ours_flat_literal` is the
flat authoring (its expanded object type gains a `;` per member, so it is not a whitespace-only
variant); prettier's output of it is `output_prettier`.

- **tsv**: `fn(lead, (a: {⏎\tb: C;⏎\td: E…;⏎}) => {⏎\tg(a);⏎});`
- **Prettier**: `fn(⏎\tlead,⏎\t(a: {⏎\t\tb: C;⏎\t\td: E…;⏎\t}) => {⏎\t\tg(a);⏎\t}⏎);`

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Hugged sole-parameter object type.
