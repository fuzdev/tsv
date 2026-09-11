# Annotation `:`→type gap, own-line comment run

The **own-line** authoring of a comment between an annotation's `:` and its type. Trailing
the `:` (`let a: // c⏎\tB`) the comment stays put in both formatters and only the
continuation's indent diverges
([annotation_continuation_indent](../annotation_continuation_indent_prettier_divergence/));
here prettier **relocates** — it pulls the first comment up onto the `:` line and leaves the
type flush — while tsv keeps the line the author gave it and hangs the type one level in
below the run.

```
let a:                  let a: // c1
→// c1                  B;
→B;
```

## Reason

A comment in this gap **leads the type**, and own-line-ness is authoring signal for a
leading position — the corollary in
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy. The `:`→type gap is a value gap, so it takes the keyword→value
family's answer (`append_keyword_value_line_comments`): a same-line comment trails the `:`,
an own-line comment keeps its own line, and the type hangs one level in — the same shape as
the function type's `=>`
([function_type_return_own_line_line_comment](../../function_type_return_own_line_line_comment_prettier_divergence/))
and `keyof`. The gap already answered this way for an own-line `// prettier-ignore`
([annotation_prettier_ignore_own_line](../../annotation_prettier_ignore_own_line_prettier_divergence/)),
so a directive and a plain comment keep the same placement.

The break itself is not a layout choice on either side: a `//` runs to end-of-line, so
inlining the gap would swallow the type.

What the cases pin:

- **c1–c4** — a variable, a parameter, a class property and a property signature. At the
  property signature prettier is not idempotent — its second pass moves the comment past
  the `;` (`audit_signature.txt` pins the chain).
- **c5–c7** — a function declaration's, an arrow's and a method signature's return type.
- **c8** — an index signature's value.
- **c9–c11** — a union, an intersection and a hugging union hang as a simple type does. The
  multi-member union is the one type prettier also puts on its own line, from either
  authoring ([annotation](../annotation_prettier_divergence/)), so that case matches.
- **c12/c13** — a run keeps one comment per line, in order; prettier pulls only the first up.
- **c14/c15** — a **block ahead of the line comment** keeps its own line too, where prettier
  pulls the block up alone.
- **c16/c17** — a block on the `:` line stays there, and the author **blank** after it
  survives, since the break the `//` below forces is not layout the gap may collapse.
  Prettier keeps both as well; the divergence is the indent below.
- **c18** — the control: a comment authored **on** the `:` line trails it in both
  formatters, and only the indent diverges. Both authorings are stable under tsv — neither
  moves the comment to the other's line.

`unformatted_ours_compact` authors every case flush and unspaced;
`unformatted_ours_paren_shell` wraps several types in a redundant paren shell holding the
comment, which strips to the same fixed point — `c18`'s shell opens on the `:` line, so its
comment stays there; `unformatted_ours_leading_pipe` gives both unions an authored leading
`|`.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
