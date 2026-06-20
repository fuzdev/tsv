# Line comments in casts — robustness cases

Regression coverage for line-comment handling in angle-bracket type assertions
beyond the four core positions
([`../type_assertion_line_comment_svelte_divergence`](../type_assertion_line_comment_svelte_prettier_divergence/)).

- **`a` — generic cast type with a nested `>`** (`<Map<string, number> // c⏎>x`):
  the cast's closing `>` is found by scanning from the type's *end*, so the type's
  own `>` is already behind the search start and never a candidate
  (`find_assertion_close_angle`'s comment/string skipping is only needed for a `>`
  inside a comment, like `<T /* > */>`). The trailing-type comment is thus placed
  correctly. Both formatters break the cast and trail the comment on the type
  line — **tsv matches prettier here** (this case is in the fixture only because
  it shares the parser divergence below).
- **`b` — object operand after `>`** (`<T> // c⏎{x: 1}`): the after-`>` line
  comment routes the operand onto a continuation line (the same path as a plain
  identifier), so the object does **not** hug the cast. tsv keeps the comment
  after `>`; prettier relocates it into the cast trailing the type. **Divergence.**
- **`c` — block + line comment trailing `<`** (`< /* b */ // c⏎string`): both
  comments share the `<` line and tsv keeps them there (the open-delimiter
  family); prettier moves them to their own line. **Divergence.**

## Parser divergence (`_svelte`)

Each comment sits in the region acorn-typescript re-parses, so it is duplicated in
the root `comments` array; our parser keeps a single entry (`expected_ours.json`
vs `expected_svelte.json`). See
[conformance_svelte.md](../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.

## Formatter divergence (`_prettier`)

`b` and `c` are recorded in `output_prettier.svelte`; `a` is byte-identical there.
See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation (Angle-bracket type assertion).
