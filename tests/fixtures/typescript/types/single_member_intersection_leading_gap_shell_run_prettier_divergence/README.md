# single_member_intersection_leading_gap_shell_run_prettier_divergence

The one-member face of
[intersection_leading_gap_shell_run](../intersection_leading_gap_shell_run_prettier_divergence/):
a **leading-operator** intersection (or union) with a single member, whose leading gap holds
a block comment and whose member is a redundant paren shell holding a leading `//`.

tsv, at `A`:

```ts
type A = q< /* c1 */ // d1
	B
>;
```

## Reason: with one member the operator is dropped, so its gap is the enclosing gap's

A one-member composite prints as its member — the `&` / `|` never renders — so a comment
between that operator and the member lands exactly where the operator-less authoring would
put it: in the ENCLOSING gap. Two windows, contiguous in source, one run, one emitter.

The leading-edge seam (`Printer::head_stripped_paren_shell`) used to decline any
leading-operator composite whose gap held a comment, because a **multi-member** intersection
emits that gap itself and a claim over only the shell would hand the enclosing gap bytes
nobody partitioned to it. With one member there is no such emitter: the composite's own head
region opens at the operator (`HeadShell::region_start`) and the composite stands down
(`Printer::composite_head_region_claimed`).

Left declined, the run had no gap that knew the position's indent. The intersection's
first-member hoist relocated it to wherever the intersection happened to be built and added
the continuation indent that aligns a type-alias `=` — one level too many at every position
here — while the reparse, reading the comments from the enclosing gap, printed the position's
own. The two passes disagreed: an F1 violation, not a divergence, at all six.

`O` is the union spelling, which collapses by the same rule.

## Prettier

Prettier drops the run onto its own line below the delimiter where tsv keeps it on the line
the author glued it to — the placement
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
catalogs for the type-argument `<`, the tuple `[` and the union member gap. At `C` it goes
further and relocates `/* c2 */` **before the `:`** and `// d2` past the `;`
(`p /* c2 */: D; // d2`), re-binding both comments; tsv keeps each where it was written.

## Files

`unformatted_ours_leading_operator.svelte` is the authored form — the operator, the gap
comment and the shell all present; tsv normalizes it to `input.svelte`.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
