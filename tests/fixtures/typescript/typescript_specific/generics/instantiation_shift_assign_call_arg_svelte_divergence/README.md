# instantiation shift-assignment target in a call argument - Svelte divergence

`g(f<T> >>= c)`: acorn-typescript — the parser Svelte uses — reads `f<T>` as a type
argument list and builds an `AssignmentExpression` whose `left` is a
`TSInstantiationExpression`, which `expected_svelte.json` records. Outside a paren or a call
argument list it rejects the same statement (`Assigning to rvalue`), but that is its
assignment-target early error, and inside one it skips that check: it cannot yet tell
whether the list is an arrow's parameters (`maybeInArrowParameters`), and never runs the
check once the list turns out not to be one.

## Why tsv differs

tsc's **grammar** refuses the reading. A type argument list in an expression is taken
only where the next token can follow one (`canFollowTypeArgumentsInExpression`), and a
`>`-led token never can — the close and the `>` would be ambiguous with a re-scanned `>>`.
So there is no instantiation to assign to: the `<…>` falls back to the comparison
`f < T >`, which has no right operand where `>>=` stands (TS1109 `Expression expected.`).
That is a production refusal, not the deferred early error tsv takes for `f<T> += c`
([nonsimple_target](../../../expressions/assignment/nonsimple_target_svelte_divergence/)),
so tsv rejects, as it does the bare statement
([instantiation_operator_follow](../instantiation_operator_follow/)'s `input_invalid_*`
files). The parenthesized `(f<T>) >>= c` is the spelling every grammar derives, and tsv
keeps its pair ([instantiation_target_paren](../../../expressions/assignment/instantiation_target_paren_svelte_prettier_divergence/)).
`tsv_rejects.txt` pins tsv's error.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
