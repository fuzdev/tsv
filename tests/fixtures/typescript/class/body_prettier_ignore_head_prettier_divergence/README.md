# body_prettier_ignore_head_prettier_divergence

An own-line directive in the head→`{` gap of a class, where the following node is the
class **body**. tsv freezes that body over its own node span — the braces and every
member inside them — while the head (name, `extends` clause) stays parent-owned and
normalizes:

```ts
class Bbb extends Aaa
// prettier-ignore
{
	nnn   =   1;
}
```

Prettier **relocates** the directive inside the body, pulling the `{` up onto the head
line, and freezes only the **first member** there. So the same authoring freezes the
whole body for tsv and one member for prettier, and prettier's form no longer holds
the directive at the position the author wrote it.

A class **expression** shares the head and behaves identically. **Both spellings**
behave alike — placement keys the freeze, not the comment's spelling.

The plain-comment forms of the same head gap are cataloged separately:
[heritage_keyword_own_line_block_comment](../heritage_keyword_own_line_block_comment_prettier_divergence/)
and [extends_keyword_line_comment](../extends_keyword_line_comment_prettier_divergence/)
pin the heritage-keyword gaps.

## Reason

tsv never relocates a directive: the placement the author wrote is the placement that
decides the freeze, so the frozen node is the one that actually follows the directive;
◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
