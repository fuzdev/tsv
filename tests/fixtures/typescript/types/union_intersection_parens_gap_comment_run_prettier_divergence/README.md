# union_intersection_parens_gap_comment_run_prettier_divergence

A line comment in the member gap of an intersection whose first member is a
stripped redundant paren shell carrying its own deferred trailing comment
(`(⏎ a // line comment⏎) & // inj⏎ b` — the gap comment written on the `&`'s
line, or on the `)`'s line before it). Two deferred comments reach one line
end: the shell's `// line comment` flushes at the member break, and the gap's
`// inj` must not land welded behind it.

**tsv**: both comments keep their own line, in the authored order, the gap
comment at the member's indent:

```
type B = a & // line comment
	// inj
	b;
```

**prettier**: the same separation and order, and it settles on the identical
form — but from the on-the-`&`-line authoring it takes **two passes** (pass 1
leaves the gap comment one level out, at the statement's indent;
`prettier_intermediate_after_amp.svelte`), where tsv normalizes in one.
`input.svelte` itself is a fixed point for **both** formatters, and the
`unformatted_ours_*` variants state the ours-only one-pass normalization.

An inline emission **welds** the pair (`a & // inj // line comment` — reordered,
and reparsing as one comment whose text contains the second); the gap comment
defers through the same `line_suffix` run, where the flush's own separator
breaks between the two (`doc/arena_render_suffix.rs`).

Reason: print-once over the weld, authored order preserved. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
