# escaped_whitespace_long_prettier_divergence

A **long** CSS value containing an escaped whitespace (`xxxxx\ yyyyy`), at the
100/101-column print-width boundary.

`\ ` is a valid escape whose escaped code point is that space (CSS Syntax 3
§4.3.4/§4.3.7), so `xxxxx\ yyyyy` is a **single ident** whose value is
`xxxxx yyyyy`. The space is *inside* a token — it is not a separator between two
values, and therefore not a place a formatter may wrap.

Both cases are pinned:

- the **100-char** line fits exactly and stays inline;
- the **101-char** line must break — and breaks at a real separator (before
  `xxxxx\ yyyyy`), never at the escaped space inside it.

Breaking there would put a `\` at end-of-line, and `\` before a newline is **not**
a valid escape (§4.3.4) — the output would not re-parse at all.

**Prettier silently drops the escape's payload**: `xxxxx\ yyyyy` → `xxxxx\yyyyy`.
That re-parses, but as the ident `xxxxxyyyyy` — a different value. It also leaves
the 101-char line over the print width (its usual print-width behavior; see
[conformance_prettier.md §CSS: Layout](../../../../../docs/conformance_prettier.md#css-layout)),
so the two divergences stack in `output_prettier.svelte`.

tsv preserves the escape and treats the print width as a hard limit.

See [conformance_prettier.md §CSS: Values](../../../../../docs/conformance_prettier.md#css-values).
