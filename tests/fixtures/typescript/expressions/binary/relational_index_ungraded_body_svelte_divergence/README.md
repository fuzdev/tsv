# Index whose bracketed body is graded no further — Svelte Divergence

**A known over-rejection, not a sanctioned reading.** Every line of `input.svelte` is a
comparison chain to **tsc and to acorn-typescript alike**, and tsv rejects it.

An index inside a `<` region is graded as a type: `f<A[K] | B>(x)` is a generic call,
`f<A[a.b()]>(x)` a comparison chain, and the body of the index is what tells them apart.
A body that opens on `{` or `[` is the one that grade cannot read — the same two heads
the region's own first token cannot be read past
([relational_bracketed_head_ungraded_body](../relational_bracketed_head_ungraded_body_svelte_divergence/)),
one level down — so the region commits on any matching `>` that a line terminator, a `(`
or a template follows, and the type parse then rejects a body whose content is a value.
`tsv_rejects.txt` pins tsv's own error on the first line, and every line is pinned on its
own by `ungraded_index_bodies_stay_rejected` in
[tests/index_type_args_follow.rs](../../../../../index_type_args_follow.rs); the canonical
AST in `expected_svelte.json` is the comparison chain acorn-typescript builds, and tsc
reads the same chain.

The commit is the side the grade has to err on. The same bytes with a TYPE in the
brackets are a generic call to both oracles — `f<A[{ a: 1 }]>(x)`, `f<A[[0]] | B>(x)`,
`f<A[() => {}]>(x)`, pinned by
[less_than_index_type_body](../../../syntax/disambiguation/less_than_index_type_body/) —
and a grade that refused the bracket instead would read each of those as a comparison
chain and reprint it as one: a silent wrong tree on real generic syntax, where this is a
loud parse error on a chain no program compares with.

The `(` body has no such gap — inside an index a paren shell is read by its content,
under both of the lookahead's readings — and an arrow function's body stands where a
function type's return does, so a `{` or `[` there is the same ungraded head
(`a9`, `a10`).

The **printer** side is unaffected: a chain whose region holds such an index and is
followed by a token that does not commit the list (`a < b[{ ...s }] > c`) still parses,
and takes a paren pair around the `>`'s left operand, so tsv never emits one of these
bare. See
[conformance_prettier_ts.md §Relational chain type-argument parens](../../../../../../docs/conformance_prettier_ts.md).

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject), and with no accepted
parse there is nothing for a formatter to claim — hence no `expected.json` and no
format-claim siblings.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (bracketed type-argument head that grades no body).
