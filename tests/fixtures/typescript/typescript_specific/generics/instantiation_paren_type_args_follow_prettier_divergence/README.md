# instantiation_paren_type_args_follow_prettier_divergence

Prettier strips the paren pair around an instantiation expression that a second type argument list follows; tsv keeps it.

tsv: `new (f<T>)<U>(x)`, `(f<T>)<U>(x)`, `(f<T>)<U>`
Prettier: `new f<T><U>(x)`, `f<T><U>(x)`, `f<T><U>`

## Reason

**Semantic preservation.** This is the `<` member of the follower class [instantiation_paren_follow](../instantiation_paren_follow_prettier_divergence/) pins, reached one level down: the `<` after the close opens a type argument list (of a `new`, a call, a tagged template or another instantiation) rather than a comparison, but the token is the same, and tsc's grammar never reads a first list ahead of it (`canFollowTypeArgumentsInExpression` refuses a `<` unconditionally). So the stripped spellings are other programs or none:

- `f<T><U>(x)` parses under tsc as the comparison `(f < T) > <U>(x)`, whose right operand is the type assertion `<U>(x)`, and `new f<T><U>(x)` as `(new f < T) > <U>(x)` — different programs, with no diagnostic.
- `f<T><U>`x`` is the same comparison over an asserted template.
- `new a.b<T><U>()` and `const a = f<T><U>` do not parse under tsc at all (`Expression expected`) — the assertion `<U>` has no operand — so prettier throws on its own output.

- In a member chain, `f<T><U>(x).y` is the same comparison with `(x).y` asserted; and in a class heritage, bare `extends f<T><U>` parses under neither tsc nor tsv, which read `extends f<T>` and stop at the second `<`.

Paired, every one is the tree acorn-typescript and tsc build alike: the pair settles the first list at its own `)`, leaving the second to be read on its own terms. A redundant pair (`unformatted_ours_double_paren`, `new ((a.b<T>))<U>()`) collapses to one, and an argument-less paired `new` (`unformatted_ours_no_args`, `new (a.b<T>)<U>`) gets its argument list; prettier strips the pair from both.

**The bare spelling tsv repairs.** `f<T><U>` with no argument list is acorn-typescript's instantiation of an instantiation, and a syntax error to tsc (`Expression expected`), so tsv prints it with the pair, `(f<T>)<U>`, the form both read alike. Prettier keeps it bare in a template, where the component's expressions are not read by tsc's parser, so the template's `unformatted_ours_bare_follow` variant bares `{f<T><U>}`: tsv lands on `input.svelte`, prettier on `output_prettier.svelte`. (A bare spelling tsc accepts as a comparison, `f<T><U>(x)`, tsv prints as written instead: [callee_type_args_second_list](../../../expressions/new/callee_type_args_second_list_prettier_divergence/).)

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
