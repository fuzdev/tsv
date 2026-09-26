# ignore_directive_template_end_prettier_divergence

A `<style>` written between two paragraphs, and a `<!-- prettier-ignore -->` ending the template
(`unformatted_ours_lifted_style`). The directive stands above nothing in the source, so it
freezes nothing: the style is formatted. The reorder prints the `<style>` last, directly below
the directive — and there, as in `input.svelte`, the directive freezes the style as it now stands,
already formatted, so the document is its own fixed point.

Prettier formats the style the same way, but its first pass leaves the directive glued to the
paragraph above it (`prettier_intermediate_lifted_style.svelte`); its second pass gives it the
section blank above the `<style>`, the form tsv prints in one pass.

## Reason

◆prettier_bug (non-idempotent). See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
