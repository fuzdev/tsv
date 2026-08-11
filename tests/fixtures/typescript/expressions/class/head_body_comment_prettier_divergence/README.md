# Divergence: class-expression head→body `{` comment

The class-**expression** face of the declaration head→body-`{` gap. tsv keeps the comment
where the author wrote it: a single-line block collapses onto the head line with `{`
hugging it, while a **line** comment or a **multiline** block the author broke after drops
`{` to its own line, flush with the statement. Prettier **absorbs** every one of them into
the class body, leading its first member (`output_prettier.svelte`) — a move across the `{`
the comment was written outside of, and the same answer it gives the class *declaration*
([relocated](../../../syntax/comments/declaration_head_body_own_line_block_relocated_prettier_divergence/)).

The four head shapes are the point of this fixture: a class expression reaches its body
brace by four routes — bare name (`class A`), anonymous (`class`), heritage
(`extends B`), and type parameters (`<T>`) — and they must give one answer. Only the
heritage and type-parameter routes went through the shared header→body seam; the bare-name
and anonymous ones emitted the gap themselves and so collapsed a broke-after multiline
block (`class A /* x⏎y */ {`) that their siblings kept, and the bare-name route emitted a
stray space before the brace (`class A // c⏎␣{`). That second one was a **stable fixed
point**, so idempotency, the ratchets and the whole gate were blind to it — only a prettier
comparison shows it. All four now resolve the gap through the one seam.

See [conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)
for the rule and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
for the catalog entry.
