# type_param_keyword_line_comment_prettier_divergence

A line comment **already on the keyword line** between a type parameter's `=`
default keyword and its value (`R extends A | B | void = // c\n\t…`), where the
trailing comment is long enough to push the line past the print width.

**tsv**: emits the trailing `=` comment via `line_suffix` (zero width), so it
never forces the preceding `extends` constraint to break — the constraint union
stays inline:

```
R extends A | B | void = // inline trailing comment that is intentionally long…
	// own-line comment about the default
	C
```

**Prettier 3.9**: counts the trailing comment toward the line width and breaks
after `extends`, dropping the constraint union to the next line:

```
R extends
	A | B | void = // inline trailing comment that is intentionally long…
	// own-line comment about the default
	C
```

Both forms are stable under their respective formatters.

## Reason

tsv treats a same-line trailing line comment as zero-width so a long comment
never reflows the code it trails — emitting it as plain text would instead
force-break the constraint union (content added) and risk merging two line
comments onto one line (boundary loss). Prettier 3.9 changed to break the
`extends` constraint under the long trailing comment. The own-line comment cases
in this fixture (`U = // c\n // c\n V`, `T extends // c\n // c\n A`) keep each
comment distinct in both formatters and are not the divergence.

The own-line first-comment counterpart is a separate divergence
([type_param_keyword_own_line_comment](../type_param_keyword_own_line_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
