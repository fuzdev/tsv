# prettier_ignore_semicolon_detached_comment_prettier_divergence

A frozen statement's terminator is the printer's, so the slice ends at the statement's
content and the `;` is re-emitted (the plain case matches prettier —
[prettier_ignore_semicolon_detached](../prettier_ignore_semicolon_detached/)). The two tools
disagree on where that content ends when a **comment** sits between it and the `;`.

Prettier reaches its content end through a comment-stripped copy of the text, so the comment
falls outside the frozen slice and is re-printed past the terminator:

```
tsv       fn(  a  ) /* c1 */;
prettier  fn(  a  ); /* c1 */
```

Same for the `VariableDeclaration` and keyword spellings (`const b  =  x /* c2 */;`,
`debugger /* c3 */;`), where prettier's `locEnd` is positional (the last declarator, the
keyword) and ejects the comment without needing any whitespace to trim.

A `//` is the fourth case and a stronger one: it owns the rest of its line, so a terminator
written below one stays below it and the whole node freezes as authored (`fn(  c  ) // c4⏎;`).
Prettier pulls the `;` up and lands the comment after it. Pulling it up is not an option here —
appending a `;` to a line comment's line does not move the terminator, it swallows it, and
`const a = x // c⏎;` followed by `(y) => 1;` welds into `const a  =  x // c;`, which no longer
parses at all.

An **author blank inside the slice** is printed once, by the slice itself: the terminator is
re-emitted glued to the content it terminates, but everything above that content end is copied
verbatim, blank lines included. Reading such a blank a second time at the statement seam
fabricates one below the `;` that the author never wrote — and a fabricated blank is its own
fixed point, so nothing but this fixture and a prettier `compare` reaches it. Both spellings
carry it (`// c5`, `/* c6 */`).

`unformatted_ours_detached.svelte` pads each block-comment case's terminator onto its own line;
tsv normalizes them back to input, and the `// c4` / `/* c6 */` cases are already input's own
form.

## Reason

A comment is content, and the directive's promise is about the bytes it covers —
◆comment_preservation, the same reading that makes a `prettier-ignore` range
[byte-verbatim](../../../../svelte/syntax/prettier_ignore/range_glued_prettier_divergence/).
See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
