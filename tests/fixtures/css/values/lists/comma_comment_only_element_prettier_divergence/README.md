# comma_comment_only_element_prettier_divergence

A comma list whose **first element is nothing but comments** — `font-family: /* c */, b`.

A comment run that opens a declaration's value is between-material, not value content
(postcss keeps it in `raws.between`), so when the list breaks, the run stays on the
colon's line and the elements break beneath it. That is what tsv does, and it matches
prettier — see [comma_comment_interior](../comma_comment_interior/).

This fixture is the one shape where the hoist has nothing left to hoist *from*: with no
content after the run inside element 0, cutting the run out leaves that element empty.
Prettier hoists anyway and strands a bare `,` on its own line:

```
font-family: /* c */
	,
	b;
```

tsv declines the hoist and leaves the run with its element:

```
font-family: /* c */, b;
```

## Reason

Stable quirk. Both outputs re-parse to the same value, so nothing is at stake but
legibility, and a line holding only a comma communicates nothing about the value it
punctuates. tsv's form is also what its own inline path already emits for the shape, so
declining costs no extra rule — the hoist simply requires a member boundary with content
on the far side of it, and here there is none.

See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [comma_comment_interior](../comma_comment_interior/) — the shapes where tsv and prettier
  agree: the run hoists onto the colon's line, and a comment counts as a node for the
  break decision
- [multi_comment_before_colon](../../../tokens/comments/multi_comment_before_colon_prettier_divergence/)
  — the same run, on the property side of the colon
