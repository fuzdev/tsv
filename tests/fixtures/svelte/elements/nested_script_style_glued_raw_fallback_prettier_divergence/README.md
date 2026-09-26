# nested_script_style_glued_raw_fallback_prettier_divergence

A nested `<script>` / `<style>` whose body tsv keeps exactly as written — a `<style>` body that
fails its CSS parse, and a `<script>` body that formats to nothing — glued to content on both
sides. The element renders no box, so the text on either side meets directly
(`text1<style>…</style>text2` renders `text1text2`) and neither glued boundary may take a break.
With the body kept on one line there is nothing left to break, so the element stays inline and
the paragraph fits on one line.

Prettier breaks the body onto its own lines: the unparseable style body is indented between the
two tags, and the empty script body becomes a blank line on the first pass and a single line break
on the second (`output_prettier.svelte`, with the rest of the chain in `audit_signature.txt`).

- `unformatted_ours_spaces.svelte` — extra spaces at the paragraphs' content boundaries; tsv
  normalizes it to `input.svelte`.

All of them render identically.

## Reason

Design choice. The body is the raw fallback cataloged in
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies)
(kept as written rather than given a freeze shape); the glued boundaries around it are a render
fact. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [nested_script_style_glued](../nested_script_style_glued/) — bodies that break, where both
  formatters agree
