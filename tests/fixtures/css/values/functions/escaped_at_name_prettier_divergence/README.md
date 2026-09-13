# escaped_at_name_prettier_divergence

A function whose **name** carries an escape spelling an `@` — `a\@b(2.5)`, `\@b(2.5)`, and
the same inside another function's arguments.

tsv: `a\@b(2.5)` — the escape is ident content, so `a\@b` is the single ident `a@b`, an
ident sequence glued to a `(` is a `<function-token>` (CSS Syntax 3 §"Consume an ident-like
token"), and its arguments normalize
Prettier: `a\@b (2.5)` — reads the escape's raw `@` byte as the start of an **at-word**, so
the value is the word `a\`, the at-word `@b` and a parenthesized group, and a group after a
non-empty at-word takes the ordinary member gap

## Reason

Same class as [escaped_delimiter_name](../escaped_delimiter_name_prettier_divergence/): which
*spelling* of a character the author used is the whole of the difference, because prettier's
tokenizer dispatches on the escape's payload byte rather than on the escape. `\@` is the
fifth such payload — the hex spelling `a\40 b(2.5)` is one name to both formatters and
agrees (a control here). Unlike `\)` / `\"` / `\'`, which unbalance prettier's value parse and
freeze the whole declaration, `\@` parses on both sides: only the member split differs, and
the arguments still normalize.

The `a\` half is invisible because of a second prettier rule, `printCommaSeparatedValueGroup`'s
"Ignore escape `\`" arm — a node whose value holds a backslash is printed glued to whatever
follows it — which is also what hides the split at a raw `@` (`a@b` → `a @b`, the control
below). The gap prettier does write is the at-word's own, on its right.

The name's **extent** is the same disagreement read one member over: with a second function
beside it (`a\@b(2.5) c(2.5)`) tsv closes the first call at its own `)` and starts a new member
at `c(`, while prettier's split has already put its gap after `a\`'s at-word, so its output
(`a\@b (2.5) c(2.5)`) spaces one member earlier and leaves the second call glued.

The bounding cells are all in the fixture: an **empty** `@` after the escape (`a\@(2.5)`,
where prettier's "Ignore `@` in Less" arm glues the group and the two agree), a name with no
`(` at all (`a\@b`), an **authored** gap (`a\@b (2.5)`, which both keep), the hex spelling
(`a\40 b(2.5)`), and the raw `@` (`a @b (2.5)`, a member of its own on both sides — see
[atword](../../operators/atword/)).

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Escaped `@` in a function name").
