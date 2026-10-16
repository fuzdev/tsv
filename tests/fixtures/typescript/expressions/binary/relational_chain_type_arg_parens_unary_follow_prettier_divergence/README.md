# relational_chain_type_arg_parens_unary_follow_prettier_divergence

A comparison chain whose right operand opens with a unary `+` or `-` — `f < T > +c` — keeps a paren pair around its `<` operand; prettier prints it bare.

tsv: `a1 = (f < T) > +c`, `a4 = (new f() < T) > -c`
Prettier: `a1 = f < T > +c`, `a4 = new f() < T > -c`

## Reason

**Semantic preservation.** tsc takes a type argument list in an expression only where the next token can follow one (`canFollowTypeArgumentsInExpression`), and it refuses `+` and `-` there before it asks about a line break: in that position they are unary operators, not binary ones. So to tsc, and to tsv, `f<T>⏎+c` is the comparison `(f < T) > +c` whatever its spacing, after a name, a member, a `new` callee or inside an argument list. acorn-typescript — the parser Svelte uses — agrees on one line but holds the list past a line break, reading `(f<T>) + c`: an instantiation added to `c`, which a type strip turns into `f + c`. That reading is its own miss, not a grammar tsv follows: see [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections) (the entry "A `<`, `+` or `-` after a would-be type-argument close").

Prettier's bare output keeps the ambiguity: tsc reads it as the input, but once a break lands past the `>`, acorn-typescript reads the instantiation again. tsv's pair around the `<` operand is the spelling every parser reads as the same comparison — the relational-chain rule.

The `b` cells bound the class from the other side: past a line break every other follower keeps the list — a free-standing next statement, a call, a tagged template, a binary operator — for all three parsers, and both formatters print them alike.

## Variants

`unformatted_ours_line_break` writes every cell bare with the follower on its own line (`f<T>⏎+c`) — the spelling whose reading tsc and acorn-typescript split on. tsv reads tsc's comparison and normalizes it to `input.svelte`; prettier lands on its bare chain. The Svelte-side reading of that variant has no fixture shape (a variant carries no parse divergence), so tsv's tree is pinned in `tests/instantiation_line_break_unary_follow.rs`.

`unformatted_ours_same_line` writes the `a` cells bare on one line (`f<T> + c`), where all three parsers already read the comparison.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
