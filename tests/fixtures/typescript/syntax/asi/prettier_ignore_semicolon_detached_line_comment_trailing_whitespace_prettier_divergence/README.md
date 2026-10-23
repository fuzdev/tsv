# prettier_ignore_semicolon_detached_line_comment_trailing_whitespace_prettier_divergence

The `// c4` case of
[prettier_ignore_semicolon_detached_comment](../prettier_ignore_semicolon_detached_comment_prettier_divergence/),
with whitespace left after the comment's text. A line comment's span runs to the end of its line,
trailing whitespace included, so the frozen slice ends after that whitespace and the comment is
still the slice's last content: the terminator the author wrote below it stays below it.

```
tsv       let a = b // c1⎵⏎;
prettier  let a = b; // c1
```

The slice is byte-verbatim, so the whitespace after the comment's text is kept with it.

Ending the slice before that whitespace instead would read the slice as ending in code, re-emit
the `;` glued to it — inside the comment — and turn the next statement's `[a]` into a member
access on the frozen value (`let a = b // c1;⏎[a].map(fn);` reads as `let a = b[a].map(fn)`).
Each cell is followed by a statement that begins with `[`, so a swallowed terminator changes
what the program means.

## Reason

◆comment_preservation, as in the fixture this one extends. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
