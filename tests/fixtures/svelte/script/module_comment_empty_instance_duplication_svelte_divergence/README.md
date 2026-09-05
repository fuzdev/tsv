# Parser divergence: module-script comment duplicated onto a statement-less instance script

The statement-less spelling of
[../module_comment_instance_duplication_svelte_divergence](../module_comment_instance_duplication_svelte_divergence/):
Svelte parses the `<script module>` and instance `<script>` with one shared
`root.comments` array (`get_comment_handlers` / `add_comments` in
`svelte/.../1-parse/acorn.js`), so the instance parse's `add_comments` walk sees
the module script's `// shared note` ahead of the instance's own comments. With
no statement to lead, every comment reaches the walk's root special case (a
Program's leftover comments become its `trailingComments`), and the module
comment lands there **first**: `instance.content.trailingComments` is
`[shared note, first instance note, second instance note]`, so each of the
instance's own comments reads one index later than tsv lists it.

**tsv attaches each comment once, in its source region** (`expected_ours.json`
vs `expected_svelte.json`): the module comment stays on the module body's
first statement, and the instance Program's `trailingComments` holds exactly
the instance's own two. The set of distinct comments is identical — only the
cross-script duplication differs — and `ast_diff` confirms semantic (code)
equivalence.

Formatting is unaffected: the formatter locates comments by position and emits
each once at the author's placement, so both scripts round-trip to the input.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
