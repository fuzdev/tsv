# relational_sibling_authored_pair_prettier_divergence

A comma-list element the author wrote in a paren pair keeps it when the element holds a `<` and a later element of the same list holds a `>` that may close the type-argument region the `<` opens. Prettier strips the pair, and its own next pass prints the line as a generic call.

tsv: `fn((a < b), c > (d, e))`
Prettier: `fn(a < b, c > (d, e))`, which its second pass prints as `fn(a<b, c>(d, e))`

## Reason

**Semantic preservation.** A comma is the type-argument separator, so once the pair is gone the `<` in one element and the `>` in a later one bound a single list, and the `(` behind the `>` commits it: tsc, acorn-typescript and tsv all read `fn(a < b, c > (d, e))` as `fn` called with ONE argument, the instantiation `a<b, c>` called with `(d, e)`. As the author wrote it, the `)` of the pair ends the first element and the line is two comparisons. `audit_signature.txt` pins prettier's second pass printing the generic call for every cell, the sequence statement losing its own pair (`a<b, c>(d, e);`) and the template expression its substitution's.

The rule keeps a pair and never adds one. It asks three things of the source: the author wrote the pair, the element is not the list's last (a trailing comma prints away, so the index decides, not the next byte), and a later element holds a `>`, `>>` or `>>>` the parser flagged as able to close a region. It does not read what follows that `>`. The cells:

- **Carriers** — a call, `new`, an optional call, a method call, an array, a sequence statement, a `for` header, a template expression, and a dynamic `import()`, whose source holds the pair and whose options argument holds the closer; the three template cells below the script are the same rule in an expression island, an attribute value and an `{#if}` head.
- **Positions** — the paired element after another element, an element between it and the closer, an element after the closer, and two paired elements ahead of one closer (`fn((a < b), (c < d), e > (f, g))`), each of which keeps its own pair.
- **Inside the paired element** — the comparison against a member or a literal, with a member on its left, under `&&`, and a `<` chain whose two regions one `>>` closes.
- **Behind the closer** — a mixed-arithmetic operand, a template, an `await` with and without its clarity pair: the follower is not read, and prettier adds the clarity pair to the bare `await`, which commits the list on its second pass just the same.
- **The closer deeper in its element** — under `&&` and in an arrow's concise body.

`fn((a < b), c > d)` and `fn(a < b, c > d)` are both fixed points: nothing commits a list past `d`, so the bare line is two comparisons and the kept pair is harmless — the cost of a rule keyed on open regions rather than on what follows the `>`. The controls strip a redundant pair where no later element holds a closer (`c`, `c >= d`, `() => c`, a closer only ahead of the comparison), where the element opens no region (`a <= b`, `a + b`), and inside a nested call, whose `)` ends the region. A generic call written as one, `fn(a<b, c>(d, e))`, stays a call.

## Variants

`unformatted_ours_compact` is the space-free authoring of every cell.

`unformatted_ours_double_pair` writes every kept pair doubled (`((a < b))`); tsv prints one.

`unformatted_ours_redundant_pair` writes the controls with a pair around the element (`fn((a < b), c)`, `fn(c > (d, e), (a < b))`, `fn((a <= b), c > (d, e))`, `fn(g((a < b)), c > (d, e))`, …); tsv strips each — the half a rule keeping every authored pair would fail.

Prettier lands every variant on `output_prettier.svelte`, so the chain `audit_signature.txt` pins covers them too.

The width half is [relational_sibling_authored_pair_long](../relational_sibling_authored_pair_long_prettier_divergence/); the cells whose stripped form prettier cannot reparse are [relational_sibling_authored_pair_regex_follow](../relational_sibling_authored_pair_regex_follow_prettier_divergence/) and [relational_sibling_authored_pair_shift](../relational_sibling_authored_pair_shift_prettier_divergence/); comments around a kept pair are [relational_sibling_authored_pair_comment](../relational_sibling_authored_pair_comment_prettier_divergence/), and a frozen paired element is [relational_sibling_authored_pair_prettier_ignore](../relational_sibling_authored_pair_prettier_ignore_prettier_divergence/).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §The dual-stable remainder, enumerated (item 8).
