# lifted_run_space_join_prettier_divergence

A `<script>` written between two inline neighbours with whitespace around it. The script goes
to the top, and the neighbours join as if it had never been there: `<b>x</b> <script>…</script> <b>y</b>`
is `<b>x</b> <b>y</b>` (`unformatted_lifted_inline_spaced`), and `<b>x</b><script>…</script> y`
is `<b>x</b> y` (`unformatted_lifted_inline_text_space_after`) — both formatters agree on these.

With the space written only in **front** of the script (`<b>x</b> <script>…</script>y`,
`unformatted_ours_lifted_inline_text_space_before`), prettier deletes it and prints `<b>x</b>y`
(`variant_space_dropped.svelte`), a render change its next pass keeps. tsv keeps the space.

## Reason

◆prettier_bug. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
