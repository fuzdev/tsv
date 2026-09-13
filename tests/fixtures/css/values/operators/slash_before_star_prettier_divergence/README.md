# slash_before_star_prettier_divergence

A `/` glued to the `*` after it by a separator rule rather than by its author — at the
value's head, at a group's head, in a custom property, and beside a `font` shorthand's
font-size. Two rules can carry that glue. The **head** rule says an operator with nothing on
its left binds to the member after it, and tsv glues it there exactly where prettier does
(`/2.5`, `/a`, `/(2.5)`, pinned by [operator_head_glue](../operator_head_glue/)); the
**`font`** carve-out keeps a `/` glued to the font-size before it and carries that glue onto
the member after it (`font: 12px/1.5`, a custom property riding the same scope). This is the
one member neither glue can reach.

`/` + `*` is not two tokens glued together. css-syntax-3 §4.3.1 consumes comments (§4.3.2)
ahead of every token, so `/ *2.5` becomes `/* 2.5` — the opener of a comment that never
closes, which swallows the rest of the stylesheet. Neither parser reads the result back:
Svelte's `parseCss` raises `Expected token */` and tsv's own parser `Unterminated
comment`. Prettier emits it anyway (its arm reasons about node adjacency and never asks
what the glued text lexes as), so `output_prettier.svelte` holds output that prettier's
own next pass then rejects.

tsv keeps the authored gap, at both arms — the refusal is read once, ahead of every arm
that can glue (`value_gap_is_glued`'s `introduces_merge`), rather than restated at each: the
`font` arm reached `font: 12px/ * 2` → `12px/* 2` through its own door while the head arm's
own refusal stood. It is scoped to the `*` alone — a `/` is a `<delim-token>` beside every
other member, a second `/` included (`/ /2.5` → `//2.5` is two delimiters and a number, the
author's own token stream, and both formatters emit it; only tsv's output is stable there,
since prettier's *next* pass reads a top-level `//` as a comment and mangles the declaration
— `//2.50` → `;//2.50` — a separate bug this fixture does not catalog) — and to glue the
rule would INTRODUCE: an authored `12px/*` is not a value either parser reads, so there is
nothing to spell back.

## Reason

◆prettier_bug. Output that does not re-parse is never the defensible side — the same line
[operator_head_weld](../operator_head_weld_prettier_divergence/) draws one operator over,
where the glue merges two tokens into one instead of opening a comment.

See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "`/` before a `*`".
