# union_hug_leading_line_comment_prettier_divergence

A **line** comment in the leading `|`→first-member gap of an otherwise *hugging* union
(`{ … } | null` — an object-like member with only void siblings). The comment must survive,
and it forces the union to expand.

The hug prints its members inline (`{ a: 1 } | null`) and emits that leading gap block-only,
so a line comment there has nowhere to go — it would be **dropped**, silent content loss. It
could not be inlined regardless: a `//` runs to end-of-line and would swallow the member. So
the hug is declined and the union expands, which is what prettier does too.

`union_prints_hugged` is the single source of truth for whether the hug actually happens,
because the union printer lays the members out while a **separate gate at each position that
can hold a union** decides whether to break after its keyword — and the two must agree. Asking
the bare syntactic `should_hug_union_type` at such a gate while the printer declines splits
them: the keyword keeps its operand glued (`type A = | // c⏎{ a: 1 }⏎| null`) while the members
explode below it, where a non-hugging union of the same shape correctly breaks after the
keyword.

Every one of those positions is covered here, because the syntactic form is necessary but not
sufficient once a comment can change the printer's mind — and each gate that re-derived the
answer had drifted:

- `=` — the type-alias RHS (`A`, `B`)
- `:` — an annotation (`d`), via `union_return_hugs`
- `=>` — a function-type return (`E`), same gate
- `as` — a cast (`f`), via `build_union_hanging_indent_doc`
- a mapped-type value (`G`)
- a conditional check (`H`) and branch (`I`)
- a type argument, in type (`J`) and expression (`k`) position

⚠️ **This list is load-bearing, and it has been wrong before.** It once named only the first
four, which read as "the vein is closed" — while the last five asked the bare syntactic
predicate and mangled every one of them. An enumeration that trails the code is worse than
none: it is what stops the next reader from probing. If you add a position that can hold a
union, add it here, and probe it against prettier rather than trusting this list.

`l`/`M`/`n` are the controls: the same positions with nothing to disqualify the hug, so it
still happens and the keyword stays glued. A **block** comment in the gap (`C`) also stays
hugged and inline — it renders fine there, nothing is lost, and prettier agrees. The
*between-members* block comment — the other clause that declines the hug, and the one the
narrower duplicate scan used to miss — is pinned by
[union_hug_member_block_comment](../union_hug_member_block_comment/), which fully matches
prettier (no alignment is involved, so it is not a divergence).

## The divergence

Only the **encoding of the member offset**. Both formatters offset the member past the `|` by
prettier's per-member `align(2, …)` (`union-type.js`); prettier emits that 2-column offset as
`tab + 2 spaces` under `--use-tabs`, tsv rounds it up to one whole tab — the same visual width
at `tabWidth = 2`. Identical in kind to case A of
[union_intersection_parens_leading_line_comment](../union_intersection_parens_leading_line_comment_prettier_divergence/),
and not specific to the hug — any union member with a leading line comment shows it.

See [conformance_prettier.md §Tabs-Only Alignment](../../../../../docs/conformance_prettier.md#tabs-only-alignment)
and the [Tabs-Only Indentation Philosophy](../../../../../docs/conformance_prettier.md#tabs-only-indentation-philosophy).
