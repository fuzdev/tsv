# redundant_paren_shell_line_comment_prettier_divergence

A **redundant** paren shell around a type whose **trailing** gap holds a **line** comment, at
the seven positions where a keyword→value seam strips the shell before the shell's own
emitter can retain it: the `as` / `satisfies` cast, a `: T` annotation, a type-alias `=` RHS,
a type-parameter `=` default, a mapped-type `]:` value, a type-predicate `is`, and a
conditional type's `extends`. The redundant-pair face of
[required_paren_shell_line_comment](../required_paren_shell_line_comment_prettier_divergence/),
which answers the same question where the author's shell and a pair the construct *requires*
are the same pair.

**tsv**: retains the shell and opens it, every comment keeping the line it was written on:

```
const a = b as (
	// c1
	C // c2
); // c3
```

**Prettier**: strips the shell, hangs the leading run in the keyword→value gap, and carries
the trailing one out to the end of the enclosing line — where the comment that already sits
there is waiting (`const a = b as // c1⏎C; // c2 // c3`).

## Reason: the two comments are authored in DIFFERENT gaps and WELD

`// c2` is written inside the shell and `// c3` after it, and prettier's strip lands them on
one output line, where the second `//` becomes text of the first. That is content loss, and
it is irreversible — the merged form is a fixed point in both formatters, so no later pass
recovers the second comment. Prettier is not even self-consistent about the result: its own
output here needs **three** passes to settle (pinned by `audit_signature.txt`), and the
`is` case loses a comment position on the way.

Retention is the rule every bracketed type region already follows for its own trailing gap —
a type literal's `}`, a type-argument list's `>`, a tuple's `]`, a function type's `)`, an
indexed access's `]` — and the rule the shell's own emitter already applies wherever no
keyword→value seam gets to it first
([type_suffix_trailing_comment](../../declarations/variable/type_suffix_trailing_comment_prettier_divergence/)).
A `//` in one of these gaps forces its construct **open**: the closer drops to its own line
and the comment flushes inside, where nothing can land on top of it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **c1–c3** — the cast, with a **leading** run as well. The leading `//` is what takes the
  keyword→value hang, and the hang strips the very shell the trailing `//` retains.
- **c4–c5** — the same cast position with the trailing run **alone**. No leading run, so no
  hang — the cast strips the shell on its own second path, the one that defers a trailing
  comment past the statement `;`. Two sites, one rule.
- **c6–c8** — the `: T` annotation.
- **c9–c11** — the type-alias `=` RHS.
- **c12–c14** — the type-parameter `=` default, where the third comment stays inside the
  `<…>` rather than escaping past the `;`.
- **c15–c17** — the mapped-type `]:` value.
- **c18–c20** — the type-predicate `is`, where the third comment sits between the return type
  and the body's `{`.
- **c21–c23** — a conditional type's `extends`, the third keyword→value hang seam (a
  mixed-or-trailing shell declines the narrow trail-on-inner relocation and reaches it).
- **c24** — the control: a **leading** run with no trailing one still takes the hang, the
  shell stripping and the comment relocating into the keyword→value gap. The retention is
  keyed on the shell's TRAILING gap, and only on a `//` there — a comment never changes
  which parens are retained, only where it renders once they are.

## Files

`unformatted_ours_flat.svelte` carries the flat authoring — the shell's interior written at
no indent — which reaches `input` under tsv only; prettier welds from either authoring. It
deliberately leaves each leading comment on its own line rather than gluing it to the `(`:
whether a `//` the author glued to an opening delimiter stays there is a separate question
from this one, answered per delimiter elsewhere, and a variant is no place to settle it. The
control's shell is stripped either way, so it keeps its glued authoring — that is what
selects the keyword-trailing placement of `// c24` (two authorings, two fixed points).

`audit_signature.txt` pins prettier's chain out of `output_prettier.svelte`, which takes
three passes to reach a fixed point.
