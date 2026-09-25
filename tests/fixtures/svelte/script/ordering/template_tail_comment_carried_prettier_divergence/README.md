# template_tail_comment_carried_prettier_divergence

Template text ending in a non-breaking space, followed by a comment glued to the `<script>`
(`unformatted_ours_before_comment.svelte`: `text NBSP <!-- c --><script>…`). The comment is the
script's leading comment, so it travels with the script above the template — and the text, which
the comment stood behind, now ends the document. tsv spells its last character `&#xA0;`, as it
does when the script itself follows the text
([template_tail_nbsp](../template_tail_nbsp_prettier_divergence/)).

Prettier carries the comment the same way but drops the whole text node, landing on
`variant_text_dropped.svelte` in one pass — a render change.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
