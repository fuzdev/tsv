# relational_region_prefix_shell_long_prettier_divergence

The width half of [relational_region_prefix_shell](../relational_region_prefix_shell_prettier_divergence/): the pair tsv keeps behind `typeof` or a unary `-` between a `<` element and a later element's `>` is two columns, so a line of exactly 100 columns holds with it and one more column breaks the list, where prettier's stripped line still fits.

tsv: `fn(lower < upper, typeof (xxx…), limit > (lower + upper) / 2);` on one line at 100 columns, broken one element per line at 101
Prettier: `fn(lower < upper, typeof xxx…, limit > (lower + upper) / 2);` on one line at both widths, which its second pass prints as the generic call `fn(lower<upper, typeof xxx…, limit>(lower + upper) / 2)`

## Reason

**Semantic preservation.** Without the pair, `typeof xxx…` and `-1` are types and the `<` and the `>` bound a type-argument list the `(` behind the `>` commits; see the main fixture. The width follows the printed text: the kept pair is measured, so the list breaks one column later than it would bare. Broken one element per line, the `>` still closes the region on every parser's reading, and the kept pair still keeps the two comparisons.

The cells pair a 100-column line that holds with a 101-column one that breaks, behind `typeof` and behind a unary `-` on a number. `audit_signature.txt` pins prettier's second pass printing each line as a generic call.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair behind `typeof` or a sign") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
