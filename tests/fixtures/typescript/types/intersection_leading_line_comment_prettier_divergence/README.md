# intersection_leading_line_comment_prettier_divergence

A leading line comment on the first member of an intersection type, written
trailing the `=` (`type C = // leading\n\ta & b;`).

**tsv**: keeps the comment trailing the `=`, with the intersection on a
continuation line indented one level:

```
type C = // leading
	a & b;
```

**Prettier 3.9**: relocates the comment to its own line after the `=`:

```
type C =
	// leading
	a & b;
```

Both forms are stable under their respective formatters. The same pattern applies
when the inner type is a parenthesized union (`(a | b) & c`).

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(after the `=`) and indents the intersection continuation. Under prettier 3.8
this was a multi-pass normalization quirk — prettier converged to tsv's form in
two passes. Prettier 3.9 changed to drop the comment onto its own line, so the
two formatters now settle on different stable outputs.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation.
