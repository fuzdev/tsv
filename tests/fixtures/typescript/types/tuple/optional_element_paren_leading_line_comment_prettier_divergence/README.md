# optional_element_paren_leading_line_comment_prettier_divergence

An optional tuple element's operand shell whose leading gap holds a **line** comment, at
the operand kinds that are **not** unions. The element's operand needs its parens, so the
author's shell and the required pair are the same pair — the coincidence
[required_paren_shell_line_comment](../../required_paren_shell_line_comment_prettier_divergence/)
catalogs for the trailing gap, read at the leading one.

**tsv**: renders the run inside that pair, opening it:

```
type A = [
	(
		// c
		B extends C ? D : E
	)?
];
```

**Prettier**: hoists the comment out in front of the pair (`// c⏎(B extends C ? D : E)?`),
re-binding it from the operand to the whole element.

The reason this is a divergence rather than a preference is case **O**: for a **union**
operand prettier renders the identical authoring *inside* its own pair, exactly as tsv
does. Prettier answers one gap two ways, keyed on the operand's kind; tsv answers it once.
Keeping the comment inside is also what keeps the two spellings of this position agreeing —
glued instead (`(// c⏎↹B extends C ? D : E)?`, tsv's own earlier form) the `(` sits on the
comment's line and the `)` on the type's, a third shape neither the union spelling nor
prettier produces. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **A**, **F**, **G** — the divergence at the three non-union operand kinds a required pair
  reaches here: conditional, function type, intersection.
- **J** — a redundant **double** shell, collapsing to the one pair with its comment found
  between the two `(`s (the gaps are the deep ones, not one paren's own window).
- **O** — the **union** operand control: prettier agrees with tsv here, which is what makes
  the parting its own inconsistency rather than a blanket tsv preference. The plain fixture
  [optional_element_paren_comment](../optional_element_paren_comment/) pins that case and
  the block spellings of both gaps.
- `unformatted_ours_shell.svelte` — the authoring as written, which tsv normalizes to
  `input.svelte` and prettier does not.
