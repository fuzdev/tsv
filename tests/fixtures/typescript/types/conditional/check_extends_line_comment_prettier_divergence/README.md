# check_extends_line_comment_prettier_divergence

A line comment after a conditional type's `extends` keyword, before the check's
extends-type (`type C = T extends // c\n\tU ? X : Y`).

**tsv**: keeps the comment after `extends`, with the extends-type on the next
line (the conditional breaks because the comment forces it multiline):

```
type C = T extends // c
	U
	? X
	: Y;
```

**Prettier**: relocates the comment to trail the extends-type `U`, then breaks
the conditional:

```
type C = T extends U // c
	? X
	: Y;
```

## Reason

Per Comment Position Philosophy: the user wrote the comment after `extends`, so
tsv keeps it there rather than floating it past the extends-type. Both forms are
idempotent in their respective formatters. A same-line block comment
(`T extends /* c */ U ? X : Y`) stays inline in both formatters and is not a
divergence; only a line comment after `extends` diverges.

Previously tsv emitted the comment inline and **swallowed the rest of the
conditional** (`type C = T extends // c U ? X : Y;` — `U ? X : Y` absorbed into
the comment, a non-idempotent content loss); keeping it on the `extends` line
via `line_suffix` with the extends-type on the next line fixes the loss and
preserves the user's placement.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
