# init_left_side_shell_comment_prettier_divergence

An own-line comment inside the grouping parens of a declarator initializer's **leftmost**
node — a conditional's test, a member's object, a callee (`const a = (⏎// c⏎x as any) ? y : z`).
The initializer's printer strips that shell and hoists the run ahead of its whole doc, so
the doc opens with the comment and a hardline; the declarator hangs the value under `=`,
which puts the run and the value at the value's indent. That is the shape a **binary**
initializer already takes in both formatters (`const d`), applied to every left-side kind.

tsv, one pass (`input.svelte`):

```ts
const a =
	// c
	(x as any) ? y : z;
```

Prettier takes two passes and lands elsewhere: its first pass attaches the comment to the
leftmost leaf and prints it flush on the `=` line (`const a = // c⏎(x as any)⏎\t? b⏎\t: c`,
pinned as `prettier_intermediate_to_variant_inner_shell.svelte`); its second reads that comment as the
binding's trailing comment and indents the continuation:

```ts
const a = // c
	(x as any) ? y : z;
```

Both forms are fixed points for both formatters — `variant_operator_line.svelte` pins the
second (for a **call** value prettier's landing is the own-line form, the same as tsv's, so
`const c` reads identically in both files) — so the difference is which one the authored
form converges to. tsv keeps the
comment on the line the author gave it (an own-line comment ahead of the value), which is
also the form its binary sibling reaches in one pass; prettier's landing is an artifact of
its first-pass attachment. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
