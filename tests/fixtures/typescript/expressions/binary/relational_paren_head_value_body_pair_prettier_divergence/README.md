# relational_paren_head_value_body_pair_prettier_divergence

The relational-chain pair over a `(` type-argument head whose body is a VALUE, at a follower that commits the region: prettier prints the chain bare, tsv keeps a pair around the `<` operand.

tsv: `(x < (a > b)) > (t, u)`
Prettier: `x < (a > b) > (t, u)`

## Reason

**Semantic preservation.** The same rule as [relational_chain_type_arg_parens_kept_shell](../relational_chain_type_arg_parens_kept_shell_prettier_divergence/), read at the cells where the follower — a `(` or a template — commits the region with no line break anywhere. Each body here is one tsc abandons and acorn-typescript reads as the comparison chain, so the bare authoring parses and the pair is the printer's insurance: the printed region opens on a `(` the printer keeps, and a `(`-headed region is a type-argument list to any reader that grades bracket matching and the follow token rather than the body.

- **An unmatched `>`** (`p1`–`p3`): a `>` that closes no nested argument list ends the would-be region early, ahead of a follower that continues the comparison; `>>` there is a shift outright.
- **An arrow with a bare-name parameter** (`p4`–`p6`): no function type spells `=>` behind anything but a parameter list's `)`. Prettier parenthesizes the parameter and prints `x < ((a) => b) > (t, u)`, which IS a generic call over a parenthesized function type — a different program, and the one its own second pass then prints as `x<(a) => b>(t, u)`.
- **A regex literal** (`p7`, `p8`): no type begins with a `/`, and the pattern may spell a `<`…`>` of its own.
- **`infer` with no binding name** (`p9`): an ordinary value name.

## Variants

`unformatted_ours_bare_chain` drops the outer pair — and, where prettier added them, the arrow parameter's parens — which is the authoring the divergence is about. tsv normalizes it back to `input.svelte`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
