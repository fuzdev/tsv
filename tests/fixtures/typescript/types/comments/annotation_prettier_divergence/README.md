# annotation_prettier_divergence

A line comment between a property signature's `:` and a multi-member **union**
type (`{ b: // c\n\t\tX | Y }`).

**tsv**: keeps the comment trailing the `:`, with the union dropped to a
continuation line indented one level:

```
b: // union
	X | Y;
```

**Prettier**: relocates the comment to its own line after the `:`:

```
b:
	// union
	X | Y;
```

Both forms are stable under their respective formatters.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(after the `:`) and applies the uniform forced-continuation indent to the union.

This was a **match** under prettier 3.8 — both formatters kept the comment
trailing the `:` and indented the union continuation. Prettier 3.9 changed the
union-after-`:` case to drop the comment onto its own line, so it now joins the
simple- and intersection-type cases as a deliberate divergence. The contrast
fixtures pin the boundary: prettier **relocates** the comment for a simple type
([annotation_simple](../annotation_simple_prettier_divergence/)) and keeps an
intersection **flush** ([annotation_continuation_indent](../annotation_continuation_indent_prettier_divergence/)),
while tsv applies one continuation layout everywhere.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
