# Decorated class modifier followed by a line break — Svelte Divergence

A class modifier keyword carries a `[no LineTerminator here]` restriction: `declare`
and `abstract` bind to the `class` head only when the head is on the same line.
Break the line and the keyword is an ordinary expression statement instead —
which is exactly how tsv's *undecorated* paths read `declare⏎class A {}` and
`abstract⏎class B {}` (two statements each), matching both acorn and tsc.

Behind a **decorator** the same input has no valid reading at all, because the
decorator is then left with no declaration to attach to. tsc says so directly —
**TS1146 "Declaration expected"**, emitting a `MissingDeclaration` node before the
`declare` expression statement and the class. tsv follows tsc and rejects.

acorn-typescript instead accepts, and the AST it builds is degenerate: an
`ExpressionStatement` for the bare `declare`, plus a `ClassDeclaration` whose span
runs **back over that statement** to the decorator, so the two siblings overlap.
Matching a self-overlapping tree is worse than rejecting, so tsv diverges — the same
call made for the sibling
[async_generic/param_decorator](../../../expressions/arrow/async_generic/param_decorator_svelte_divergence/).

The rule is shared by both modifiers and by the `export` forms; the cases where
acorn agrees that there is no parse are pinned as ordinary `input_invalid_*` files
next door in [declare](../declare/):

- `input_invalid_declare_abstract_line_break.svelte` — `@d declare abstract⏎class B {}`
- `input_invalid_export_declare_line_break.svelte` — `export @d declare⏎class D {}`

The reachable-only-with-a-decorator forms this fixture stands for are
`@d⏎declare⏎class A {}` (pinned here) and `@d abstract⏎class B {}`, which acorn
accepts with the same overlapping shape.

The *two-modifier* spelling needs no decorator to lose its reading: bare
`declare abstract⏎class B {}` is rejected too, since `abstract` binds to `class`
only on the same line. It sits with the rest of that boundary in
[declare/line_break](../../declare/line_break/) — one of the few places tsv refuses
what prettier formats, argued in
[conformance_prettier_ts.md §tsv rejects what prettier formats](../../../../../../docs/conformance_prettier_ts.md#tsv-rejects-what-prettier-formats).

**Upstream candidate**: @sveltejs/acorn-typescript — `canHaveLeadingDecorator`'s
`isDeclareClass` / `isAbstractClass` lookaheads skip line terminators, so a decorator
is admitted in front of a modifier that then fails to bind to the class.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Decorated class modifier line break).
