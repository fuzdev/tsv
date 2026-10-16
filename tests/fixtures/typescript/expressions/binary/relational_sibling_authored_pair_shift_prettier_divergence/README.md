# relational_sibling_authored_pair_shift_prettier_divergence

[relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/) with shifts: a `<<` in the paired element and a `>>` in a later one. The pair stays; prettier strips it, and its stripped output is one tsc's parser rejects.

tsv: `fn((a << b), c >> (d, e))`
Prettier: `fn(a << b, c >> (d, e))`

## Reason

**Semantic preservation.** tsc re-scans a `<<` to a `<` where it looks for type arguments, and a `>>` closes two of the regions that opens, so to tsc's parser the stripped line starts a type-argument list it cannot finish (`'(' expected`). acorn-typescript and tsv read that line as two shift expressions, so the stripped form is a split between the parsers, not a second program; the authored pair ends the first element before the comma and every parser reads the shifts the author wrote. The cells are a call and an array.

Prettier parses TypeScript with tsc's parser, so it throws on its own output: the chain stops at `output_prettier.svelte` and no `audit_signature.txt` can follow it — the erroring pass F4b tolerates. Keeping these cells out of the main fixture is what lets that fixture pin prettier's full chain.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
