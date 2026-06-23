# case_colon_in_comment_prettier_divergence

A block comment between a case/`default` label and its `:` may itself contain a
colon (`case 1 /* a:b */:`). The label's real `:` is the one *outside* the
comment — the scan must skip comment contents to find it (a naive `find(':')`
matched the inner colon and **dropped** the comment).

- `case 1 /* a:b */:` — tsv preserves the comment before the colon; **prettier
  also preserves it here** (this position is a plain match once the scan is
  correct).
- `default /* c:d */:` — tsv preserves the comment before the colon; **prettier
  relocates it into the body** (`default: /* c:d */ break;`). This is the same
  relocation as the sibling [case_colon_comment](../case_colon_comment_prettier_divergence/);
  the colon inside the comment is the added scan-robustness case.

Per the comment-position philosophy, tsv keeps comments where the author wrote
them. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
