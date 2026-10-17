# relational_region_arrow_param_pair_prettier_divergence

Between a comma-list element holding a `<` and a later element holding a `>` that may close the type-argument region the `<` opens, an arrow function's lone parameter the author wrote without a pair prints without one. Prettier adds the pair, and its output is one tsc rejects.

tsv: `xs.filter(x => x < max, y => y > (lo + hi) / 2)`, `check(lo < hi, [x => x > (lo + hi) / 2])`, `fn(a < b, { q: c => c > (d, e) })`
Prettier: `xs.filter((x) => x < max, (y) => y > (lo + hi) / 2)`, `check(lo < hi, [(x) => x > (lo + hi) / 2])`, `fn(a < b, { q: (c) => c > (d, e) })`

## Reason

**Semantic preservation.** A comma is the type-argument separator, so tsc reads past the `<` as a type-argument list wherever its recovering parse can complete one. `(x) => x` is a function type's spelling — a parameter list, an arrow and a return type. As a list element of its own the paired arrow completes the list outright: `xs.filter((x) => x < max, (y) => y > (lo + hi) / 2)` is `xs.filter` called with ONE argument, the generic call `x<max, (y) => y>(lo + hi)`, to tsc, to acorn-typescript and to tsv alike, which prettier's own next pass prints as `x<max`. Inside the array it reads as a tuple of one function type, inside the object literal as a type literal's member, and the `>` behind it closes the list: with the pair, tsc reads a generic call and rejects the line (`',' expected`, `';' expected`), where acorn-typescript and tsv still read the comparisons. Without the pair, `x =>` starts no type, and every parser reads the comparisons the author wrote. Prettier parses TypeScript with tsc's parser, so it throws on its own output: the chain stops at `output_prettier.svelte` and no `audit_signature.txt` can follow it — the erroring pass F4b tolerates.

The pair around a lone parameter is the printer's style, never the tree's. The rule adds none in the region and keeps the spelling the author wrote: a parameter written in a pair keeps it, which is what holds a list element's arrow (`fn(a < b, (c) => c ? d : e, f > (g, h))`) a line tsv's own parse reads as comparisons. The region is the parser's flag on the later `>`, and the rule reads nothing past it. The cells:

- **Carriers** — a call, `new`, an optional call, an array, an object literal's value, a sequence statement, a template expression holding the sequence, and a template substitution holding the arrow; the three template cells below the script are the same rule in an expression island, an attribute value and an `{#if}` head.
- **Positions** — a list element of its own, between the two comparisons or holding the closer (the realistic `xs.filter(x => x < max, y => y > (lo + hi) / 2)`, and the same arrow inside an array between them); the arrow holding the closer in its own body inside an array or object, standing ahead of the closer's element in the same array or object, after another element, curried, and behind `async`.
- **What is not read** — the closer's follower (a mixed-arithmetic operand, a template, a bare name) and a `<` side no list can be read from (`a < b + 1`). Behind a bare name or past `a < b + 1` tsc's recovery completes no generic call, so the pair would be harmless there; the rule is keyed on open regions rather than on what tsc's lookahead takes, a superset with one fixed point.

The controls hold the pair where no later element holds a closer (`c`, `c >= d`), where no element opens a region (`a <= b`, `a + b`, a `<` inside a nested call, a closer only ahead of the comparison) and in an object literal whose `:` stands between the members.

## Variants

`unformatted_ours_compact` is the space-free authoring of every cell. `unformatted_ours_bare_param` writes each control's parameter bare (`fn(a < b, [c => c])`, `fn(a <= b, [c => c > (d, e)])`, `const b1 = { p: a < b, q: c => c > (d, e) }`, …); tsv adds the pair to each, as everywhere outside a region — the half a rule following the source in every list holding a `<`, or treating an object literal as a list, would fail.

The pairs tsv leaves out of an arrow function's conditional body and around an assignment in the same regions are [relational_region_style_pair](../relational_region_style_pair_prettier_divergence/).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "No pair of the printer's own between the two") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
