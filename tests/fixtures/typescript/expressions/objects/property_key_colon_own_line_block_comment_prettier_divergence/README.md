# Divergence: object property key→`:` own-line block comment collapses to trail the key

An own-line **block** comment in an object property's key→`:` gap
(`{ a⏎/* c */⏎: 1 }`). A single-line block forces nothing, and a comment in this
gap trails the key (a trailing position), so tsv collapses the authored breaks
and keeps the comment inline in its authored syntactic slot (`a /* c */: 1` —
the form both formatters hold stable when authored inline). Prettier instead
**relocates** the comment across the `:` and hangs it leading the value:

```ts
// tsv (collapse in place)   // prettier (relocate past `:`)
const o = {                  const o = {
	a /* c */: 1                 a:
};                               /* c */
                                 1
                             };
```

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier takes it to the relocated hang instead.
- `variant_own_line.svelte` — prettier's landing form, dual-stable: there the
  comment sits *after* the `:`, leading the value — a different syntactic
  position, which both formatters preserve (the value-gap own-line rule). An
  author who wants the comment leading the value writes it there; tsv honors
  both positions and collapses only the line structure of the trailing one.

A run of blocks collapses in order, each comment kept distinct — lossless. The
same-gap **line** comment (which forces the break) is the sibling
[property_key_colon_line_comment](../property_key_colon_line_comment_prettier_divergence/);
the inline-authored block is a plain match in both formatters.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
