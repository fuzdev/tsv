# after_export_default_paren_prettier_divergence

A decorated class written **inside a pair** after `export default`
(`export default (@dec class A {});`). The pair is not redundant: in the decorators grammar
(and to tsc) the bare `export default @dec class A {}` is a class DECLARATION, which binds `A`
in the module scope, and the parenthesized form is an export of a class EXPRESSION, which
does not — two programs. So tsv keeps the authored pair, and breaks it open around the
decorators the way it prints every decorated class in a required pair
(`(⏎\t@dec⏎\tclass {}⏎)`, as at a statement's start or an arrow's leftmost body).

A line comment before the `)` is the pair's and stays inside it, after the class, glued or
on its own line: past `);` it would land on the line a trailing `//` holds, and the two would
merge (`// c5 // c6`). A block comment there — glued or own-line — prints past `);`, as for
the undecorated class and as prettier prints it.

**Prettier** keeps the pair too, but prints a blank line below the `(` — its decorator list
opens with the hardline an export declaration's decorators take, inside a pair that already
broke a line — and moves the line comments before `)` past `);`, merging `// c5` into `// c6`
(`output_prettier.svelte`).

See [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
