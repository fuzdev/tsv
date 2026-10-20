# mapped_type_arg_hug_convergence_long_prettier_divergence

An assignment value that is a short call whose single type argument is a mapped type the author
did not break after its `{`, too wide for the call to fit on its own line after `=` and for its
argument list to hold the mapped type flat there. Both formatters share one fixed point — the
call hugs the `=` and the mapped type breaks inside its `<…>` — but prettier needs 2 passes to
reach it from such an authoring; tsv converges in one.

## Pattern

Source: `const v1 = w.f…<{ readonly [K in X]: 1 }>(x);`

- **Prettier pass 1** (unstable): a mapped type is a complex type argument only when its printed
  `<…>` will break, and an unbroken one does not, so the call reads as poorly breakable and the
  value breaks after `=` — where the mapped type, unfittable on that line too, breaks for width.
- **Prettier pass 2** (stable): the mapped type now breaks right after its `{`, which forces it
  open, so the type arguments read as complex, the `=` keeps the call on its line and the
  mapped type breaks inside the hug.

tsv prints the pass-2 form directly — in a declarator, an exported declarator, an assignment
and a class field; for a member callee and a plain one, and under an `await`; with an argument
and with none. `unformatted_ours_flat` is the flat authoring and
`unformatted_ours_member_newline` breaks the mapped type inside its member only, which forces
nothing in either formatter (ours normalizes both in one pass; prettier's first pass, the same
for both, is pinned by `prettier_intermediate_flat`).

The window is exact and the fixture pins both of its boundaries: at 100 the argument list after
the `=` break holds the mapped type flat, so both formatters break the `=` and the argument
(one char wider and the call hugs); at the other end, at 100 the hugged call's first line fits,
while one char wider it does not, and both formatters break the `=` with the mapped type broken
after it.
Controls outside it: a `new` callee and an object type argument hug the `=` in both
formatters in one pass, and a property value has no `=` to break after.

## Reason

Prettier bug (non-idempotent): from an authoring that does not break the mapped type after its
`{`, prettier's first pass is not a fixed point. tsv prints prettier's own settled form in one
pass — a convergence-speed divergence with a single authoring-independent fixed point.

See [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Mapped type argument hug convergence).
