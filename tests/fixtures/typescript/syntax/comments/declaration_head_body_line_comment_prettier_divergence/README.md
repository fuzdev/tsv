# Divergence: declaration head→body `{` line comment

A **line** comment in a declaration's head→body-`{` gap (`function a() // c⏎{`). tsv keeps
it where the author wrote it — trailing the head, with the body `{` dropped to its own
line, flush with the head. Prettier **absorbs** it into the body
(`function a() {⏎\t// c⏎\tfn();⏎}`), a move across the `{` the comment was written outside
of, and one that re-binds it from "about this declaration" to "about the body's first
statement" (`output_prettier.svelte`).

```ts
// tsv (preserve)              // prettier (absorb into the body)
function a() // c              function a() {
{                              	// c
	fn();                      	fn();
}                              }
```

The `{` **must** drop to the next line: emitted inline, the `//` would swallow the brace
(`function a() // c {`), output that does not reparse. That is the same content-preservation
argument the class/interface heritage gap already makes, and the same landing it already
takes — the two cases at the end of this fixture (`class B // c⏎{`, `interface C // c⏎{`)
are the controls, unchanged by this rule and cataloged as
[Heritage last item before `{`](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
The value-level function definitions — function declaration, class method, getter,
constructor, function expression, object method — answering the same gap prettier's
way there, absorbing into the body, would make the catalog's "consistent with tsv's handling of
line comments before block bodies across all statement types" false of exactly this family.
They now share the one answer.

A **block** comment in the same gap carries no such hazard: a single-line block collapses
onto the head line with `{` hugging it
([declaration_head_body_comment](../declaration_head_body_comment/)), and a multiline block
the author broke after keeps its break
([declaration_head_body_multiline_block_break](../declaration_head_body_multiline_block_break/)
and its
[divergent sibling](../declaration_head_body_multiline_block_break_prettier_divergence/)).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
