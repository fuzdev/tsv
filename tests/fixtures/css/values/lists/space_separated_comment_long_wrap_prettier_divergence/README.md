# space_separated_comment_long_wrap_prettier_divergence

The comment-bearing twin of [space_separated_long_wrap](../space_separated_long_wrap_prettier_divergence/):
a space-separated value holding a `/* c */` whose content fills exactly to printWidth, so the
declaration's trailing `;` would be the 101st column.

tsv: wraps the last fill item to a continuation line (every line ≤100)
Prettier: tolerates the **1-char** overage — the comment-bearing value takes the same `fill()` as
the comment-free one, and that fill doesn't count the parent's trailing `;`. The item that wraps
is whichever fill item is last: a word, or a trailing comment. The same boundary holds on the
fresh line a word drops to after a multi-line comment's last line — the tail pairs with it at 100
columns with the `;`, and wraps at 101.

## Reason

Print width. tsv treats printWidth as a hard limit; a comment in the value changes nothing about
that. See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Space-separated value wrap").

## Related

- [space_separated_comment_long](../space_separated_comment_long/) — the agreeing cells (100 fits,
  102 wraps) for every comment position, and the leading run's glue
- [space_separated_long_wrap](../space_separated_long_wrap_prettier_divergence/) — the comment-free
  twin
- [space_separated_multiline_comment_wrap_long](../space_separated_multiline_comment_wrap_long/) — the
  agreeing cells (100 fits, 102 wraps) of that fresh-line fill
