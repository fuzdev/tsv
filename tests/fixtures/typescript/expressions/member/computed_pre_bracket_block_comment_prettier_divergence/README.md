# computed_pre_bracket_block_comment_prettier_divergence

A block comment in the gap between a computed access's **object and its `[`** — the
block-comment counterpart of
[computed_pre_bracket_line_comment](../computed_pre_bracket_line_comment_prettier_divergence/),
which covers the same gap for `//` comments.

**tsv** keeps each comment where the author wrote it: an **own-line** block keeps its
own line before the `[`, the bracket following on the next line (cases `a`, `b`) — the
same answer the gap's `//` already gets. A comment **glued** on the object's line, a
glued run, and a run continuing a multiline block's closing line all trail the object
before the `[` in **both** formatters (cases `c`, `d` — a match, carried as controls).

**Prettier** hoists an own-line block out of the gap entirely, to lead the whole
assignment RHS — and when the gap also holds a trailing comment it **reorders** the two
(case `b`: `/* c3 */` renders above `/* c2 */`), the same defect as the `//` sibling.
With a **numeric** index its destination flips again, to *inside* the brackets
(`a[/* c */⏎0]`).

```
// tsv                     // prettier
const a =                  const a =
	arr                        /* c1 */
	/* c1 */                   arr[i];
	[i];
```

Both formatters break after the `=` (the RHS carries a broken comment run), so the
divergence is the comment's position alone. The `//` sibling keeps `arr // c1` on the
`=` line instead — a trailing `//` defers via `line_suffix`, which the `=` layout
never sees as a break, while a block run with an own-line member breaks for real.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it rather
than hoisting it out of the member expression; own-line-ness is authoring signal for a
leading position, so the comment holds its own line before the `[` exactly as the same
gap's `//` does. Preserving also keeps two comments in the gap **distinct and in
order** — prettier's hoist reorders them (case `b`) — and sidesteps prettier's
index-kind instability (out to the RHS for `[i]`, into the brackets for `[0]`): one
authored position, one destination.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
