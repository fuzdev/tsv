# nonblock_consequent_own_line_comment_run_prettier_divergence

A brace-less consequent whose pre-`;` gap holds a same-line trailer AND an own-line
line comment, with a third comment in the `;`→`else` gap — three comments reach one
flush at the `else` break. The own-line sibling of
[nonblock_consequent_trailing_comment_run](../nonblock_consequent_trailing_comment_run_prettier_divergence/).

**tsv**: collapses the consequent onto the `if` head (the `else_line_comment_nonblock`
family's divergent-variant landing) and keeps every comment on its own line, in
authored order, at the `if`'s level:

```
if (a) expr; // c1
// c2
// c3
else fn1();
```

**prettier**: keeps the consequent expanded (`if (a)⏎\texpr; // c1`) with the same
separation and order — the same page from a different layout. Neither landing is the
other's fixed point: tsv re-collapses prettier's (the family's standing rule), and
prettier re-expands tsv's, so `output_prettier.svelte` records prettier's answer on
`input.svelte`.

tsv previously printed the own-line pre-`;` comment as real text whose line closed
only at the `else` break, so the `;`→`else` comment's deferred run flushed welded onto
it (`// c2 // c3` — the second comment becoming the first's text). In clause position
the whole terminator-gap run now defers through the same `line_suffix` machinery,
dedented to the construct's level, so the run meets the flush in authored order with
the separator breaking between members (`doc/arena_render_suffix.rs`).
`unformatted_ours_expanded.svelte` states the ours-only normalization from the
expanded authoring.

Reason: print-once over the weld, authored order preserved; the collapse itself is the
`else_line_comment_nonblock_prettier_divergence` family's sanction. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
