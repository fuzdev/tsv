# `readonly` over a parenthesized or template operand reads as a value

The parenthesized twin of
[call_type_args_readonly_tuple_index](../call_type_args_readonly_tuple_index_prettier_divergence/).
`fn<readonly (A)['k']>(x)` is a generic call to tsc, whose parser takes `readonly` over any
type and leaves the array-or-tuple rule to its checker (TS1354). acorn-typescript refuses
`readonly` over the parenthesized type and falls back to a comparison chain, where
`readonly(A)['k']` is a call on the name `readonly` — a reading that parses, so
acorn-typescript (and Svelte) accepts the line as `fn < readonly(A)['k'] > x`; with no index
(`readonly(A)`) the same. A template is the one literal head a value spells
(`` readonly`t`[0] ``, a tagged template), and reads the same way. tsv reads each as acorn
does; the array-suffix control (`fn<readonly (A | B)['k'][]>(x)`) is a call to every parser.

tsv prints the comparison inside the relational pair (`(fn < readonly(A)['k']) > (x, y)`),
which every parser reads as the comparison: tsc reads the bare line as the call where a `(`
or a template follows the `>`, and the printed-form grade does not read the follower, so the
pair stands whatever follows it — at `> x` too, where prettier prints the line bare.

Prettier formats TypeScript with tsc's parser. Its first pass strips the pair
(`output_prettier.svelte`), leaving lines tsc reads as the call at a `(` or template
follower; its second pass writes that call, with the type paren stripped
(`fn<readonly A['k']>(x, y)`, pinned in `audit_signature.txt`), a line acorn-typescript and
Svelte reject.

See [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript).
