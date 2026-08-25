# inline_adjacent_sibling_newline_flow_prettier_divergence

The sibling-newline flow rule at the **adjacent** separator. Every case in
[inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/) puts prose on
both sides of the flowing sibling, so the separator that flows always belongs to a text node. Here
the separator sits *between two non-text siblings* and carries no prose of its own — the run's
whitespace-only node.

Svelte 5 collapses inter-sibling whitespace to one space, so this separator is the same document
whichever way it is spelled, and it flows exactly like the text-bounded ones beside it. Without
that, one run reaches two answers: the boundaries touching a text node flow while the one between
the two siblings does not, leaving a break in a line that fits —
`text1 text2 <span>inline1</span>⏎<span>inline2</span> text3`, which is neither formatter's form.

- **element pair** — `<span>` beside `<span>`, prose in the run. Prettier agrees on this one.
- **expression-tag pair** — `{expr1}` beside `{expr2}`. Prettier splits the pair here even though
  the line fits, which is the divergence `output_prettier.svelte` records.

Two controls pin the rule's edges:

- **structural cause** — the same run in a container made multiline by a block child rather than by
  its own newlines. It already flowed, so the container's [`MultilineCause`] is the only axis
  between it and the cases above; without it "the run flows" is equally satisfied by a rule that
  never consults the cause at all.
- **prose-free run** — two siblings with no text anywhere in the run. This does **not** flow:
  flowing means reflowing into a text fill, and a run with no content text has no fill to reflow
  into, so its newlines are the author's only structure (a vertical list of siblings).

`prettier_variant_newline.svelte` is the fully newline-authored form — prettier's other stable
form, which tsv normalizes to `input.svelte`. The prose-free control is identical in both files,
which is the point: it is the one run whose authored lines survive.

Every boundary tsv collapses here is inter-node whitespace that renders as one space either way, so
the output renders identically to the input.

## Reason

Design choice: tsv converges the space- and newline-authored spellings of one document onto a
single fixed point, where prettier holds a distinct stable form for each authoring.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
