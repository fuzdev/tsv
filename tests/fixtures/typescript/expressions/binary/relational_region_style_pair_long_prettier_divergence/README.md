# relational_region_style_pair_long_prettier_divergence

The width half of [relational_region_style_pair](../relational_region_style_pair_prettier_divergence/): the pair tsv leaves out of an arrow function's conditional body or an assignment between a `<` element and a later element's `>` is two columns, so a line of exactly 100 columns holds bare where prettier's paired line would break.

tsv: `fn(lower < upper, (value) => value ? … : fallback, limit > (lower + upper) / 2);` on one line at 100 columns
Prettier: the pair takes the line to 102, and the list breaks one element per line

## Reason

**Semantic preservation.** The pair is text tsc's recovering type-argument parse completes, so prettier's output is one tsc rejects, at every width; see the main fixture. The width follows the printed text: with no pair, the flat line is measured bare.

The cells pair a 100-column line that holds with a 101-column one that breaks: the whole call with the arrow, the whole call with an assignment, the arrow as the last element holding the closer in its own body (at 101 the body hangs below the `=>` in both formatters, where no pair is printed either way), and, inside a list already broken, the arrow's own line, which holds at 100 and hangs its body at 101.

Prettier throws on its own output, so its chain stops at `output_prettier.svelte` — the erroring pass F4b tolerates.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "No pair of the printer's own between the two") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
