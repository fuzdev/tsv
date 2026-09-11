# Function / constructor type `=>`→return gap, own-line comment run

The **own-line** authoring of a comment between a function type's `=>` and its return
type. Trailing the `=>` (`() => // c⏎\tB`) the comment stays put in both formatters and
only the continuation's indent diverges
([type_position_parens_leading_line_comment](../type_position_parens_leading_line_comment_prettier_divergence/));
here prettier **relocates** — it pulls the first comment up onto the `=>` line and
leaves the return type flush — while tsv keeps the line the author gave it and hangs the
return type one level in below the run.

```
type A = () =>          type A = () => // c1
→// c1               B;
→B;
```

## Reason

A comment in this gap **leads the return type**, and own-line-ness is authoring signal
for a leading position — the corollary in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy. The `=>`→return gap is a keyword→value gap, so it takes the
family's answer (`append_keyword_value_line_comments`, gated by
`keyword_value_hang_doc`): a same-line comment trails the `=>`, an own-line comment keeps
its own line, and the return type hangs one level in — the same shape as `keyof`
([type_operator_keyword_line_comment](../type_operator_keyword_line_comment_prettier_divergence/)),
`await` / `new`
([await_new_operand_own_line_line_comment](../../expressions/await_new_operand_own_line_line_comment_prettier_divergence/))
and a switch label's `case`→test. The gap's frozen arm already answered this way for an
own-line `// prettier-ignore`
([function_type_prettier_ignore_return](../function_type_prettier_ignore_return_prettier_divergence/)),
so a directive and a plain comment now keep the same placement.

The break itself is not a layout choice on either side: a `//` runs to end-of-line, so
inlining the gap would swallow the return type.

What the cases pin:

- **A/C/D** — the function type, the constructor type and the abstract constructor type,
  one builder for all three.
- **E** — a run keeps one comment per line, in order; prettier pulls only the first up
  and leaves the rest below, splitting one authored run across two positions.
- **F** — a **block ahead of the line comment** keeps its own line too, where prettier
  pulls the block up alone.
- **G** — a block on the `=>` line stays there, and the author **blank** after it
  survives, since the break the `//` below forces is not layout the gap may collapse.
  Prettier keeps both as well; the divergence is the indent below.
- **H/I** — a **hugging** union return (a brace member and a reference member beside
  `null`) prints with no group of its own and answers the gap as a simple type does.
- **J** — the same gap inside a parameter annotation, one level deeper.
- **K** — the control: a comment authored **on** the `=>` line trails it in both
  formatters, and only the indent diverges. Both authorings are stable under tsv —
  neither moves the comment to the other's line.

`unformatted_ours_compact` authors every case flush and unspaced;
`unformatted_ours_paren_shell` wraps several return types in a redundant paren shell
holding the run, which strips to the same fixed point — `K`'s shell opens on the `=>`
line, so its comment stays there.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
