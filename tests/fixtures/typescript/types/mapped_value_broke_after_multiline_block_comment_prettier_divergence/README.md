# mapped_value_broke_after_multiline_block_comment_prettier_divergence

A multi-line block comment the author broke after, ahead of a mapped type's plain
(non-union) value (`{ [K in T]: /* c⏎d */⏎V }`).

**tsv** keeps the block after the `:` and hangs the value below it, one level under
the member — the answer its union value already gives
(`mapped_value_own_line_block_comment`).
**Prettier** is non-idempotent here: its first pass leaves the block after the `:`
with the value flush at the member's indent, its second breaks the `[K in T]`
brackets open and trails the block after the key type, inside them
(`[⏎K in T /* c⏎d */⏎]: V`), which is where it settles (`audit_signature.txt`).

## Reason

Per Comment Position Philosophy: the author wrote the block after the member `:`,
ahead of the value, so tsv keeps it bound to the value rather than floating it into
the key's brackets. The break after the `*/` is prettier's own `printLeadingComment`
rule — a block with a newline after it takes a soft `line`, which a multi-line
block forces open. A multi-line block glued to the value keeps that line, and a
single-line block the author broke after collapses onto the value, in both
formatters. The break belongs to the RUN rather than to one comment, so a
multi-line block the author glued *ahead* of a single-line one they broke after
(`type M`) hangs exactly as the lone block does. The line-comment spelling is
[mapped_value_line_comment](../mapped_value_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
