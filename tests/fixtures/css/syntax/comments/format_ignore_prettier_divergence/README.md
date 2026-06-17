# format_ignore_prettier_divergence

`/* format-ignore */` suppresses formatting of the next rule — tsv's
tool-neutral spelling of the directive, honored alongside `prettier-ignore`.
This is the **standalone** CSS path (`.css`, parsed by Svelte's `parseCss` and
formatted by `tsv_css` directly), the companion to the Svelte-embedded coverage
in
[svelte/syntax/format_ignore/css_nested](../../../../svelte/syntax/format_ignore/css_nested_prettier_divergence/).

This fixture pairs a `format-ignore` case with a `prettier-ignore` control to
show the divergence precisely. tsv honors **both** spellings, so `input.css`
keeps both rules verbatim (idempotent). Prettier honors only its own
`prettier-ignore`, so in `output_prettier.css` the `prettier-ignore`d rule is
preserved unchanged while the `format-ignore`d one is expanded and normalized —
that one rule is the entire divergence:

```css
/* format-ignore */
.a   {   color:   red;   }   /* tsv keeps; prettier expands + normalizes */

/* prettier-ignore */
.b   {   color:   blue;   }  /* tsv keeps; prettier keeps (recognized) */
```

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
