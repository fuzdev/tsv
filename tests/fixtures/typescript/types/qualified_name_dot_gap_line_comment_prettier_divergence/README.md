# Divergence: qualified-name dot-gap line comments (preserve)

A *line* comment in either gap around a qualified name's `.` (`ns // c⏎.Type`, `ns. // c⏎Type`).
tsv keeps it where the author wrote it and continues the rest of the name one level down; prettier
**relocates** it past the `;`, where it no longer reads as being about the name at all.

```ts
// tsv (preserve)          // prettier (relocate)
let a: ns // c1            let a: ns.Type; // c1
	.Type;
```

The qualified-name twin of
[meta_property/dot_gap_line_comment](../../expressions/misc/meta_property/dot_gap_line_comment_prettier_divergence/):
same shape (`name` `.` `name`), same two gaps, same rule — and, before the fix, the same bug. Both
route through one printer, so neither can drift.

Block comments in these gaps are **not** a divergence: prettier keeps each on its authored side of
the dot, and so does tsv — pinned by the regular sibling
[qualified_name_dot_gap_comments](../qualified_name_dot_gap_comments/).

The type alias is the one nested shape: a breaking right-hand side goes below the `=` (prettier's
own convention for a multiline RHS — a broken union takes the same form), so the name's `+1`
continuation nests inside that, not against the statement. The annotation and heritage cases show
the rule flat. Prettier isn't an oracle for any of it — it relocates the comment out of the name
first, and on the heritage case leaves `.Base` sitting level with the `extends` it belongs to.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation)
and [§Comments inside a multi-word keyword](../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
(the punctuator-joined member of that class — the reason its usual detector, a `d.text` literal with
an *interior* space, cannot see it: the joining literal is `"."`).
