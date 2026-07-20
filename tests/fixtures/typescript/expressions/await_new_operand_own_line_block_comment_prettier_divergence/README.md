# await_new_operand_own_line_block_comment_prettier_divergence

A single-line block comment in an `await`→operand or `new`→callee gap collapses to the
inline form (`await /* a */ foo`, `new /* b */ Foo()`) — whether authored glued,
trailing the keyword, or on its own line. The own-line authoring
(`unformatted_ours_own_line.svelte`) is what tsv normalizes here, in **one** pass.

Both formatters reach the same fixed point; they differ in how many passes it takes.

- **tsv** collapses in one pass — the shared keyword→value rule
  (`comment_hangs_next`): a block comment forces nothing, so a break after it is the
  author's layout and is reflowed, exactly as at the `as`/`satisfies`, `export =`,
  and module-header gaps.
- **Prettier** is **non-idempotent** on the own-line authoring: its first pass pulls
  the comment up to the keyword but keeps the break (`await /* a */⏎foo;`), and only
  its second pass collapses that to `await /* a */ foo;`. The unstable first pass is
  pinned by `prettier_intermediate_own_line.svelte`.

The intermediate form is the bug this fixture guards: it is the *glued* authoring with
the break kept, so a formatter that emitted it would not be idempotent on its own
output. Only a **line** comment or an **own-line multiline** block still hangs the
operand (a glued multiline block collapses inline).

See [conformance_prettier.md §Authored breaks in value position](../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)
and [§Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
