# instantiation divide-assignment target in a call argument - Svelte divergence

`g(f<T> /= c)`: acorn-typescript — the parser Svelte uses — reads `f<T>` as a type argument
list and builds an `AssignmentExpression` whose `left` is a `TSInstantiationExpression`,
which `expected_svelte.json` records. Outside a paren or a call argument list it rejects the
same statement (`Assigning to rvalue`), but that is its assignment-target early error, and
inside one it skips that check (the list may still be an arrow's parameters,
`maybeInArrowParameters`).

## Why tsv differs

tsc's **grammar** refuses the list. A type argument list in an expression is taken only
where the next token can follow one (`canFollowTypeArgumentsInExpression`): on the same line
that is a binary operator or a token that cannot start an expression, and a `/=` can — it is
the head of a regular expression literal. So tsc reads the comparison `f < T >` followed by
the literal `/= c);`, which the line ends before it closes (TS1161 `Unterminated regular
expression literal.`). That is a production refusal, not the deferred early error of
`f<T> += c` ([nonsimple_target](../../../expressions/assignment/nonsimple_target_svelte_divergence/)),
so tsv reads what tsc reads and rejects — the `>>=` sibling is
[instantiation_shift_assign_call_arg](../instantiation_shift_assign_call_arg_svelte_divergence/),
and the bare statement is an `input_invalid_*` file of
[instantiation_operator_follow](../instantiation_operator_follow/). The parenthesized
`(f<T>) /= c` is the spelling every grammar derives, and tsv keeps its pair
([instantiation_target_paren](../../../expressions/assignment/instantiation_target_paren_svelte_prettier_divergence/)).
`tsv_rejects.txt` pins tsv's error.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
