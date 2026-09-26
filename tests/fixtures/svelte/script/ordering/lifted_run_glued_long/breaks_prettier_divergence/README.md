# lifted_run_glued_long/breaks_prettier_divergence

A `<script>` written between two neighbours glued to it — a global `svelte:*` element and text
(`a<svelte:document /><script>…</script>bbbb`), an expression tag and text
(`a{c}<script>…</script>bbbb`). The script goes to the top, and the neighbours join as if it had
never been there — glued, one welded unit, laid out exactly as without the script. Each unit
would end its line at 101 after the preceding word, so the whole unit travels to a fresh line.
The glued seam the script left takes no break: `a<svelte:document />⏎bbbb` and `a{c}⏎bbbb` would
render `a bbbb` where the source renders `abbbb`. The 100-wide twins, which stay on the line
under both formatters, are [fits](../fits/).

`output_prettier.svelte` is prettier's output from `input.svelte` — the layout it prints for the
same units with no script in them, and where both `unformatted_ours_lifted_*` variants land: it
keeps each unit on the word's line, dangling `<svelte:window⏎/>` and breaking before the tail
text instead. The divergence is the welded-unit travel, not the join.

## Reason

◆design_choice. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and [§Moving a run](../../../../../../../docs/conformance_prettier_svelte.md#moving-a-run-break-before-travel-and-welded-units).
