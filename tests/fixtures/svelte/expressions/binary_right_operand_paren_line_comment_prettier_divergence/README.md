# Binary operand paren at the chain's end, line comment, in a template island

The template twin of
[right_operand_paren_line_comment](../../../typescript/expressions/binary/right_operand_paren_line_comment_prettier_divergence/):
a `//` between a binary chain's last operand and the `)` of its required pair, in an
expression tag, an attribute value and a block head.

- **tsv**: keeps the comment inside the pair and opens it, as in a `<script>`.
- **prettier**: relocates the comment past the `)` and defers it to the end of the
  line — which in a template island is past the tag's `}`, so the comment comes out as
  **rendered page text** (`{x - (y - z)} // c1`), and past a block head it moves into the
  block's body on the second pass (pinned in `audit_signature.txt`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
