# header_brace_line_comment_prettier_divergence

A line comment between an `enum` header and its body `{` keeps its own line, and
the `{` drops below it. Prettier relocates the comment past the `{` — into the
body, or trailing the collapsed `{}`.

tsv: `enum E // c1\n{` — the comment stays in the gap the author wrote it in
Prettier: `enum E { // c1` (then, on its second pass, `enum E {\n\t// c1`)

```
// tsv                          // prettier
enum E // c1                    enum E {
{                                      // c1
	A                                  A
}                                }
```

## Reason

Emitting this gap inline and then appending a bare `" {"` let the comment
**swallow the opening brace** (`enum E // c1 {`), output that does not reparse —
content corruption, not a position change. So the gap must break after a `//`
either way; the only question left is where the comment goes, and tsv keeps it
where the author wrote it (see Comment Position Philosophy).

Prettier's relocation is also information-losing here. From a run it emits
`enum G // c4 // c3`: the two comments are **reordered** and **merged** onto one
line, where `// c3` stops being a comment and becomes text inside `// c4`. tsv
keeps each on its own line, in order, so a reparse still finds two comments. A
line comment followed by a **block** is reordered the same way
(`enum H /* c6 */ // c5`).

A single-line **block** comment in the gap carries no such hazard, so it
collapses onto the header line with `{` hugging it, matching prettier
(`enum L /* c8 */ {`).

This is the `enum` face of the declaration header→`{` gap, whose model is the
class/interface header ([heritage_last_item_line_comment](../../class/heritage/heritage_last_item_line_comment_prettier_divergence/));
all four share `Printer::build_header_pre_body_doc`. The comment on the *other*
side of the same brace is [open_brace_comment](../open_brace_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
