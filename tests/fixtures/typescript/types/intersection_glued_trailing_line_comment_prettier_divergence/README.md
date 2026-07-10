# intersection_glued_trailing_line_comment_prettier_divergence

A trailing line comment on an intersection member that is object-adjacency-glued
to a **following** member — so the comment can't stay inline where the glued run
continues past it on the same line.

**tsv** keeps the comment on its member's line and breaks that boundary, leaving
the object-adjacent members before it glued:

```
// D: comment on the object, glued to a following non-object member
type D = A &
	B & { c: C } & // c
	D &
	E;

// G: comment on a member, glued to a following object
type G = A &
	B & // c
	{ c: C };
```

**Prettier** `lineSuffix`-relocates the comment past the glued members to the end
of the visual line — across a member boundary (D → trails `D`), and even past the
`;` (G → trails the whole type, which then collapses back inline since nothing
forces the break):

```
type D = A &
	B & { c: C } & D & // c
	E;

type G = A & B & { c: C }; // c
```

(`output_prettier.svelte` pins Prettier's first pass; its `audit_signature.txt`
records the second-pass fixed point where `G` collapses to one line.)

## Reason

Per Comment Position Philosophy, tsv keeps a comment where the author wrote it.
The comment carries authorship signal for **its** member; relocating it across a
member boundary (or past the `;`) to trail a different member would lose that
association. A line comment can never share a line with the following member, so
tsv breaks that boundary to preserve the position — while still applying the same
object-adjacency gluing as Prettier everywhere the comment doesn't force a break.

Only a **trailing** line comment on a member glued to a *following* member
diverges. When the comment instead lands at a neither-object boundary (the last
member before a break), tsv matches Prettier — see the regular fixture
[comments/intersection_object_adjacent_line](../comments/intersection_object_adjacent_line/).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation.
