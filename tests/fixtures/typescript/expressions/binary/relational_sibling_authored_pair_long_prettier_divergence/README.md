# relational_sibling_authored_pair_long_prettier_divergence

The width half of [relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/): a paired comparison ahead of a later element's closer keeps its pair whether the list fits or breaks, and when the paired comparison itself is too wide, it breaks inside the pair.

tsv: `fn(⏎\t(a < b),⏎\tc > (d, eee…)⏎)`, `fn(⏎\t(aaa… <⏎\t\tbbb…),⏎\td > (e, f)⏎)`
Prettier: `fn(a < b, c > (d, eee…))`, `fn(⏎\taaa… <⏎\t\tbbb…,⏎\td > (e, f)⏎)`

## Reason

**Semantic preservation**, as in the main fixture: stripped, the `<` and the later element's `>` bound one type-argument list and the `(` behind the `>` commits it. A line break changes nothing there — the type grammar reads across it — so every cell is the generic call to tsc, acorn-typescript and tsv once the pair is gone, and `audit_signature.txt` pins prettier's second pass printing it: `fn(a<b, c>(d, eee…))`, and for the broken comparison `aaa…<⏎bbb…,⏎d⏎>(e, f)`, with `d` pulled into the list.

The boundary is tsv's, and the pair is in it: the first `fn` is exactly 100 columns and stays on one line; the second is one `e` longer, so its list breaks one element a line with the pair intact. Prettier's stripped line is two columns shorter, so it keeps both on one line. In the third cell the paired comparison is wider than a line on its own: the comparison breaks after its `<`, inside the pair, and the `)` closes on the right operand's line.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling").
