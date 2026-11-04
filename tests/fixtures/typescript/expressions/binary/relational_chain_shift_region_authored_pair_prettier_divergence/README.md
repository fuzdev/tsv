# relational_chain_shift_region_authored_pair_prettier_divergence

A `<` whose left operand holds a `<` or `<<` and whose right operand holds a `>>` or `>>>` the parser flagged as able to close the region the two open keeps the pairs the author wrote in both operands. Prettier strips them, and its own next pass prints the line as a generic call.

tsv: ``x1 = (f < A) < C >> `t`;``
Prettier: ``x1 = f < A < C >> `t`;``, which its second pass prints as ``x1 = f<A<C>>`t`;``

## Reason

**Semantic preservation.** The tree is ``(f < A) < (C >> `t`)``: two comparisons with a shift on the right. Bare, `f < A < C >>` is `f<A<C>>`, a type-argument list that the `>>` closes, and the template behind it commits the list: tsc, acorn-typescript and tsv all read ``f < A < C >> `t` `` as the tagged template ``f<A<C>>`t` ``. The author's pair is the `)` that ends the region. A sequence, an assignment pair and an arrow behind the `>>` commit it the same way, and a `>>>` closes three regions (`x4`). The region may be bounded by an inner link of the chain: in `x15` the outer `<` holds no closer, and the pair still stays. A comment inside or beside the pair stays where it was written (`x16`, `x17`). `audit_signature.txt` pins prettier's second pass printing the generic call.

The cells whose stripped line no parser can follow live in [relational_chain_shift_region_authored_pair_rejected_output](../relational_chain_shift_region_authored_pair_rejected_output_prettier_divergence/).

This is the comma-list owner's rule ([relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/), [relational_region_prefix_shell](../relational_region_prefix_shell_prettier_divergence/)) over one comparison chain: between the region's opener and its closer, every pair the printer would decide follows the source. So the pair behind `typeof` stays too, in either operand (`x9`, `x18`; bare, `typeof b` is a type query and the line a generic call).

The rule keeps a pair and never adds one, and it reads neither what follows the `>>` nor what the right operand holds ahead of it. `x10`, `x11` and `x6`–`x8` are its cost: the pair behind the unary `-`, the one ahead of a plain follower, and the ones ahead of a right operand whose closer stands behind a call, a binary pair or a unary (`g(C) >>`, `(C * D) >>`, `-C >>`) are harmless (`-b` is no type, nothing commits the list, and a call or an arithmetic pair is no type argument either, so the would-be list is refused), bare all read as written, and tsv keeps the authored pair anyway. `x12` writes no pair and gets none. `x13`, `x14` and `x20` hold no closer (a `>>>` behind two `<` closes no region), so a redundant pair strips.

## Variants

`unformatted_ours_double_pair` writes the kept pairs doubled; tsv prints one.

`unformatted_ours_redundant_pair` writes the controls `x13`, `x14` and `x20` with a pair around the left comparison; tsv strips it, as prettier does.

`unformatted_ours_inside_pair` writes `x17`'s comment inside the pair (`(f < A /* c */)`); tsv prints it past the `)`, the lossless move a trailing block comment takes at a pair's close.

Prettier lands every variant on `output_prettier.svelte`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
