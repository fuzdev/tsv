# type_members_prettier_ignore_after_brace_inert_prettier_divergence

A directive **trailing the opening brace** of a type literal or interface body
is **inert**: tsv honors a directive only when it is alone on its line, so the
comment stays on the brace line as an ordinary comment and the first member
formats normally.

Prettier honors the placement — it relocates the brace-trailing directive to
its own line and freezes the first member (`output_prettier.svelte`).
`unformatted_ours_perturbed.svelte` perturbs the first members; only tsv
normalizes it to input — prettier turns it into `variant_frozen.svelte`, the
own-line frozen form that is stable under **both** tools (own-line is a
placement tsv honors).

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line — an author who wants the
freeze writes the directive on its own line, where both tools agree.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
