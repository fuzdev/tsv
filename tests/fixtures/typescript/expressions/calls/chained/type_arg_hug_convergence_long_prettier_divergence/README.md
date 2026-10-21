# type_arg_hug_convergence_long_prettier_divergence

A member chain whose last call's sole type argument is an object type (alone, or in a union with
`null` or `void`) or a mapped type written flat, too wide to fit flat on the expanded chain's line
that carries it, while the chain's head and the type argument's `{` fit on the first line. Both
formatters share one fixed point — the chain stays flat and the type literal breaks inside its
`<…>` (the hug) — but prettier needs 2 passes to reach it from the flat authoring; tsv
converges in one.

## Pattern

Source (flat): `const rows1 = db.where(filter).select<{ …wide… }>(columns);`

- **Prettier pass 1** (unstable): the flat type literal carries no forced break, so the chain's
  one-line measurement reads the whole flat content, overflows, and the chain expands — the
  type literal, unfittable on the expanded chain's line too, breaks for width inside it.
- **Prettier pass 2** (stable): the now-multiline type literal re-reads as authored-expanded
  (its newline after `{`), the chain's one-line measurement truncates at that forced break and
  sees only the line up to the `{`, which fits — the chain collapses back to flat with the type
  literal hugging.

tsv prints the pass-2 form directly, in every position the chain can sit — a declarator, a
call argument, a property value, a returned `await`, a statement whose short head merges into
the chain's first line, and a Svelte template expression — for an object type, a mapped type
and an object type in a union with `null`, across an optional link, a non-null one and a
`new` head, for a call with no arguments, for a last call taking an arrow — a
block-bodied one too, whose own break comes after the type literal — and for a comment-bearing
object argument, which no object-argument hug takes; a later call in the group with type
arguments of its own leaves the first typed call's type argument to decide.
`unformatted_ours_flat` is the flat authoring (ours normalizes in one pass; prettier's first
pass is pinned by `prettier_intermediate_flat`); `unformatted_expanded` is that first pass as
an authoring, the broken chain, which both formatters take to `input` in one pass.

The window is exact and the fixture pins both of its boundaries: at 100 the expanded chain's
line holds the type literal flat up to the call's `(` — the arguments break out below it — so
the broken chain is the shared stable form (one char wider and the chain hugs); with no
arguments that line runs on through the `();`, and the same 100/101 pair holds there, as it
does with a comment inside the empty argument list (which breaks it like an argument list, so
the line ends at the `(` again), and with a comment between the call's name and its `<` or
after an optional call's `?.` (both part of that line); at the other end, at 100 the hugged
first line fits, while one char wider it does not, and both formatters keep the broken chain
with the type literal broken inside it.
Controls outside it: an earlier call taking a function argument keeps the broken chain in both
formatters (prettier's `lastGroupWillBreakAndOtherCallsHaveFunctionArguments`), and a type
literal inside a reference argument does not hug the `<`, so its `<…>` breaks and the literal
stays flat inside it. A memberless type literal (`{} | null`) has nothing to break, and the
chain stays flat in both.

## Reason

Prettier bug (non-idempotent): from the flat authoring prettier's first pass is not a fixed
point. tsv prints prettier's own settled form in one pass — a convergence-speed divergence with
a single authoring-independent fixed point.

See [conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript) (Member-chain type-argument hug convergence).
