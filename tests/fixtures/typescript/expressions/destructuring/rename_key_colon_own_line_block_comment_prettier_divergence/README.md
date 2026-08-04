# rename_key_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a destructuring rename's key→`:` gap
(`{ a⏎/* c */⏎: b }`, also in a parameter pattern). A single-line block forces
nothing, and a comment in this gap trails the key (a trailing position), so tsv
collapses the authored breaks and keeps the comment inline in its authored
syntactic slot — glued, `{ a /* c */: b }`, the same per-site parity as an
object property (the form both formatters hold stable when authored inline).
Prettier instead **relocates** the comment across the `:`, expanding the
pattern and hanging it leading the local name:

```ts
// tsv (collapse in place)     // prettier (relocate past `:`)
const { a /* c */: b } = o;    const {
                               	a:
                               		/* c */
                               		b
                               } = o;
```

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier takes it to the relocated hang instead (one pass — no
  intermediate).
- `divergent_variant_own_line.svelte` — prettier's landing, which is **not**
  dual-stable: unlike an object literal (whose authored expansion tsv
  preserves, making the object-property hang a `variant_*`), a destructuring
  pattern has no multiline preservation, so tsv re-collapses prettier's form —
  keeping the comment in its relocated after-`:` slot, glued leading the local
  (`{ a: /* c */ b }`). Three stable forms coexist, keyed on which side of the
  `:` the comment was authored; only prettier moves a comment between them.

The object-property sibling is
[property_key_colon_own_line_block_comment](../../objects/property_key_colon_own_line_block_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
