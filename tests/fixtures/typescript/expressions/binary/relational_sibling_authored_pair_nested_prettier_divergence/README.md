# relational_sibling_authored_pair_nested_prettier_divergence

A paren pair the author wrote around a comparison inside a comma-list element stays when a later element of the same list holds a `>` that may close the type-argument region the `<` opens. Prettier strips the pair; where the comparison ends the element, its own next pass prints the line as a generic call.

tsv: `xs.filter((x) => (x < max), (y) => y > (lo + hi) / 2)`
Prettier: `xs.filter((x) => x < max, (y) => y > (lo + hi) / 2)`, which its second pass prints as `xs.filter((x) => x<max, (y) => y>(lo + hi) / 2)`

## Reason

**Semantic preservation.** Where the comparison is the last thing the element prints, its right operand is the element's last token, so once the pair is gone nothing ends the region at the element's comma: the `<` and the later `>` bound one type-argument list (`max, (y) => y` is a type, a function type among them), and the `(` behind the `>` commits it. tsc, acorn-typescript and tsv read the stripped line as `xs.filter` called with ONE argument. The pair ends the region where the author's `)` stands, and every parser reads the two arrows the author wrote. `audit_signature.txt` pins prettier's second pass printing the generic call for those cells.

This is the whole-element rule of [relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/) one level in, and the rule is the same at every depth: inside the region, a pair around a node holding a `<` or `<<` prints iff the author wrote it, and none is added. Where the stripped comparison would close before the comma (`(c < d) ? e : f`, `(c < d) || e`) prettier's form still reads as the two comparisons, but the line is the author's and tsv keeps it; where it would be the left operand of another `<` or `<<` (`ok = (1 << bits) < limit, value >> …`), tsc's type grammar reads a nested list or closes the `>>` as two `>`s, and the stripped line is one tsc rejects. The cells:

- **What the element ends on** — an arrow function's expression body, with its parameter in a pair or bare; a binary's right operand under `&&`, `||`, `??` and `==`; a conditional's alternate; an assignment's value; a curried arrow's innermost body; a `yield` operand.
- **Anywhere else in the element** — a shift that is a comparison's left operand (the realistic `check(ok = (1 << bits) < limit, value >> (bits - 8))`), a conditional's test, a `||`'s left operand, a conditional's consequent.
- **Every pair on the way down** — an arrow body pair around `a && (x < max)` keeps both.
- **Positions and carriers** — the element after another element and ahead of one, a `new` call and an array.

The assignment cells print no pair around the assignment: inside the region a pair of the printer's own is left out ([relational_region_style_pair](../relational_region_style_pair_prettier_divergence/)), so the author's pair around the comparison is the one that ends the region. Prettier adds that assignment pair instead (`fn((x = c < d), …)`), which ends the region too, so those cells read back as written in prettier's chain.

The controls: the same lines written with no pair print with none; a comparison inside a nested call's argument list or an array of its own opens no region the later `>` closes, so its pair follows the ordinary rules and strips; and a pair with no closer later in the list strips.

## Variants

`unformatted_ours_redundant_pair` writes the bracketed controls with their pair (`fn(g(h, (c < d)), …)`, `fn([(c < d)], …)`), the no-closer control as `xs.filter((x) => (x < max), (y) => y)`, and every cell on one line; tsv strips each control's pair. Prettier's first pass from it lands on a form `audit_signature.txt` documents.

A `<<` comparison where the element ends, and the left operand of another comparison in the cells prettier's stripped form cannot reparse, are pinned in `tests/relational_sibling_authored_pair.rs` rather than here.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §The dual-stable remainder, enumerated (item 8).
