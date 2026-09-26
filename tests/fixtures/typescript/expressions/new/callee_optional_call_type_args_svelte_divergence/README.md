# Optional call with type arguments after an argument-less `new` — Svelte Divergence

`new f?.<T>(x)` has no valid reading. The `?.` cannot extend the callee of `new`, and
behind the `new` it would chain from an argument-less `new f`, which cannot head an
optional chain either — **tsc rejects with TS1209 "Invalid optional chain from new
expression"**, and so does prettier in a `<script>` body, where its TypeScript parse is tsc's.
In a template prettier accepts it and prints `{new f()?.<T>(x)}`, inventing the argument list
as acorn-typescript does.

## Why tsv Differs

**Acorn-typescript accepts**, building a `ChainExpression` over an optional
`CallExpression` whose callee is `new f` with an empty `arguments` list — the tree of
`new f()?.<T>(x)`, a program with an argument list the author never wrote. Inside a
`new` callee its `parseSubscript` stops ahead of a `?.<` rather than refusing it, so the
optional call attaches to the finished `new f` outside; the same position without type
arguments, `new f?.(x)`, it rejects ("Optional chaining cannot appear in the callee of new
expressions").

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for validity
the oracle is tsc, which rejects, as prettier does in a `<script>` body. A tree that
supplies an argument list tsc treats as a syntax error — and that prettier's template path
then prints as though the author had written it — is worse than no tree, the same call made
for the type-argument-free spelling and for its mirror `new f<T>?.(x)` (type arguments
first, which acorn also rejects —
[callee_type_args_follow](../callee_type_args_follow/)). tsv rejects all three.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
