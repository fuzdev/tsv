# Callee, `new` callee and tag pair, comment in a stripped operand pair

The callee twin of
[as_operand_shell_stripped_operand_paren_comment](../../parenthesized/as_operand_shell_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of a callee whose
required pair the printer emits (`(a + (b // c⏎) // d⏎)()`) stays right after that pair, ahead of
the comment written after the inner pair, which follows on its own line, and the argument list
drops below both. A call with arguments, an optional call, a call with type arguments, a `new`
callee and a template tag take the same place, as do nested pairs, an own-line `//` in the inner
pair and a block written ahead of the comment after the pair
(`(a + (b // c⏎) /* x */ // d⏎)()` prints `(a + b) // c⏎\t/* x */ // d⏎\t();`). A block alone after
the pair, or nothing at all, leaves the comment after the pair in the same way, ahead of the
arguments.

tsv: `(a + b) // c⏎\t// d⏎\t();`, the two comments in the order written, each on its own line —
the form tsv gives the bare spelling (`(a + b // c⏎// d⏎)()`)
Prettier: breaks the callee's pair open and keeps the continuation flush
(`(⏎\ta + b⏎) // c⏎// d⏎();`, `output_prettier`), moving the comment written after the inner pair
inside a non-empty argument list; from
the stripped-pair spelling at a template tag it puts the two on one line, where they weld into
one comment (`(a +⏎\tb) // c // d⏎\`t\``, `audit_signature_shell`)

A call-headed chain takes the callee's form (`(a + b) // c⏎\t// d⏎\t().m;`), and a tag with type
arguments keeps the comment ahead of them (`(a + b) // c⏎<T>\`t\``), as the bare spelling does. A
member chain's head already keeps the two apart — a control.

A function or arrow callee or tag keeps its own pair's interior instead, and there the comment
written last in a stripped shell down its right edge is that interior: it prints inside the pair,
which breaks open, as the bare spelling's does (`(() => a + (b // c⏎))()` prints
`(⏎\t() => a + b // c⏎)();`, the form both formatters give `(() => a + b // c⏎)()`), at a
chain's head and at a tag too. A comment written after the pair stays after it, in the order
written (`(() => a + (b // c⏎)) // d⏎()` prints `(⏎\t() => a + b // c⏎) // d⏎\t();`), where
prettier moves it inside the pair, below the first (`output_prettier`), and from the
stripped-pair spelling puts the two on one line, where they weld into one comment
(`audit_signature_shell`). A comment the author writes after the statement stays there — a
control. A block in a shell under the pair the printer adds around a conditional or assignment
arrow body prints past that pair (`(() => q ? r : a + (b /* c */))()` prints
`(() => (q ? r : a + b) /* c */)();`), as both formatters print the bare spelling, where prettier
keeps it inside the pair from the stripped-pair spelling.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence),
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
