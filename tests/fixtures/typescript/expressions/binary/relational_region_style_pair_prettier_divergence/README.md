# relational_region_style_pair_prettier_divergence

Between a comma-list element holding a `<` and a later element holding a `>` that may close the type-argument region the `<` opens, an arrow function's conditional body and an assignment the author wrote without a pair print without the one both formatters otherwise give them. Prettier adds the pair, and its output is one tsc rejects.

tsv: `fn(a < b, (c) => c ? d : e, f > (g, h))`, `fn(a < b, c = d, e > (f, g))`
Prettier: `fn(a < b, (c) => (c ? d : e), f > (g, h))`, `fn(a < b, (c = d), e > (f, g))`

## Reason

**Semantic preservation.** A comma is the type-argument separator, so tsc reads past the `<` as a type-argument list wherever its recovering parse can complete one, and a pair the printer invents is text that recovery can complete: `(c ? d : e)` opens like a function type's parameter list (`c?`, then `d: e`), and `(c = d)` like a parameter with an initializer. With the pair, tsc reads the line as a generic call missing its tokens and rejects it (`',' expected`, `'=>' expected`); acorn-typescript and tsv still read the two comparisons. As written, without the pair, every parser reads the comparisons. Prettier parses TypeScript with tsc's parser, so it throws on its own output: the chain stops at `output_prettier.svelte` and no `audit_signature.txt` can follow it — the erroring pass F4b tolerates.

The pair is the printer's style, never the tree's: an arrow's concise body and a list element are each a whole assignment expression, so the bare spellings are the same program. Between the element holding the `<` and the element holding the closer, the closer's own element included, tsv prints each pair only where the author wrote it, and the rule reads the parser's flag on that `>`, never what follows it. The cells:

- **Carriers** — a call, `new`, an optional call, a method call, an array, a sequence statement, a template expression and a `for` header; the three template cells below the script are the same rule in an expression island, an attribute value and an `{#if}` head.
- **The arrow** — with no parameter, a typed one, curried, and with the closer inside its own conditional body.
- **The assignment** — as an arrow body, an array element, a template substitution, and with an array or object pattern target.
- **Positions** — an element between the `<` and the shape, the shape between the closer and a later element, the shape inside an array, a `<` against a member and under `&&`.
- **What is not read** — the closer's follower (a mixed-arithmetic operand, a template, a bare name), what the conditional holds (a literal branch, a member test) and the assignment's operator (`+=`). tsc's recovery completes none of `fn(a < b, (c) => (c ? d : e), f > g)`, `(c ? 1 : 2)`, `(c.d ? e : f)` or `(c += d)`, so their pairs would be harmless; the rule is keyed on open regions rather than on what tsc's lookahead takes, a superset with one fixed point.
- **The flag, not the list** — a `<` side no list can be read from (`a < b + 1`) and an authored pair around the comparison leave the parser's flag set, so the pairs are left out there too.

The controls hold the pairs where no later element holds a closer (`c`, `f >= g`), where no element opens a region (`a <= b`, `a + b`, a closer only ahead of the comparison, a `<` inside a nested call) and in an object literal, whose `:` stands between the members. A pair the tree needs stays in the region (`(c = d) + 1`, `(c = d) ? e : f`), and a conditional or an arrow body that takes no pair anywhere is untouched.

## Variants

`unformatted_ours_compact` is the space-free authoring of every cell. Each cell written WITH the pair is no variant here — tsc rejects those lines as written, so prettier cannot read the file — and its fixed point, printed as written, is pinned in `tests/relational_region_pairs.rs`.

The width half is [relational_region_style_pair_long](../relational_region_style_pair_long_prettier_divergence/); the arrow parameter's pair is [relational_region_arrow_param_pair](../relational_region_arrow_param_pair_prettier_divergence/), and the pair the author wrote behind `typeof` or a unary `-` is [relational_region_prefix_shell](../relational_region_prefix_shell_prettier_divergence/).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "No pair of the printer's own between the two") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
