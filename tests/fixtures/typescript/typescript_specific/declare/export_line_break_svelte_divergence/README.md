# `export declare` split from its declaration — Svelte Divergence

`declare` carries a `[no LineTerminator here]`: the declaration it marks must begin
on its line. Behind `export` a break therefore has no valid reading, because `export`
is then left with nothing to attach to — **tsc rejects with TS1128 "Declaration or
statement expected"**, and so does prettier.

The rule is uniform across every head `declare` takes — `class`, `function`,
`namespace`, `enum`, `const`, `interface`, `type`, `abstract class` — all eight are
TS1128 in tsc and all are rejected by prettier. This fixture pins one; the parser
applies the check once, at the post-`declare` dispatch, so the heads cannot drift
apart.

## Why tsv Differs

**Acorn-typescript accepts**, welding across the break into a single
`ExportNamedDeclaration` whose `ClassDeclaration` carries `declare: true` — the same
tree it builds for the same-line spelling, so the line break simply vanishes. tsv
produced that tree too until this rejection landed.

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for
validity the oracle is tsc, which rejects, and prettier — the accept test — rejects
as well. A tree that silently discards a line terminator both other oracles treat as
fatal is worse than no tree, the same call made for the decorated sibling
[decorators/declare_line_break](../../decorators/declare_line_break_svelte_divergence/),
where acorn's accepted tree is self-overlapping.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## The boundary

Only the modifier→declaration gap is restricted; `export`'s own gap is not. All of
these stay accepted, in tsv and prettier alike:

- `export declare class B {}` — same line throughout
- `export⏎declare class B {}` — the break is after `export`, which carries no
  `[no LineTerminator here]`
- `declare⏎class B {}` — no `export`, so `declare` demotes to an expression statement
  and the class stands alone (two statements, matching acorn and tsc)

The statement-path forms of this boundary, where the break makes the modifier an
ordinary identifier statement rather than an error, are
[declare/line_break](../line_break/) — which also pins the `export declare abstract⏎class`
spelling, since `abstract` enforces the same rule one token later.
