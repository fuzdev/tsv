# instantiation_paren_assertion_follow_prettier_divergence

Prettier strips the paren pair around an instantiation expression ahead of `as` / `satisfies`; tsv keeps it.

tsv: `(f<T>) as T`, `(f<T>) satisfies T`, `(-f<T>) as T`
Prettier: `f<T> as T`, `f<T> satisfies T`, `-f<T> as T`

## Reason

**Semantic preservation.** Whether a `>` closes a type argument list is decided by the token after it, and the two parsers answer differently for a word. tsc takes the list ahead of any binary operator (`canFollowTypeArgumentsInExpression`), and `as` / `satisfies` are binary operators to it, so the bare `f<T> as T` is the assertion over the instantiation. acorn-typescript — the parser Svelte compiles with, and tsv's parse oracle — gives the list up ahead of any token that can start an expression on the same line, and a word can, so to it the close is a comparison: `f<T> as T` reads as `f < T > as` followed by a stray `T` and does not parse, and a type the expression grammar can also read — `f<T> as [T]`, `f<T> as -1`, `` f<T> as `t` `` — reads as a different program (`f < T > as[T]`), which tsv's next pass then prints as that comparison. Prettier, whose TypeScript parse is tsc's, strips the pair, so a `<script lang="ts">` or a template expression it formats stops compiling (in the template, prettier's own next pass throws on it, which F4b tolerates). tsv keeps the pair — over a member, call, nested-list or sealed-chain head alike (`((a?.b)<T>) as T`) — the spelling every parser reads as the input meant — the same rule the pair already follows ahead of `+`, `<` or `!` ([instantiation_paren_follow](../instantiation_paren_follow_prettier_divergence/)).

The axis is the join of two tokens, not the operand's node: an operand that merely ends on the close — a prefix operator's argument (`-f<T>`, `typeof f<T>`) — re-lexes the same way, so the pair wraps that whole operand, and a pair authored around the instantiation alone moves out to it (`unformatted_ours_inner_pair`). A binary operand of `as` takes its pair in both formatters already (`(a * f<T>) as T`), and an operand that does not end on the close (`f<T>(a) satisfies T`) prints bare in both.

The input's cells are the ones whose bare spelling no parser but tsc accepts, so prettier's form is inert under tsv. The misreading spellings (`as [T]`, `as -1`, `` as `t` ``) cannot be fixture cells: prettier's form of them parses under tsv as the comparison, so it is no tsv fixed point, which `authoring:audit` requires of `output_prettier.*`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
