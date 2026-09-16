# relational_chain_type_arg_parens_close_follow_prettier_divergence

The follower-token sweep of the relational-chain pair: whatever stands past the `>`, prettier prints the chain bare and tsv keeps a paren pair around the `<` operand.

tsv: `(x < y) > { a: 1 }`
Prettier: `x < y > { a: 1 }`

## Reason

**Semantic preservation.** Printed flat and bare, every cell here is a comparison chain to tsv, acorn-typescript and tsc alike: the token past the would-be closing `>` starts an expression on the `>`'s own line, and no reader takes a type-argument list ahead of that. A line terminator at that join is a different question, and the readers split on it by follower — the split is stated once in the catalog entry linked below. What each cell's bare spelling re-parses as, once the printer breaks after the `>`:

- **An instantiation plus a free-standing statement, to all three** — `a1`–`a7`, `a11`, `a12` and `a13`: an identifier, literal, prefix `!` / `~`, object or `typeof` follower, and every `<` operand shape. `const a11 = x<y>` is followed by a free-standing block (what that block parses as is stated in [relational_chain_type_arg_parens_object_long](../relational_chain_type_arg_parens_object_long_prettier_divergence/)). Both `BinaryExpression` nodes are gone.
- **An index into the instantiation, to all three** — `a10`: `x<y>⏎[0]` is one member expression, `(x<y>)[0]`, with both `BinaryExpression` nodes gone.
- **A `-` / `+` binary over the instantiation, to tsv and acorn-typescript only** — `a8` and `a9`: `x<y>⏎-1` is `(x<y>) - 1`. To tsc the broken spelling is still the chain (the catalog entry states why); the pair is needed here because tsv's own parse reads its output that way, not because tsc does.

So the follower is no protection, and the whole sweep moves together: a string, numeric and bare-identifier `<` operand (`a1`–`a3`); a literal, prefix `!` / `~`, signed numeric, array, object and `typeof` follower (`a4`–`a12`); and an indexed `<` operand, which is as much a type as a reference one (`a13`). tsv puts a pair around the `>`'s left operand instead — the same tree in the spelling every parser reads alike — and both synthesizes and retains it.

The rule is **layout-blind**: whether the `>` ends a line is not knowable where parens are decided, so the pair stands at every width, including the ones that never break. It is also **region-keyed** rather than follower-keyed, which is why the cells that stay bare stay bare: an operand that is no type (`a < arr[b - 1] > c`), and a `>>` / `>>>` run, which is one token the printer cannot split. Those are pinned from the bare side by [relational_lt_vs_type_args](../relational_lt_vs_type_args/).

The as-authored half of the same bug (a break the printer REMOVES rather than adds) is [relational_chain_type_arg_parens](../relational_chain_type_arg_parens_prettier_divergence/); the width-driven half is [relational_chain_type_arg_parens_long](../relational_chain_type_arg_parens_long_prettier_divergence/) and the object-operand half [relational_chain_type_arg_parens_object_long](../relational_chain_type_arg_parens_object_long_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
