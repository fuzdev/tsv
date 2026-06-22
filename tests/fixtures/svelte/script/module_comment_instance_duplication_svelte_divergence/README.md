# Parser divergence: module-script comment duplicated onto the instance script

Svelte parses the `<script module>` and instance `<script>` with one shared
`root.comments` array (see `get_comment_handlers` /
`add_comments` in `svelte/.../1-parse/acorn.js`). The instance parse's
`add_comments` walk is not given a fresh queue, so **every** comment that
precedes the instance script's first node — including the module script's own
leading (`// shared note`) and trailing (`/* trailing in module */`) comments —
is shifted into `instance.content.body[0].leadingComments`. The comment is thus
attached **twice**: once at its real home in the module script, once again on
the instance script's first statement.

**tsv attaches each comment once, in its source region** (`expected_ours.json`
vs `expected_svelte.json`): the module comments stay on the module body
(`leadingComments` / `trailingComments`), and the instance body[0] carries no
leak. The set of distinct comments is identical — only the cross-script
duplication differs — and `ast_diff` confirms semantic (code) equivalence.
This is the same anti-duplication stance tsv takes on acorn-typescript's
backtrack-reparse comment duplication.

Formatting is unaffected: the formatter locates comments by position and emits
each once at the author's placement, so both scripts round-trip to the input.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
