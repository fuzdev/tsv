# relational_region_prefix_shell_prettier_divergence

From a comma-list element holding a `<` to a later element holding a `>` that may close the type-argument region the `<` opens, a paren pair the author wrote directly behind `typeof` or a unary `-` stays. Prettier strips it, and its own next pass prints the line as a generic call.

tsv: `fn(a < b, typeof (c) > (d, e))`, `fn(a < b, -(1) > (d, e))`
Prettier: `fn(a < b, typeof c > (d, e))`, which its second pass prints as `fn(a<b, typeof c>(d, e))`

## Reason

**Semantic preservation.** A comma is the type-argument separator, and `typeof c` and `-1` are types — a type query and a negative literal type — so once the pair is gone the `<` in one element and the `>` in a later one bound a type-argument list, and the `(` behind the `>` commits it: tsc, acorn-typescript and tsv all read `fn(a < b, typeof c > (d, e))` as `fn` called with ONE argument, the instantiation `a<b, typeof c>` called with `(d, e)`. Written with the pair, `typeof (` is no type, the list cannot be read, and every parser reads the two comparisons the author wrote. `audit_signature.txt` pins prettier's second pass printing the generic call for every cell the hazard reaches, the sequence statement losing its own pair and the template expression its substitution's.

The rule keeps a pair and never adds one. It asks of the source only whether the author wrote the pair; the region is the parser's flag on the later `>`, as for the other pairs of the family, and what follows that `>` is not read. The cells:

- **Carriers** — a call, `new`, an optional call, a method call, an array, a sequence statement, a template expression and a `for` header; the three template cells below the script are the same rule in an expression island, an attribute value and an `{#if}` head.
- **The operand** — a number (`-(1)`, `-(1.5)`), a member, `this`, and a pair that holds only the head of a longer operand: a member object (`typeof (c)[d]`, `typeof (c).d`, `typeof (c.d).e`), a non-null operand (`typeof (c)!`), a callee (`typeof (c)()`, `typeof (c)().d`) and a tag (`` typeof (c)`t` ``). Behind a call or a tag the bare spelling reads the same, and prettier's chain keeps it — the pair is kept by the rule, which asks only whether the author wrote it.
- **Positions** — the shell as the `>`'s own left operand, as an element of its own between the two, behind an arrow, after another element, and in the `<` element itself, right of its `<` (`fn(a < typeof (c), d > (e, f))`, `fn(a < -(1), d > (e, f))`).
- **The follower** — a mixed-arithmetic operand, a template, and a bare name; and a `<` side no list can be read from (`a < b + 1`). Neither line is a generic call bare, so the kept pair is harmless there — the cost of a rule keyed on open regions rather than on what any parser's list grammar takes.

Where a later token already ends the type, the bare and the paired spelling read the same, and each authoring is its own fixed point: `typeof (c) === 's'` beside `typeof c === 's'`, `-(c)` beside `-c`. The controls strip the shell outside a region (no closer, `a <= b`, `a + b`, a nested call, an object literal) and behind `void` and unary `+`, whose operand reads as no type, and behind `!`, which tsc's parser reads as a type prefix with or without the pair, or behind no operator at all; a pair the operand needs stays anywhere.

## Variants

`unformatted_ours_redundant_shell` writes each control with the shell (`fn(a < b, typeof (c))`, `fn(a <= b, typeof (c) > (d, e))`, `fn(a < b, void (c), d > (e, f))`, `fn(a < b, (c) + 1, d > (e, f))`, …); tsv strips each — the half a rule keeping every authored shell would fail.

The cells whose stripped form prettier cannot reparse are [relational_region_prefix_shell_regex_follow](../relational_region_prefix_shell_regex_follow_prettier_divergence/); the pairs tsv leaves out in the same regions are [relational_region_style_pair](../relational_region_style_pair_prettier_divergence/).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair behind `typeof` or a sign") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §The dual-stable remainder, enumerated (item 8).
