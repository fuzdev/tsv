# relational_region_inner_gt_clarity_pair_recovered_list_prettier_divergence

The cells of [relational_region_inner_gt_clarity_pair](../relational_region_inner_gt_clarity_pair_prettier_divergence/) where prettier's output is one tsc rejects: the `>` sits inside a pair or a bracket the region holds, or the region opened on a `<<`.

tsv: `fn(a < b, (c > await d) as T)`
Prettier: `fn(a < b, (c > (await d)) as T)`, which tsc rejects (`')' expected.`)

## Reason

**Semantic preservation.** tsc's type-argument parse is `parseDelimitedList(TypeArguments, parseType)` with error recovery: a `)` or `]` the list is missing is reported and stepped over, and the parse stops at the first token the type grammar cannot take. A `>` inside a pair is therefore still that token, and a `(` directly behind it commits the list — errors and all. acorn-typescript backs off a failed list and reads the comparisons, so the two oracles part on prettier's output: a compiling `<script lang="ts">` stops type-checking. Prettier's own TypeScript parser is tsc's, so its second pass throws on `output_prettier.svelte`; the chain truncates (F4b) and no `audit_signature.txt` exists.

The bare operand is the same answer as in the sibling fixture, for the same `>`: nothing ahead of it can be reworded, and the clarity pair behind it is the one token the tree does not need. The cells are the pairs and brackets that can stand around the comparison — an assertion's operand pair (`as`, `satisfies`), a union's, a prefix operator's, an array — and the regions a `<<` opens. tsc re-scans it to a `<` and reads the `<` left behind as a nested list's, so a `>>` closes both, and `fn(a << b, c >> (await d))` is tsc's reject alone. Behind a `<` of its own the two nest under it and a `>>>` closes all three: `fn(a < b << c, d >>> (await e))` is one tsv's own parse rejects too.

`unformatted_ours_compact` is the space-free authoring of every cell.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
