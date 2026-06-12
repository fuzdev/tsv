# type_args_open_angle_comment_prettier_divergence

A line comment trailing a call/`new` _expression_ type-argument list's opening
`<` on the same line (e.g. `foo< // c`) is preserved on the `<` line. Prettier
relocates it to its own line as the first argument's leading comment.

This is the type-argument list of a call/`new` _expression_ (`foo<A, B>(x)`,
`new Foo<A, B, C>(x)`), distinct from the type-_argument_ list in _type_ position
(`Map<A, B>`), which is covered separately by
[type_argument_open_angle_comment](../../../types/type_argument_open_angle_comment_prettier_divergence/),
and from the type-_parameter_ _declaration_ `<` (`function f<T>`), covered by
[type_params/open_angle_comment](../../../types/type_params/open_angle_comment_prettier_divergence/).

tsv: keeps the comment trailing `<` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                 // prettier
foo< // c1             foo<
	A,                       // c1
	B                      A,
>(x);                    B
                       >(x);
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `<` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable.

**Multi-argument only.** A single-argument list with a leading line comment
(`foo< // c\n A>(x)`) hugs `<`/`>` in both formatters and already matches
Prettier — it is not a divergence and is left unchanged. Empty argument lists
are not representable. Only the expanding multi-argument cases (a line comment
after `<`, or own-line content forcing a break) diverge.

This is the call/`new`-expression member of the open-delimiter family. Call and
`new` expression type arguments share one multiline path
(`build_type_parameter_instantiation_doc_with_line_comments`, `types/type_params.rs`);
it routes through the shared `Printer::delimiter_line_comment_prefix` helper
(with `build_leading_comments_multiline_after_delim` to drop the pulled comment
from the first argument) used by the object/array literal, destructuring,
block-body, `namespace`/`module`, class/interface/enum body, type literal,
import/export specifier, tuple-type, and type-_position_ type-argument cases.

Before this, the comment was dropped entirely (a content-loss bug — the
expression path lacked the leading-`<` line-comment detection, so the comment
fell through to the block-comment-only group path).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
