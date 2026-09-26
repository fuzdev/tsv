# lifted_run_edge_tail/script_prettier_divergence

A `<script module>` written between `x` and `y`, and an instance `<script>` written after `y` and
a non-breaking space, at the end of the document (`unformatted_ours_lifted_nbsp`). The module
script's neighbours join (`xy`), and the reorder lifts both scripts above the template, so the
text — and its non-breaking space, which rendered where the script stood after it — now ends the
document. Svelte's parse would trim that last character there, so tsv spells it `&#xA0;`, as for
any template text the reorder leaves at the end of the document.

Prettier's first pass keeps the raw character at the end (`audit_signature_lifted_nbsp.txt`, pass
1), and its own second pass loses it with the parse's trim — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
