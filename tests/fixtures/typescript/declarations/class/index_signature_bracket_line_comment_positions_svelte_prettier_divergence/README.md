# Class index-signature in-bracket line comments (four positions)

The line-comment companion to
[index_signature_bracket_comment_positions](../index_signature_bracket_comment_positions_svelte_divergence/)
(the block-comment version). Four line-comment positions inside a **class**
index signature's `[key: string]` brackets, one per class:

- **A — `[`→key** (`[ // a`): tsv keeps the comment on the `[` line; prettier
  relocates it to its own line as the key's leading comment.
- **B — key→`:`** (`key // b⏎:`): both formatters keep it after the key and break
  the bracket so the `//` can't swallow `: string` — a **match**.
- **C — `:`→key-type** (`key: // c`): tsv drops the key type to a continuation
  line indented one level (uniform forced-continuation indent); prettier keeps it
  flush.
- **D — key-type→`]`** (own-line `// d` before `]`): tsv keeps the comment on its
  own line; prettier pulls it up to trail the key type (`key: string // d`).

The class, interface, and type-literal index-signature printers share
`build_index_signature_member_doc`, so this is the class face of the type-member
fixtures
([open_bracket](../../../types/type_members/index_signature_open_bracket_line_comment_svelte_prettier_divergence/),
[key_colon](../../../types/type_members/index_signature_key_colon_line_comment_svelte_prettier_divergence/),
[key_type](../../../types/type_members/index_signature_key_type_line_comments_svelte_prettier_divergence/),
[close_bracket](../../../types/type_members/index_signature_close_bracket_line_comment_prettier_divergence/)).

## Formatter — prettier divergence

Positions A, C, and D differ from prettier; B matches. Every difference is the
same preserve-the-author's-placement rule tsv applies at every comment site —
prettier relocates (A to its own line, D up to the key type) or keeps flush (C),
tsv holds each comment where it was written and indents forced continuations one
level. Prettier is non-idempotent on A and D (its first-pass `output_prettier.svelte`
converges over two passes — pinned by `audit_signature.txt`). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation, §Uniform forced-continuation indent, and §Comment Position
Philosophy.

## Parser — svelte divergence

For the comment **before the key** (A) and **after the key** (B),
acorn-typescript's backtrack-and-reparse duplicates the comment in the root
`comments` array (10 entries vs our 8); our parser keeps a single entry per
comment. The AST is semantically equivalent (`ast_diff` confirms match) — only
the duplicate count differs, exactly as in the block-comment fixture. See
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment
Attachment Differences.
