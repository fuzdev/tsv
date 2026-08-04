# range_interior_comment_prettier_divergence

A `<script>` / `<style>` / `<svelte:options>` written **inside** an ignore range is still
lifted to the component root and printed at its canonical position — the range freezes
template *formatting*, it does not pin a section's position, and leaving the section's bytes
in the frozen slice would emit it twice (a component the parser rejects: `Duplicate instance
script found`). Both tools hoist it. They part over the **comment beside it**:

```svelte
<!-- prettier-ignore-start -->
<!-- c -->
<script>
	let a = 1;
</script>
<!-- prettier-ignore-end -->
```

Prettier hoists `<!-- c -->` out with the section, attaching it above the `<script>` and
leaving the range empty. tsv leaves the comment where the author froze it, and hoists only
the section:

```svelte
<!-- prettier-ignore-start -->
<!-- c -->
<!-- prettier-ignore-end -->
```

## Reason

A range marker is an explicit authoring boundary, so a comment inside it is the strongest
form of position-carries-signal: the author did not merely write the comment there, they
fenced the region off. Moving it across that fence is the relocation class tsv declines —
and unlike the section, the comment has no correctness reason to move, since nothing rejects
a comment that stays put.

The divergence is a **normalization** one, not a fixed point: `input.svelte` is stable under
both tools (prettier does not pull an already-hoisted comment back), so there is no
`output_prettier.svelte`. It is the path from the authored form that differs — one source,
two fixed points. `unformatted_ours_section_in_range.svelte` is that source (tsv normalizes
it to `input`, prettier does not), and `variant_comment_hoisted.svelte` is where prettier
lands: a **dual-stable** form, since tsv keeps it too once the comment is already out.

The ordinary hoist — a section inside a range, no comment — matches prettier and needs no
divergence: see the sibling [range_section_hoist](../range_section_hoist/).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
and [§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
