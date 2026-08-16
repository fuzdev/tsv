# Divergence: dotted namespace/module head dot-gap line comments (preserve)

A *line* comment in either gap around the `.` of a dotted namespace or module **declaration head**
(`namespace A // c⏎.B {}`, `namespace C. // c⏎D {}`). tsv keeps it where the author wrote it and
continues the rest of the name one level down; prettier **relocates** it — first past the `{`,
then, on a second pass, into the body as its first statement's leading comment.

```ts
// tsv (preserve)          // prettier (relocate, 2 passes)
namespace A // c1          namespace A.B {
	.B {                       // c1
		export const a = 1;      export const a = 1;
	}                        }
```

The declaration-head site of the rule the qualified name already states — `namespace A.B {}` is a
`name` `.` `name` chain and routes through the very same printer, `build_dotted_pair_doc`, as
[types/qualified_name_dot_gap_line_comment](../../../types/qualified_name_dot_gap_line_comment_prettier_divergence/)
and
[expressions/misc/meta_property/dot_gap_line_comment](../../../expressions/misc/meta_property/dot_gap_line_comment_prettier_divergence/).
One printer serves all three, so none can drift; the `module` spelling and a deeper `E.F.G` chain
are the same path again, not separate rules.

Block comments in these gaps are **not** a divergence: prettier keeps each on its authored side of
the dot, and so does tsv — pinned by the regular sibling
[nested_dot_comment](../nested_dot_comment/).

The run case is why relocating is not an option tsv could adopt even if it wanted the position.
Prettier's fixed point is `namespace H.I {⏎// c5 // c4`: the two comments are **reordered** and
**merged** onto one line, so `// c4` stops being a comment and becomes text inside `// c5` —
content loss, and the same merge the enum/namespace header→`{` gap makes one step further out.
tsv keeps each on its authored line, in order.

Prettier is non-idempotent on its own output here (the comment lands on the `{` line at pass 1 and
inside the body at pass 2), so `audit_signature.txt` pins the rest of the chain.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [§Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
(the punctuator-joined member of that class), plus
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
for the one-level continuation.
