# mapped_type_arg_hug_call_callee_long_prettier_divergence

The [mapped_type_arg_hug_convergence_long](../mapped_type_arg_hug_convergence_long_prettier_divergence/)
window for a callee rooted in a call that does not print as a member chain: an assignment value
that is a short call whose single type argument is a mapped type the author did not break after
its `{`, too wide for the call to fit on its own line after `=` and for its argument list to hold
the mapped type flat there, under a callee like `w().f…`. Both formatters share one fixed point —
the call hugs the `=` and the mapped type breaks inside its `<…>` — but prettier needs 2 passes
to reach it from such an authoring; tsv converges in one.

## Pattern

Source: `const v1 = w().f…<{ readonly [K in X]: 1 }>(x);`

- **Prettier pass 1** (unstable): an unbroken mapped type is not a complex type argument, so the
  call reads as poorly breakable and the value breaks after `=` — where the mapped type,
  unfittable on that line too, breaks for width.
- **Prettier pass 2** (stable): the mapped type now breaks right after its `{`, which forces it
  open, so the type arguments read as complex, the `=` keeps the call on its line and the mapped
  type breaks inside the hug.

tsv prints the settled form directly — in a declarator, an exported declarator, an assignment
and a class field; for a callee rooted in a plain call, a call with an argument, a `this` member
call, a non-null callee and a curried call; under an `await`; with an argument and with none.
`unformatted_ours_flat` is the flat authoring (ours normalizes in one pass; prettier's first
pass is pinned by `prettier_intermediate_flat`).

The window is exact and the fixture pins both of its boundaries: at 100 the argument list after
the `=` break holds the mapped type flat, so both formatters break the `=` and the argument (one
char wider and the call hugs); at the other end, at 100 the hugged call's first line fits, while
one char wider it does not, and both formatters break the `=` with the mapped type broken after
it. A callee rooted in a call with an argument has one more way to keep the hug there: one char
past that boundary its root call's argument breaks and the call still hugs the `=`.
Beside it: a member callee with no call in it (the base window,
[mapped_type_arg_hug_convergence_long](../mapped_type_arg_hug_convergence_long_prettier_divergence/))
and a member-chain callee (whose first prettier pass expands the chain) settle on the same hug,
which tsv prints in one pass; a property value has no `=` to break after.

## Reason

Prettier bug (non-idempotent): from an authoring that does not break the mapped type after its
`{`, prettier's first pass is not a fixed point. tsv prints prettier's own settled form in one
pass — a convergence-speed divergence with a single authoring-independent fixed point.

See [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Mapped type argument hug convergence).
