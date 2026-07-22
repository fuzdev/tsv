# import header multiline block comment

A **multiline** block comment in a module-header gap hangs what follows it, and tsv
indents that continuation one level — the same shape a *line* comment in the same gap
already produces, and the same shape every value gap produces for a multiline block
(`const x =⏎\t/* x⏎y */⏎\t1;`).

A multiline block is one of the two comment shapes that genuinely force a break (the
other is a line comment): the author broke after it, and reflowing would swallow the
`*/` line into the header. Once the break is forced, [§Uniform Forced-Continuation
Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
applies — a single statement spanning lines reads as a continuation, not a second
statement. Contrast the *single-line* block, which forces nothing and reflows inline
from any authored position
([header_own_line_block_comment](../header_own_line_block_comment_prettier_divergence/)).

The comment's own interior lines are left flush — tsv never re-indents a comment body.

Prettier instead relocates the comment into the braces (as the specifier's trailing
comment for `from`→source and specifiers→`from`, as a leading comment for
keyword→`{`), expanding them.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
