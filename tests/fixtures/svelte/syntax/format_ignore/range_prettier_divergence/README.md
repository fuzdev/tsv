# range_format_ignore_prettier_divergence

`<!-- format-ignore-start -->` … `<!-- format-ignore-end -->` suppress formatting
of every template node between them — tsv's tool-neutral spelling of the range
markers, honored alongside `prettier-ignore-start` / `prettier-ignore-end`.
Prettier doesn't recognize the `format-ignore` family, so it reformats the range
anyway.

tsv preserves the marked range verbatim:

```svelte
<!-- format-ignore-start -->

<div      > text1 text2 </div>

<!-- format-ignore-end -->
```

Prettier (see `output_prettier.svelte`) collapses the spacing, as if the markers
weren't there.

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
