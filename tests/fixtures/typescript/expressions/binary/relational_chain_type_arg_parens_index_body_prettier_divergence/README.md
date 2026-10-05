# relational_chain_type_arg_parens_index_body_prettier_divergence

Prettier prints a relational chain bare whatever its `<` operand's index holds; tsv keeps a paren pair around the `<` operand wherever the index could read as a type, since the region would then parse as a type-argument list.

tsv: `(a < b[() => c]) > d`
Prettier: `a < b[() => c] > d`

## Reason

**Semantic preservation**, the rule of [relational_chain_type_arg_parens](../relational_chain_type_arg_parens_prettier_divergence/) carried through an index. `b[() => c]`, `b[typeof c]` and `b[{ c: 1 }]` are member accesses in the chain and indexed-access types in a type-argument list, and which of the two a parser reads is settled by what follows the `>`: past a line break tsv's own parse (with acorn-typescript) commits the list, so the bare chain is an instantiation plus a free-standing statement the moment a width puts a break there. The pair around the `>`'s left operand is the same tree in the spelling every parser reads alike.

The rule is **region-keyed**, so the index is read as the region is. A body the type grammar spells takes the pair (`a1`–`a7`): a union, a type query, a reference with type arguments, a function type, an import type. A `{` or `[` body (`b1`–`b4`) takes it whatever it holds, as the same two delimiters do at the head of the region: tsv's parse grades neither body, so `a < b[{ ...s }] >⏎c` is a type-argument list to it, and a bare output would be one it cannot reparse. (The over-rejection that follows from it is the parser's — [relational_index_ungraded_body](../relational_index_ungraded_body_svelte_divergence/).)

## How far the body is read

No further than its **first operand**. Where a token that continues no type follows it — to tsv's own parse and to acorn-typescript's — the body is an expression and the chain stays bare (`c1`–`c9`): arithmetic, a call, a sequence, an assignment, an arrow function returning one, a unary operand, a shift, a conditional, a dynamic import. Four of those bare chains are still regions **tsc** claims once a line break follows the `>`: a shelled sequence or assignment (`c3`, `c4` — the compiler reads the shell as a parameter list, where the two spellings of one index owe tsv one verdict), a shift (`c7`, which its type parser re-scans into two `<`) and a dynamic import (`c9`, an import type to the compiler whatever its specifier). tsv follows acorn-typescript on all four, as it does on the non-null `!` inside an index.

Where the token that follows COULD continue a type — a `<`, a `|` or `&`, a `[`, a `.` — the rest of the index is taken as it stands, and so is the whole body behind a string key, a `keyof` or a `typeof` (`d1`–`d12`). Those chains take the pair although several of their bodies are plainly expressions, and for two reasons a finer reading cannot get around:

- **The printer moves the pairs a finer reading would have to match.** The operands of a comparison in the body take whichever pair their position asks for (`b[c < d > e]` prints as `b[(c < d) > e]`, `b[c < d | e > []]` as `b[(c < d) | (e > [])]`), and those pairs move the delimiters a nested argument list is matched by. Which printed spelling still closes one is not readable from the source.
- **tsc claims more than a type.** Its type-argument parse is error-recovering, so a body that merely STARTS like a type is a region the compiler claims past a line break and then rejects — `b[c.d!]`, `b[keyof]`, `b[import.meta]`, `b[c | (d, e)]` — where acorn-typescript and tsv read the chain.

A pair the chain did not need is noise; a bare chain a reader of the output then claims is a different program, or none. So the reading errs toward the pair, and — reading nothing past the first operand — reaches one verdict on both spellings of a body.

## Variants

`unformatted_ours_body_shell` moves a paren shell the printer does not keep where the author put it: one it strips, around a whole body (`b[(c | d)]`, `b[(() => c)]`, `b[({ c: 1 })]`, `b[(c + 1)]`, `b[(c < d)]`), around a `typeof`'s operand (`typeof (c)`) and around a name ahead of what follows it (`(c)<D>`, `(c).d()`, `(c) | d()`); and one it adds, around a sequence, an assignment, the operands of a nested comparison and a numeric literal ahead of a member (`b[c, d]`, `b[c = d]`, `b[c < d > e]`, `b[c < d | e > []]`, `b[1..c]`). Inside an index a pair is never the operand's own, so the two spellings of each body are one document and owe one verdict.

`unformatted_ours_bare_arrow_parameter` writes an arrow function's lone parameter bare (`b[c => c.d]`). The printer wraps it, which is what makes the printed body a function type's spelling, so the pair is decided for the form the printer emits.

`unformatted_ours_call_shaped` writes the `e` chains the way a generic call is spelled — `f<A[a.b()]>(x)`, `` f<A[a.b()]>`t` `` — where the follower would commit a type-argument list on any line. The index's body is an expression, so tsv's parse, acorn-typescript and tsc all read the comparison chain, and the chain then takes whichever pair its body does.

Prettier drops tsv's pair as it does everywhere in this family, so only tsv normalizes any of the three back to `input.svelte`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
