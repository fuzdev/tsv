# relational_chain_shift_region_authored_pair_rejected_output_prettier_divergence

[relational_chain_shift_region_authored_pair](../relational_chain_shift_region_authored_pair_prettier_divergence/) where the stripped line is one prettier cannot read back: the pair around the left comparison stays.

tsv: `x1 = (f < A) < C >> /r/.x;`
Prettier: `x1 = f < A < C >> /r/.x;`

## Reason

**Semantic preservation.** Without the pair, `f < A < C >>` bounds a type-argument list. A regex past the `>>` scans as a `/`, a binary operator, which commits the list in tsc, and nothing can follow it there: tsc, acorn-typescript and tsv all reject `x1`. Behind a `[` or a pair the region holds (`x2`, `x3`), tsc's list parse recovers through the bracket to the `>>`, the template commits, and tsc rejects the line; acorn-typescript and tsv read the comparison chain. A `<<` opens the region as a `<` does (`x4`): stripped, ``f << A < C >> `t` `` is a line tsc reads as a type-argument list and rejects. tsv keeps the authored pair, which ends the region, and every parser reads the two comparisons the author wrote.

Prettier throws on its own output, so its chain stops at `output_prettier.svelte` and no `audit_signature.txt` can follow it — the erroring pass F4b tolerates.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
