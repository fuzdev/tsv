# relational_region_inner_gt_break_long_prettier_divergence

A lone `>` that ends a line settles a type-argument list, so the break a `>` takes inside a `<` region is the one ahead of it.

tsv: `x <⏎(aaa…⏎> bbb…)`
Prettier: `x <⏎(aaa… >⏎bbb…)`, which no longer parses

## Reason

**Semantic preservation.** tsc does not guess at a `<`: it parses a type-argument list for real, with error recovery, and keeps the result errors and all. The list runs from the `<` to the first token the type grammar cannot take — a missing `)`, `]` or `}` is reported and stepped over, and a comma starts the next argument — and when that token is a lone `>` it asks its follower question THERE. A follower past a **line break** commits the list. So `x < (a > b)` is the comparison it looks like while `b` shares the `>`'s line, and `x<(a>` plus wreckage (`')' expected.`) once the printer breaks after the `>`.

The region needs no shell. A comma is the type-argument separator, so a later sibling's `>` closes the region an earlier sibling's `<` opened: `fn(x < q, aaa… >⏎bbb…)` is the generic call `x<q, aaa…>` to tsc, to acorn-typescript and to tsv's own parse, and prettier's output there is one **tsv itself rejects**. Prettier's own second pass throws on every broken cell of `output_prettier.svelte`, so its chain truncates (F4b) and no `audit_signature.txt` exists.

The pair the family's other fixtures put around a chain does not reach this `>`: it ends the region of the OUTER `>` on a `)`, and the inner `>` is met first (`s1`). Nothing between the `<` and the `>` can be reworded without changing the program, and a `(` or a template past the `>` commits on any line, so the one free choice is where the line breaks: ahead of the `>`, its follower sharing its line, is the comparison to every parser at every width.

`input.svelte` pins the layout boundary at both shapes: a line of exactly 100 stays flat (`i1`, the first `fn`), and at 101 the operand breaks ahead of its `>` (`i2`, the second `fn`). `s1`–`s8` sweep what carries tsc's recovery to the `>` — the kept-shell chain with its outer pair, an arrow body, an array head, a `<<`, a conditional's branch, an index, an object member, a block body — and the template cell holds the rule uniform across the component, since `svelte-check` hands that expression to tsc as written.

The rule is a deliberate superset (which `<` regions are still open, never which tokens tsc's recovery takes), so the fixture also pins where a region provably ENDS and the `>` goes back to ending its line like any operator: a statement's body behind a `<` in its head (`for`, `if`), and past the `)` of the call the `<` was written in.

`unformatted_ours_compact` is the one-line authoring of every cell. Prettier's own broken form cannot be a variant: tsv rejects its `fn` cells.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
