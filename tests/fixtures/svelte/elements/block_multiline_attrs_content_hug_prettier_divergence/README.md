# block_multiline_attrs_content_hug_prettier_divergence

When a whitespace-sensitive element (`<pre>`, `<textarea>`) has multiline attributes and hugged content exceeding printWidth, Prettier keeps `>{content}</tag>` on the attribute line. tsv breaks `>` to its own line.

tsv: `\n>{content}</tag>` (respects printWidth)
Prettier: `">{content}</tag>` on attribute line (exceeds printWidth)

| Line width | tsv                    | Prettier             |
| ---------- | ---------------------- | -------------------- |
| 100 chars  | inline                 | inline               |
| 101+ chars | breaks `>` to new line | inline (exceeds)     |

The `>` immediately precedes `{expr}` with no whitespace, so no whitespace text node is introduced inside the `<pre>` — whitespace semantics are preserved.

## Reason

Print width. tsv respects printWidth while maintaining whitespace semantics. See `block_multiline_attrs_content_hug/` for the matching 100-char case, and
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
