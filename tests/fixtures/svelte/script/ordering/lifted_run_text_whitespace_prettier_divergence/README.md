# lifted_run_text_whitespace_prettier_divergence

A `<script>` written between two text nodes, with whitespace on one side or both. The script
goes to the top and `x` and `y` join as if it had never been there: any whitespace around it
separates them, and a separator between two prose words is one space — `x y`, from a space on
each side, a newline on each side (the lines the script occupied are not a blank line), a blank
line on either side, or two runs in a row with whitespace between and around them. The
`unformatted_lifted_*` variants hold the authorings prettier also normalizes to `input.svelte`.

Prettier deletes the whitespace when it is written only in **front** of the script, the script
glued to what follows: `x <script>…</script>y`, `x⏎<script>…</script>y` and
`x⏎⏎<script>…</script>y` (the `unformatted_ours_lifted_*` variants) all come out `xy`
(`variant_space_dropped.svelte`) — the rendered space is gone, and prettier's next pass keeps it
gone.

## Reason

◆prettier_bug. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
