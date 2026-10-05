# relational_region_inner_gt_clarity_pair_long_prettier_divergence

The width half of [relational_region_inner_gt_clarity_pair](../relational_region_inner_gt_clarity_pair_prettier_divergence/): a `>` that may close a type-argument region leads its line, and the operand behind it prints bare on that line.

tsv: `fn(⏎a < b,⏎ccc…⏎> await ddd…⏎)`
Prettier: `fn(⏎a < b,⏎ccc… >⏎(await ddd…)⏎)`, which is the call `fn(a<b, ccc…>(await ddd…))`

## Reason

**Semantic preservation.** Two rules meet on one `>`, and both are about what stands directly behind it. A line break there commits the list ([relational_region_inner_gt_break_long](../relational_region_inner_gt_break_long_prettier_divergence/)), so the comparison breaks AHEAD of its `>`; a `(` there commits it on any line, so the operand's clarity pair is left out. Prettier takes the break after the `>` and prints the pair, and `audit_signature.txt` pins its second pass printing each broken cell as the generic call.

`input.svelte` pins the boundary at both levels. The whole call at exactly 100 stays flat and at 101 gives each argument a line; the comparison's own line at exactly 100 stays flat and at 101 breaks ahead of the `>`, the `await` bare behind it. The bare spelling is what is measured: prettier's flat form is two characters wider, so its 100-wide cell is already broken. A mixed-arithmetic operand takes the same two widths.

A `>>` behind two nested `<` closes both lists — the type grammar scans a `>` at a time — so it takes both answers too: 100 flat, 101 broken ahead of the `>>` with the operand bare. Behind a single `<` it closes nothing, ends its line as any operator does, and its operand keeps the clarity pair (the last cell).

`unformatted_ours_compact` is the one-line authoring of every cell.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
