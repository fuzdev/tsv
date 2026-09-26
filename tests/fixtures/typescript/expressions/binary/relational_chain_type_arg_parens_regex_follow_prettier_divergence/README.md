# relational_chain_type_arg_parens_regex_follow_prettier_divergence

A comparison chain whose right operand is a regular expression opening with `=` — `f < T > /= c/g` — keeps a paren pair around its `<` operand; prettier prints it bare.

tsv: `g((f < T) > /= c/g)`, `x = (f < T) > /= c/g`
Prettier: `g(f < T > /= c/g)`, `x = f < T > /= c/g`

## Reason

**Semantic preservation.** tsc takes a type argument list in an expression only before a token that cannot start one (`canFollowTypeArgumentsInExpression`), and a same-line `/=` can — as the head of a regular expression literal. So to tsc, and to tsv, `f<T> /= c/g` is the comparison `(f < T) > /= c/g`, whatever its spacing. acorn-typescript — the parser Svelte uses — reads the bare spelling as an instantiation instead, assigned to by `/=` (`f<T> /= (c / g)`), and refuses that as an assignment target except inside a paren or a call argument list, where it skips the check (`maybeInArrowParameters`) and accepts it. That reading is its own miss, not a grammar tsv follows: see [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections) (the entry "A `>`-led compound assignment to an instantiation expression", its `/=` half).

Prettier's bare output keeps the ambiguity: tsc reads it as the input, but inside `g(…)` acorn-typescript reads the instantiation again. tsv's pair around the `<` operand is the spelling every parser reads as the same comparison — the relational-chain rule, which keeps the pair wherever the printed `<`…`>` region would be read as a type argument list.

## Variants

`unformatted_ours_bare_divide_assign` writes the call line bare and glued (`g(f<T> /= c/g)`) — the spelling whose reading tsc and acorn-typescript split on. tsv reads tsc's comparison and normalizes it to `input.svelte`. The Svelte-side reading of that variant has no fixture shape (a variant carries no parse divergence), so tsv's tree is pinned in `tests/nonsimple_assignment_target.rs` (`a_same_line_divide_assign_refuses_the_list`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
