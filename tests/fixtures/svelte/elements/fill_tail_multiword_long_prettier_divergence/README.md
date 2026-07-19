# fill_tail_multiword_long_prettier_divergence

The parity-shifted tail case with a **multi-word** tail — the companion to
[fill_tail_after_expr_long](../fill_tail_after_expr_long_prettier_divergence/), which has a
single tail word.

It pins *where* the break lands. The fill's separators are measured against the word each one
precedes, so the break falls at the **first** separator that overflows — here before `cc`,
taking `cc dd` to the continuation line together:

```
tsv       `…aaaa ~{xxxxxxxxxx}` / `cc dd`        all ≤ 100
Prettier  `…aaaa ~{xxxxxxxxxx} cc dd`            101 chars
```

The failure this guards against is a break one separator too late: measuring a `line` in a
content slot *by itself* (it is 1 column flat, so it always "fits") leaves the line at 101 and
drops only `dd`. That is both over-width and non-idempotent, since the next pass measures from
a different column and moves the break again.

Prettier keeps the joined authoring stable, so the divergence is one of normalization —
`prettier_variant_joined` holds the 101-char form tsv rewrites to `input`.

## Reason

Print width is a hard limit in tsv. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
