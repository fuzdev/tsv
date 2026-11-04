# `readonly` over a tuple with an index behind it reads as a member access

`fn<readonly [A]['k']>(x)` is a generic call to tsc, whose parser takes `readonly` over any
type and leaves the array-or-tuple rule to its checker (TS1354). acorn-typescript refuses
`readonly` over the indexed access while parsing the type arguments and falls back to a
comparison chain, where `readonly[A]['k']` is a member access on the name `readonly` — a
reading that parses, so acorn-typescript (and Svelte) ACCEPTS the line as
`fn < readonly[A]['k'] > x`. tsv reads it as acorn does: the drop-in wire is acorn's on every
input acorn accepts, and tsc's reading of the bare line is an error. The rule is the
parenthesized operand's (`a < readonly (d)[e] > (f)`): a same-line index behind the operand
leaves it a value reading unless the run holds an empty `[]`, which no member access spells
(`fn<readonly [A]['k'][]>(x)` stays a call to every parser).

tsv prints the comparison inside the relational pair (`(fn < readonly[A]['k']) > x`), which
every parser reads as the comparison: tsc reads the bare line as the call where a `(` or a
template follows the `>`, and the printed-form grade does not read the follower, so the pair
stands whatever follows it. The bare comparison (`unformatted_ours_type_spelling`'s
`fn < readonly[A][0] > (x, y)`) normalizes to it.

Prettier formats TypeScript with tsc's parser, so it keeps the call spelling
(`prettier_variant_type_spelling`, whose wire pin `expected_prettier_variant_type_spelling.json`
is acorn's comparison) and strips the pair (`output_prettier.svelte`), which at
`fn < readonly[A][0] > (x, y)` leaves a line tsc reads as the call and acorn-typescript as the
comparison.

A line whose comparison reading then fails is rejected, as acorn-typescript rejects it: a
tuple or parenthesized body that is no expression (`[a: A]`, `[A?]`, `[...A]`, `[A[]]`,
`[keyof A]`, `(A[])`, `(keyof A)`), a run followed by a conditional's `extends`, or a follower
no comparison takes (an empty argument list, `fn<readonly [A]['k']>()`). tsc's parser reads
each as the call and leaves TS1354 to its checker. The `input_invalid_*` files pin those
rejects; see [checklist_typescript.md](../../../../../../docs/checklist_typescript.md).

See [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript).
