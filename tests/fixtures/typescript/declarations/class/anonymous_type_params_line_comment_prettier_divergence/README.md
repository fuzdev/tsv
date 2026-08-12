# Anonymous class line comment between keyword and type params

Prettier keeps the comment in the gap the author wrote it in but **welds it to the
keyword** and leaves the continuation **flush**: `class // c⏎<T> {}` → `class// c⏎<T> {}`.
It is the one class-head gap prettier does not relocate — the named sibling goes to the
end of the declaration line (`name_type_params_line_comment`) and the anonymous
`class`→`{` gap goes into the body (`expr_anon_line_comment`).

We keep the space, the separator every sibling header gap emits (`class A // c`,
`const a = class // c⏎{}`), and indent the continuation one level: this is a declaration
header gap, so it takes the uniform forced-continuation indent rather than a rule of its
own.

Covers: class expression, class expression with heritage, default-exported anonymous
class.

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent),
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
