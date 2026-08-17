# aligned_object_open_brace_comment_prettier_divergence

A **line** comment the author wrote on an object type's opening `{` line, under the
**alignment** rendering a union member and a parenthesized-intersection member take. tsv
keeps it there; **prettier moves it into the body** as the first member's leading comment.

```
// tsv                          // prettier
type T =                        type T =
	| { // c1                       | {
			a: A;                           // c1
	  }                                 a: A;
	| B;                              }
                                  | B;
```

## Reason

The opening-delimiter rule, at the last construct in the family that still relocated: what
the author put on an opening delimiter's line stays on it. The **plain** type literal one
line down already answered this way
([type_literal_open_brace_comment](../type_literal_open_brace_comment_prettier_divergence/)),
as do `fn( // c`, `[ // c`, `{ // c`, `Array< // c` and every statement header — so a `{`
answered the question differently from the `{` beside it, keyed on nothing the author can
see: whether the object happened to be a union member.

The two paths differ only in *rendering* — the alignment path double-indents its members
and puts the closing `}` at the union member's `align(2)` offset — and a rendering
difference is no reason for a comment-position difference. Both now resolve the pull the
same way (`Printer::delimiter_line_comment_prefix`), so the first member's leading run
excludes the pulled comment in both.

Only the **forced-multiline** arm pulls, and it needs no gate of its own: a `//` (or an
own-line comment) is exactly what forces that arm, and an inline block already hugs the
first member on the width-aware one.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **c1** — a union member's object type, the alignment rendering's main shape.
- **c2** — a parenthesized-intersection member's trailing object, which reaches the same
  builder through its own opening (`(C & {`).
- **c3** — the control from the sibling path: the plain type literal, whose own builder
  already kept the comment on the `{` line. The two paths agreeing is the point.
- **c4** — the control the other way: written on its own line the comment keeps its own
  line, and **prettier agrees**.
