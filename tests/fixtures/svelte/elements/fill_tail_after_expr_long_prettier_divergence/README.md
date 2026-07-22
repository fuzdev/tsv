# fill_tail_after_expr_long_prettier_divergence

A fill whose **tail word follows an expression tag** as the element's last child. Text that
follows a tag is built with a leading separator, which shifts the fill's content/separator
parity by one — every `line` occupies a content slot and every word a separator. Such a fill
always has even length, so its final `[line, word]` pair lands in the fill renderer's
two-items-left case.

At 101 chars Prettier keeps the tail word on the line and overflows; tsv breaks at the
collapsible space before it to hold the line ≤ 100.

```
tsv (breaks, line ≤ 100):  `…aaaa ~{xxxxxxxxxx}` / `cc`
Prettier (keeps the tail):  `…aaaa ~{xxxxxxxxxx} cc`   101 chars
```

The first `<li>` is one character shorter — exactly 100 — so the fill stays on one line under
both formatters. That pins the boundary rather than just the overflow.

Prettier is *stable on both authorings*: it preserves the broken form and preserves the joined
form. So the divergence is one of **normalization**, not output — `prettier_variant_joined`
holds the joined 101-char authoring that Prettier keeps stable and tsv rewrites to `input`.

## Reason

Print width is a hard limit in tsv. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy),
which lists "fill algorithm edge cases (101-char boundaries)" among the constructs this
governs, alongside the sibling fill-boundary entries in
[§Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
