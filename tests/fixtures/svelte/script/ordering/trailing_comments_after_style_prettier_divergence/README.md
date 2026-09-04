# trailing_comments_after_style_prettier_divergence

A comment written after the last `<style>` — the end of the source — when the template has
real content. The canonical reorder prints the `<style>` last, so the comment is hoisted to
the template's end, where it sits directly above the `<style>`: on the next pass that is a
section-leading comment, and the section blank goes above it. tsv prints that final form in
**one** pass (the run stands off the template by the section blank and off the style's own
leading run by another); prettier reaches it only on a **second** pass — its first strands the
comment glued to the last template node.

tsv: `<div>block</div>`, blank, `<!-- comment1 -->`, blank, `<!-- comment2 -->`, `<style>` — one
pass from either authoring
Prettier: same fixed point, but non-idempotent from the after-style authoring — the first pass
prints `<div>block</div>⏎<!-- comment1 -->` with no blank, then adds it on the second

## Files

- `unformatted_ours_after_style.svelte` — `comment1` authored after `</style>`; normalizes to
  `input.svelte` under tsv in one pass. Prettier does **not** (N6): its first pass is the glued
  form.
- `prettier_intermediate_after_style.svelte` — prettier's unstable first-pass output of that
  source; a second prettier pass converges to `input.svelte`.

## Reason

One fixed point per document. The hoisted run's home in the output is above the `<style>`, and
what stands above a section is that section's leading run — so it takes the section's blank
in the first pass rather than gaining it in the second. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
