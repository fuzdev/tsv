# basic_format_ignore_prettier_divergence

`<!-- format-ignore -->` suppresses formatting of the next template node — tsv's
tool-neutral spelling of the directive, honored alongside `prettier-ignore`.
Prettier doesn't recognize `format-ignore`, so it reformats the element anyway.

This fixture pairs a `format-ignore`d element with a `prettier-ignore` control to
show the divergence precisely. tsv honors **both** spellings, so `input.svelte`
keeps both elements verbatim (idempotent). Prettier honors only its own
`prettier-ignore`, so in `output_prettier.svelte` the `prettier-ignore`d element
is preserved unchanged while the `format-ignore`d one is collapsed onto one line —
that one element is the entire divergence:

```svelte
<!-- format-ignore -->
<div    class="a"     data-attr="value"    >  <!-- tsv keeps; prettier collapses -->
  text
</div>

<!-- prettier-ignore -->
<div    class="b"     data-attr="value"    >  <!-- tsv keeps; prettier keeps -->
  text
</div>
```

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
