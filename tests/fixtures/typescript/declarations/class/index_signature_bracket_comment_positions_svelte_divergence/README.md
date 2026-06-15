# Parser divergence: in-bracket class index-signature comment duplication

Tests comment positions inside a class index signature's `[k: string]` brackets
— before the key (`[/* c */ k]`), after the key (`[k /* c */ :]`), after the
colon (`[k: /* c */ T]`), and before the `]` (`[k: T /* c */]`).

For a comment **before the key** and **after the key**, acorn-typescript's
backtrack-and-reparse duplicates the comment in the root `comments` array; our
parser keeps a single entry. The AST is semantically equivalent (`ast_diff`
confirms match) and the other positions match canonical exactly — the difference
is only the duplicate count for those in-bracket comments.

This does not affect formatting — the formatter finds comments by position, not
by their count in the root array, and emits each comment once at the user's
placement (a block comment hugs `[`; a line comment before the key breaks the
bracket). The class, interface, and type-literal index-signature printers share
`build_index_signature_member_doc`, so the same handling applies in all three.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
