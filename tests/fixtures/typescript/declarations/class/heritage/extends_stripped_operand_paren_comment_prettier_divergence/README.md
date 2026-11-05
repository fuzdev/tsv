# Superclass pair, comment in a stripped operand pair

The superclass twin of
[callee_stripped_operand_paren_comment](../../../../expressions/calls/callee_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of a superclass whose
required pair the printer emits (`class A extends (a + (b // c⏎) // d⏎) {}`) stays right after
that pair, ahead of the comment written after the inner pair, which follows on its own line, and
the body's `{` drops below both. A class expression, a block written ahead of the comment after
the pair, an `implements` clause and type arguments after the pair take the same place, and a
comment written alone in the inner pair, or followed by a block alone, stays after the pair too,
ahead of the body or of type arguments.

tsv: `class A extends (a + b) // c⏎// d⏎{}`, the two comments in the order written, each on its own
line — the form tsv gives the bare spelling (`class A extends (a + b // c⏎// d⏎) {}`)
Prettier: moves the comments into the class body (`class A extends (a + b) {⏎\t// c⏎\t// d⏎}`,
`output_prettier`), and with type arguments past the body, where the two weld into one comment in
the reverse order (`(a + b)<T> {} // d // c`, `audit_signature`); from the stripped-pair spelling
it welds the two ahead of `implements` (`extends (a + b)⏎\t// c // d⏎\timplements I`) and trails a
lone comment, or one behind a block alone, past an empty body (`class A extends (a + b) {} // c`,
`audit_signature_shell`)

See
[conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Heritage last item before `{`).
