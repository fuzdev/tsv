# format_ignore_prettier_divergence

`// format-ignore` / `/* format-ignore */` suppress formatting of the next
statement — tsv's tool-neutral spelling of the directive, honored alongside
`prettier-ignore`. This is the **standalone** TypeScript path (`.ts`, parsed by
acorn-typescript and formatted by `tsv_ts` directly), the companion to the
Svelte-embedded coverage in
[svelte/syntax/format_ignore/js_css](../../../../svelte/syntax/format_ignore/js_css_prettier_divergence/).

It covers both comment delimiters (`// format-ignore` and `/* format-ignore */`)
and pairs them with a `prettier-ignore` control to show the divergence precisely.
tsv honors **both** spellings, so `input.ts` keeps every marked statement verbatim
(idempotent). Prettier honors only its own `prettier-ignore`, so in
`output_prettier.ts` the `prettier-ignore`d statement is preserved unchanged while
both `format-ignore`d ones are reformatted — those are the entire divergence:

```ts
// format-ignore
const a = {b:   1,   c:   2};   // tsv keeps; prettier reformats → { b: 1, c: 2 }

/* format-ignore */
const d = [1,    2,    3];      // tsv keeps; prettier reformats → [1, 2, 3]

// prettier-ignore
const e = {f:   3,   g:   4};   // tsv keeps; prettier keeps (recognized)
```

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
