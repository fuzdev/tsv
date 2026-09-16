# relational_chain_type_arg_parens_object_long_prettier_divergence

A relational chain whose right operand is an object literal reaches the same re-lex, and a worse end: prettier's output does not parse.

tsv: `(x < y) >⏎{⏎\ta: 1,⏎⏎\tb: 2⏎}`
Prettier: `x <⏎y >⏎{⏎\ta: 1,⏎⏎\tb: 2⏎}`, which no parser accepts — prettier's own second pass throws `';' expected`

## Reason

**Semantic preservation.** Ahead of an object, a line terminator after the `>` settles the list as type arguments, to tsv, acorn-typescript and tsc alike (which followers a break commits, and for which parser, is stated once in the catalog entry linked below). An object literal supplies that break two ways, and both are here: a blank line inside it forces the break at every width (`c1`), and width drops it below the `>` line (`c3`). What is left after the re-lex is a `TSInstantiationExpression` followed by a free-standing block, and the object's member count decides what that block is:

- **one member** — `x<y>;⏎{ a: 1 };` parses, as a `BlockStatement` holding a labelled statement: a different program, not an error.
- **several members** — `x<y>;⏎{ kkk: 1, lll: 2 };` is rejected (`Expected ';'`): output that does not parse.

Every cell here has several members, so its output fails to re-parse outright rather than quietly becoming a different program.

That truncation is why this fixture carries no `audit_signature.txt`: prettier throws on its own pass-1 output, so there is no chain to pin (rule F4b tolerates an erroring pass).

tsv keeps a pair around the `>`'s left operand, which takes the region out of type-argument position whatever the layout. The pair is **layout-blind**, so `c2` — which never breaks at the join — carries it too. `input.svelte` pins the **post-fix layout boundary**: with the pair, `c2`'s continuation line is exactly 100 columns and keeps the object welded to the `>`; `c3`'s would be 101 and drops it.

## Where the loss begins

The **bug boundary** is two columns below that, on the bare spelling, and no cell of `input.svelte` can carry it — the pair costs two columns, so a bare-100 cell's paired form is 102 and already broken. Measured on the bare spelling, one two-key object on a `const` continuation line:

- **100 columns** — `x < y > { kkk…(38): 1, lll…(37): 2 };` stays welded, and re-parses as the comparison chain it was written as: `BinaryExpression >` over `BinaryExpression <`, the `ObjectExpression` on the right.
- **101 columns** — `x < y > { kkk…(39): 1, lll…(37): 2 };` breaks all three operands, the object landing on its own line, and the output does not re-parse at all — Svelte's parser reports `Unexpected token`, prettier's own second pass `';' expected` — because what is left is a `TSInstantiationExpression` followed by a block whose `,`-separated members are no statement.

`output_prettier.svelte`'s own width cells sit under that line: bare, `c2` measures 98 and `c3` 99, both welded and both still the comparison chain, so neither shows the loss on its own. `c1` needs no width for it — the blank line inside the object forces the break at every column, which is why `output_prettier.svelte` as a whole fails to re-parse.

The plain-identifier half of the same width bug is [relational_chain_type_arg_parens_long](../relational_chain_type_arg_parens_long_prettier_divergence/); the as-authored half, where the printer REMOVES a break instead of adding one, is [relational_chain_type_arg_parens](../relational_chain_type_arg_parens_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
