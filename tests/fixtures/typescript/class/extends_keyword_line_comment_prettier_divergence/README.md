# extends_keyword_line_comment_prettier_divergence

A line comment after a heritage keyword (`extends`/`implements`), before the
first heritage type (`class A extends // c\n\tB`). Covers class `extends`, class
`implements`, and interface `extends` — the shared heritage-keyword gap.

**tsv**: keeps the comment after the keyword, with the heritage type on the next
line:

```
class A extends // c
	B {}
```

**Prettier**: relocates the comment up onto the previous line, before the
heritage keyword, and breaks the keyword + type onto the next line:

```
class A // c
	extends B {}
```

## Reason

Per Comment Position Philosophy: the user wrote the comment after the heritage
keyword, so tsv keeps it associated with the keyword rather than floating it
back before the keyword. Both forms are idempotent in their respective
formatters. A same-line block comment (`class A extends /* c */ B`) stays inline
in both formatters and is not a divergence (see the regular
[extends_keyword_comment](../extends_keyword_comment/) fixture); only a line
comment after the keyword diverges.

Previously tsv emitted the comment inline and **swallowed the heritage type**
(`class A extends // c B {}` — `B {}` absorbed into the comment, a
non-idempotent content loss); keeping it on the keyword line via `line_suffix`
with the type on the next line fixes the loss and preserves the user's
placement.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
