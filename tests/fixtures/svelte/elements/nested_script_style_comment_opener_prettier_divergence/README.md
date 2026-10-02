# nested_script_style_comment_opener_prettier_divergence

A nested `<script>` / `<style>` whose body opens with a `<!--` that never closes. The body of
a nested raw-text element is raw text — Svelte reads it up to the first literal `</script>` /
`</style>` — so the `<!--` is part of the body, not the start of a comment, and the document
parses.

Neither body is something tsv formats (`<!--a` is not JavaScript, `<!-- b` is not a style
sheet), so each is kept exactly as written and the element stays on one line.

Prettier throws on each body — `Unexpected token` from its JavaScript parser on the first, a
`CssSyntaxError` from postcss on the second — so no format oracle exists.
`prettier_rejects.txt` pins the first, which is the one prettier reaches.

## Reason

Design choice. The body is the raw fallback cataloged in
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies)
(kept as written rather than given a freeze shape).

## Related

- [nested_script_style_glued_raw_fallback](../nested_script_style_glued_raw_fallback_prettier_divergence/)
  — raw-fallback bodies prettier does format
- [textarea_comment_opener](../textarea_comment_opener/) — the same `<!--` in RCDATA content,
  where both formatters agree
