# prettier_ignore_after_brace_inert_prettier_divergence

A directive **trailing an opening brace** is **inert** across member and
statement lists — class bodies, enum bodies, and block statement lists: tsv
honors a directive only when it is alone on its line, so the comment stays on
the brace line as an ordinary comment and the first member or statement formats
normally.

Prettier honors the placement — it relocates the brace-trailing directive to
its own line and freezes the first member/statement (see
`output_prettier.svelte`). `unformatted_ours_perturbed.svelte` carries
first-member perturbations that only tsv normalizes; prettier keeps them
frozen.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
