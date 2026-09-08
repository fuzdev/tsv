# head_paren_shell_retained_gap_line_comment_prettier_divergence

The leading-edge paren shell of
[suffix_head_paren_shell_line_comment](../suffix_head_paren_shell_line_comment_prettier_divergence/)
at the one enclosing gap the family had not asked about: a **retained** paren shell's own
interior. The outer pair keeps its parens for its trailing run
([type_suffix_trailing_comment_union_member](../type_suffix_trailing_comment_union_member_prettier_divergence/)),
the inner shell strips, and the `//` therefore lands in the outer shell's leading region —
where that region's emitter, not the inner type, must print it.

Reading only the outer pair's own shallow gap left the run to be emitted from inside the
inner type's doc, at whatever indent it was built at and inside whatever groups it had
opened. Both effects are visible: a conditional inner **broke** on the pass that had the
inner shell and stayed flat on the pass that did not (`a extends b⏎↹? c⏎↹: d` vs
`a extends b ? c : d`), and a nested-intersection inner took **one indent level too many**.
Two authorings of one program, two fixed points.

The array and indexed-access links (`C`, `D`) already agreed and are the controls: neither
opens a group of its own, so the misplaced emitter had nothing to show.

## The divergence

Prettier strips the outer pair too and carries the whole trailing run out past the `;` — the
retain sanction linked above, which this directory inherits rather than adds to. What is new
here is only where the leading run prints inside a shell tsv retains.

⚠️ Prettier is **non-idempotent** on this shape: `audit_signature.txt` pins its chain from
`output_prettier.svelte` and `audit_signature_shell.txt` the chain from the paren authoring.

## Files

`unformatted_ours_shell.svelte` carries the inner-paren authoring, which reaches `input`
under tsv only.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
