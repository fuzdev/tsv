# Value shell the printer keeps, comment in a stripped operand pair

The kept-value-shell twin of
[for/update_stripped_operand_paren_comment](../../../statements/for/update_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of a value whose
own shell the printer keeps — kept because a `//` follows the pair inside it
(`let k = (a + (b // c⏎) // d⏎);`) — stays inside the shell after the value, ahead of the
comment written after the pair, which follows on its own line. The positions are every one
whose value shell keeps a `//`: a declarator initializer (last or not), a plain and a compound
assignment, an arrow body, a `for` init, a conditional's alternate, an `export default`
value and an expression statement; nested pairs, an own-line `//` and a block written ahead of
the comment after the pair (`(a + (b // c⏎) /* x */ // d⏎)` prints `a + b // c⏎\t/* x */ // d`)
take the same place.

tsv: `(⏎\ta + b // c⏎\t// d⏎)`, the two comments in the order written, each on its own line —
the form both formatters give the bare spelling (`(a + b // c⏎// d⏎)`)
Prettier: strips the shell, and the comments trail the statement past the `;` — from the
input on lines of their own (`a + b; // c⏎// d`, its second pass, `audit_signature`) — at an
expression statement the block-then-`//` run welds behind the `;` (`a + b;⏎/* x */ // d // c`) —
and from the stripped-pair spelling on one line, where they weld into one comment
(`a + b; // c // d`, `audit_signature_shell`)

An own-line comment after the pair already takes a line of its own, and the positions that
strip the value's shell (a `return` argument, a call argument, a class field) already keep
the two apart — those are controls, as is the pair with nothing written after it, whose shell
strips and whose comment trails the statement in both formatters.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
