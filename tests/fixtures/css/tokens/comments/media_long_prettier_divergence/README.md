# media_long_prettier_divergence

Long `@media` query with an inline comment. Same wrapping divergence as the
comment-free base case ([at_rules/media_long](../../../at_rules/media_long_prettier_divergence/)) —
tsv wraps at 101+ chars, Prettier never wraps.

tsv: wraps query while preserving comment inline
Prettier: keeps everything on one line (101 chars), preserves comment

Both formatters preserve comments in their original positions. The difference is only in line-width wrapping.

## Reason

Print width (not a comment stable-quirk like its siblings in §CSS: Comments —
the comment is incidental, the divergence is the unwrapped over-width line). tsv
enforces printWidth consistently. See
[at_rules/media_long](../../../at_rules/media_long_prettier_divergence/) for the
comment-free base case and
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments)
for the catalog entry.
