# condition_absorbed_brace_in_comment_prettier_divergence

A block comment between a switch's `)` and its body `{` may itself contain a
brace (`switch (x) /* { */ {`). The body's real `{` is the one *outside* the
comment — the scan must skip comment contents to find it (a naive `find('{')`
matched the inner brace, mis-anchoring the body brace and **dropping** the
comment).

- `switch (x) /* { */ {` — tsv preserves the comment between `)` and `{`;
  **prettier relocates it into the condition parens** (`switch (x /* { */) {`).
  This is the same relocation as the sibling
  [condition_absorbed_comment](../condition_absorbed_comment_prettier_divergence/);
  the brace inside the comment is the added scan-robustness case.

Per the comment-position philosophy, tsv keeps comments where the author wrote
them. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
