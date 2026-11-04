# this_predicate_operand_parens_prettier_divergence

A `this is T` predicate in a type position other than a return type keeps its paren pair wherever a token follows it, and takes one as a last union member or a `keyof` operand written bare (`C | this is B` prints as `C | (this is B)`). Prettier strips the pair, which changes the type.

tsv: `type A1 = (this is B)[];`
Prettier: `type A1 = this is B[];`

## Reason

**Semantic preservation.** tsc reads `this is` as a predicate anywhere a primary type stands (`parseThisTypePredicate`), and the predicate's type is a whole `Type`, so it runs on to the right as a function type's return does. Bare, `this is B[]` is the predicate over `B[]`, not an array of predicates; `this is B | C` takes the union, `this is B extends C ? D : E` the conditional, and `[this is B?]` the optional element's `?`. tsc and acorn-typescript both read the stripped output as that other type. Prettier's own AST has no paren node, and its paren rules have no arm for a predicate.

tsv gives a predicate with a type the pair a function type takes, at the same positions: an array element, an indexed-access object, a union or intersection member, a conditional's check type, an optional tuple element, and a `keyof` or `unique` operand. Under `readonly` the predicate prints bare and the pair goes around the `readonly` type where a token follows it, since acorn-typescript reads `readonly (` only ahead of an array suffix. A last union member (`C | (this is B)`) and a `keyof` operand (`keyof (this is B)`) read alike bare only where nothing follows the whole type: `keyof (this is B) | C` stripped is `keyof (this is B | C)`, and `C | (this is B) extends D ? E : F` a predicate over the conditional. So tsv gives them the pair a function type takes there, which prettier gives a function type too, written or not: `C | this is B` prints as `C | (this is B)`. One fixed point per document. Where the predicate ends the type — a function type's return, a type argument, a conditional's extends type — the pair strips in both formatters.

A predicate in these positions is a checker error (TS1228), but the parse is valid, and a formatter must not change what it parses to.

## Variants

`unformatted_ours_double_parens` writes every kept pair doubled; tsv prints one, prettier none.

`unformatted_ours_control_parens` writes the right-open controls with a pair; both formatters strip it, and prettier also strips the pairs the other cells keep.

`unformatted_ours_bare_member` writes the last member and the `keyof` operand bare (`C1`, `C2`); tsv adds the pair, prettier keeps them bare.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../docs/conformance_prettier_ts.md) §TypeScript (This-predicate operand parens).
