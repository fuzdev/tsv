# Divergence: default-binding separator `,` gaps, line comment (preserve, one indent level)

A *line* comment between a leading default binding and the clause after it — the named-specifier
`{` or the namespace `*` — on either side of their separating `,`. tsv keeps it trailing the comma
and drops the rest of the clause to a continuation line indented **one** level; prettier
**relocates** it, past the whole statement's `;` for a single comment, or into the braces as the
first specifier's leading comment for a run.

```ts
// tsv (preserve)              // prettier (relocate past the `;`)
import a, // c1                import a, { b } from './a'; // c1
	{ b } from './a';
```

This is the separator-gap member of the family the `import` keyword→`{` gap
([keyword_brace_comment](../keyword_brace_comment_prettier_divergence/)) and the namespace header
gaps ([namespace_keyword_comment](../namespace_keyword_comment_prettier_divergence/)) already pin —
the same gap question one token later, on the far side of the `,` a leading binding opens, and the
last module-header gap that did not take the uniform continuation indent.

The comma is a **pure separator**, so a `//` authored *before* it trails past it
(`unformatted_ours_before_comma.svelte`) — the carve-out the braced specifier list already takes
between its own elements (`a // c⏎, b` → `a, // c`) — and the two authorings reach one fixed point.
A **block** comment is unaffected and matches prettier in both arms, which is where the two arms
part: prettier hoists a block *before* the comma for a named list (`import a, /* c */ {b}` →
`import a /* c */, {b}`) but leaves it on its authored side for a namespace, and tsv matches each.
The block cases are the non-divergent [separator_comment](../separator_comment/) sibling; `c3`/`c4`
here pin that a block and a line comment in the same gap keep their order rather than merging, and
`c7`/`c8` that the braces nest **with their own breaks** under the continuation — a claim only an
emitter inside the braces doc can make.

`c9`/`c10` pin the multi-gap header: this gap and the namespace's own `*`→`as` gap together keep
the whole header at **one** indent level, never a staircase — the rule
[export_as_namespace_line_comment](../../exports/export_as_namespace_line_comment_prettier_divergence/)
states for the export side, and the reason the `as` binding rides *outside* this gap's indent rather
than inside its continuation. Prettier's form there is **information-destructive**: it merges the two
comments onto one line in reverse order (`* as // c10 // c9`), the second `//` becoming text inside
the first. tsv keeps each distinct, in order, on its own line.

Preserving is a **content-loss** fix, not a layout preference: tsv previously emitted the whole gap
inline ahead of the comma (`import a // c1, { b } from './a';`), running the `//` over the comma,
the specifiers, `from`, the source and the `;` — lost CODE, and output that does not reparse.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(the module-header enumeration under **Declaration- and module-header line-comment continuation indent**).
