# nonblock_consequent_trailing_comment_run_prettier_divergence

A brace-less consequent with a line comment before its `;` (`expr // inj⏎;`)
and a second line comment between the `;` and `else` (`; // c1`). tsv collapses
the consequent onto the `if` head (the `else_line_comment_nonblock` family's
divergent-variant landing), relocating the pre-`;` comment past the `;` — so
two comments reach the consequent line's end.

**tsv**: both keep their own line, in the authored order:

```
if (a) expr; // inj
// c1
else expr;
```

**prettier**: keeps the consequent expanded (`if (a)⏎ expr; // inj`), with
`// c1` on its own line — the same separation and order from a different
layout. Neither landing is the other's fixed point: tsv re-collapses prettier's
(the divergent-variant rule this family already pins), and prettier re-expands
tsv's, so `output_prettier.svelte` records prettier's answer on `input.svelte`.

tsv previously **welded** the pair with the order reversed
(`if (a) expr; // c1 // inj` — `// inj` swallowed into `// c1`'s text); the
`;`→`else` gap's trailing comment now defers through the same `line_suffix`
run, where the flush's own separator breaks between the two
(`doc/arena_render_suffix.rs`). `unformatted_ours_expanded.svelte` states the
ours-only normalization from the expanded authoring.

Reason: print-once over the weld, authored order preserved; the collapse itself
is the `else_line_comment_nonblock_prettier_divergence` family's sanction. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
