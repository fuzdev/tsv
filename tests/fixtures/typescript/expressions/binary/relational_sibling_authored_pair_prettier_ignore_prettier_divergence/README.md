# relational_sibling_authored_pair_prettier_ignore_prettier_divergence

A `// prettier-ignore` on the paired element of [relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/): the frozen element prints verbatim and its authored pair prints once around it, in a call, an array and a `for` header. Prettier's frozen slice is the node alone, so it prints the element without its pair.

tsv: `fn(⏎\t// prettier-ignore⏎\t(a  <  b),⏎\tc > (d, e)⏎)`
Prettier: `fn(⏎\t// prettier-ignore⏎\ta  <  b,⏎\tc > (d, e)⏎)`

## Reason

**Semantic preservation.** The verbatim slice is `a  <  b`, and the pair is not part of it, so prettier's output is the main fixture's stripped line with the author's spacing: `a  <  b,` and `c > (d, e)` bound one type-argument list, and tsc, acorn-typescript and tsv read the call as `fn` called with the generic call `a<b, c>(d, e)`. Prettier's second pass reprints the same bytes: the directive now leads that generic call, which spans both lines, and freezes it.

The last cell is a `for` header whose frozen operand holds a bare `in` (`x < y || p in q`). Under a `for` header that operand needs a pair of its own, so the header does not read as a for-in head ([init_clause_prettier_ignore_bare_in](../../../statements/for/init_clause_prettier_ignore_bare_in_prettier_divergence/)), and the authored pair is that pair too: tsv prints ONE pair around the slice, never a second one for the `in`. Prettier's bare slice gives a header no parser accepts (`')' expected`), so prettier throws on its own output and the chain stops at `output_prettier.svelte` — the erroring pass F4b tolerates, and the reason no `audit_signature.txt` follows it.

The `for` header layout — the operands after the first on an indented line once a directive stands between them — is prettier's own, matched by tsv.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling") and [conformance_prettier_ignore.md](../../../../../../docs/conformance_prettier_ignore.md) §Format-ignore directive.
