# Parser divergence: in-bracket index-signature comment duplication

Tests all four comment positions inside an index signature's `[k: string]`
brackets — before the key (`[/* c */ k]`), after the key (`[k /* c */ :]`),
after the colon (`[k: /* c */ T]`), and before the `]` (`[k: T /* c */]`) — in
both interface and type-literal members.

For a comment **before the key** (both contexts) and **after the key** (type
literal only), acorn-typescript's backtrack-and-reparse duplicates the comment
in the root `comments` array; our parser keeps a single entry. The AST is
semantically equivalent (`ast_diff` confirms match) and the other positions
match canonical exactly — the difference is only the duplicate count for those
in-bracket comments.

This does not affect formatting — the formatter finds comments by position, not
by their count in the root array, and emits each comment once at the user's
placement (a block comment hugs `[`; a line comment before the key breaks the
bracket). See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
