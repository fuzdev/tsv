# relational_chain_type_arg_parens_update_operand_prettier_divergence

A comparison chain whose `<` operand ends in a postfix update — `!b++ < T > z` — keeps a paren pair around that operand; prettier prints it bare.

tsv: `a1 = (!b++ < T) > z`, `a4 = (<U>b++ < T) > z`
Prettier: `a1 = !b++ < T > z`, `a4 = <U>b++ < T > z`

## Reason

**Semantic preservation.** The pair is the relational-chain rule's, the same one [relational_chain_type_arg_parens](../relational_chain_type_arg_parens_prettier_divergence/) states: bare, the `<`…`>` region reads as a type-argument list to a parser that asks only what the region holds. The cells are the operands the rule meets where no list can stand at all. A postfix update is an `UpdateExpression` (`LeftHandSideExpression [no LineTerminator here] ++`), and type arguments, a call, a member access and a tagged template continue only a `LeftHandSideExpression` — so `!b++⏎<T>(z)` is the comparison `(!b++ < T) > (z)` to tsc and acorn-typescript alike, on one line or two, under a prefix operator, `typeof` or an angle-bracket assertion as on its own (`a5`), and `<T>b++⏎(z)` is two statements (`c1`).

## Variants

`unformatted_ours_line_break` writes the `a` cells bare with the `<T>` on its own line and drops `c1`'s `;`; `unformatted_ours_same_line` writes the `a` cells bare on one line. Both normalize to `input.svelte` under tsv, prettier landing on its bare chain, and each carries a parse pin (`expected_unformatted_ours_*.json`) holding the canonical parser's reading — a comparison over the update, never a call.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
