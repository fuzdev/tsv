# ignore_directive_section_scope_svelte_divergence

A `<!-- prettier-ignore -->` directly above a `<script module>`, with the instance `<script>`
right after it. The directive freezes the section it leads and nothing else: the module script
prints verbatim, and the instance script is formatted (`unformatted_spaces.svelte`, whose
instance body is spaced out, normalizes to `input.svelte` in both formatters). A directive is one
of the section's own travelling comments — the run stops at the nearest section — so it cannot
reach past the module script to the section after it.

The parse side is the known leading-comment duplication: canonical attaches the comment to the
module Program *and* to the instance Program, tsv to the module Program alone
(`expected_ours.json` vs `expected_svelte.json`). The formatter locates the comment by position
and emits it once.

See [conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).
