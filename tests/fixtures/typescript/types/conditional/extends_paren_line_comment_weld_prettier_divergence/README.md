# extends_paren_line_comment_weld_prettier_divergence

The bound on the conditional-`extends` relocation that
[extends_paren_leading_line_comment](../extends_paren_leading_line_comment/) is the
canonical for. That fixture pins the rule: a redundant paren shell around the
`extends`-type whose leading gap holds a **pure line-comment run** strips, and the run
relocates to trail the extends-type — matching prettier, and lossless because the comment
ends up alone on that line.

It stops being lossless the moment a **second** line comment shares the destination. Three
runs land on that one line, and this fixture holds an authoring for each:

- **A** — a comment already trailing the extends-type (`T extends (// c1⏎U) // c2`).
- **B** — a **run** of two inside the shell itself (`T extends (// c1⏎// c2⏎U)`).
- **C** — the **true branch's own shell** run, which the breaking builder relocates onto
  this same line right behind the extends run (`T extends (// c1⏎U) ? (// c2⏎V) : W`,
  pinned by `unformatted_ours_branch_shell_run.svelte`). No window over this conditional's
  own gaps can see that contributor, so the count asks the branch shell directly.

The question is asked of the destination **line**, not of the gap: a comment the author put
on its own line between the extends-type and the `?` keeps that line and can never share
the destination, so it does not decline the relocation — see case I of
[extends_question_own_line_line_comment](../extends_question_own_line_line_comment_prettier_divergence/).

**tsv**: declines the relocation in both and keeps the shell's run in place (the
mixed / trailing hang), so every comment stays distinct and on its own line. Case A's
required pair is retained with it.

**Prettier**: relocates anyway and the two comments render back to back, where the second
`//` becomes text of the first — `// c1 // c2`, one comment where the author wrote two,
irreversibly (the merged form is a fixed point). Case **B** additionally **reorders** them
(`// c2 // c1`) and takes two passes to get there, pinned by `audit_signature.txt`.

**D** is the control: a single comment with nothing else on the destination line still
relocates, exactly as the canonical fixture says.

The rule is the one [conformance_prettier.md §Comment Position
Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
states — preserve when relocating would lose information — read as a bound on a relocation
tsv otherwise performs. See [conformance_prettier_ts_comments.md §Comment
relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
