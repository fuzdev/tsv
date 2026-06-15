# media_long_prettier_divergence

Long `@media` query with an inline comment. Same wrapping divergence as `media_long` — tsv wraps at 101+ chars, Prettier never wraps.

tsv: wraps query while preserving comment inline
Prettier: keeps everything on one line, preserves comment

Both formatters preserve comments in their original positions. The difference is only in line-width wrapping.

## Reason

tsv enforces printWidth consistently. See [media_long](../../../at_rules/media_long_prettier_divergence/) for the base case.
