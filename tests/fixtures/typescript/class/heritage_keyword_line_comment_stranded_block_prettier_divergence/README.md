# heritage_keyword_line_comment_stranded_block_prettier_divergence

A heritage clause whose keyword→first-item gap holds a **line comment** (or a
multi-line block) — so the items hang below the keyword — *and* whose list carries a
block **stranded** after a comma (`A, /* c */⏎B`). Class `implements` and interface
`extends` alike, the list fitting on one line and wrapping.

- **tsv**: keeps the comment after the keyword with the items hanging on the next line
  ([extends_keyword_line_comment](../extends_keyword_line_comment_prettier_divergence/)),
  and the stranded block on the comma's side
  ([heritage_item_after_comma_block_stranded](../../declarations/class/heritage/heritage_item_after_comma_block_stranded_prettier_divergence/)):
  a list that fits collapses to one line, where the block hugs the next item
  (`A, /* c */ B`); a list that wraps keeps the block trailing its comma line.
- **prettier**: relocates the keyword comment up before the keyword (`class Class // c`)
  and the stranded block before the comma (`A /* c */,`).

The bug this pins: under the keyword hang, the items were joined with a plain `", "`
while the stranded block's comma had already been baked into its item, so the comma
printed twice (`A, /* c */, B, C {}` — dead output). The hang now joins through the
gap-aware separators, in a group so the list still breaks by width.

`unformatted_ours_stranded` is the stranded authoring of the fitting lists; tsv
collapses it to input, prettier to `variant_stranded` (its relocated form, which tsv
keeps stable).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
