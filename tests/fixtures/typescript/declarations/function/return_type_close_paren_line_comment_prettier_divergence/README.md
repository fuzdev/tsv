# return_type_close_paren_line_comment_prettier_divergence

A line comment in the `)`→return-type-`:` gap (`f(a: string) // c⏎: string`) is
preserved where the user placed it, trailing `)`, with `:` forced onto the next
line. Prettier instead relocates the comment **onto the last parameter** and
breaks the params.

tsv: keeps the comment after `)` (`f(a: string) // c⏎: string {}`)
Prettier: moves it onto the last param (`f(⏎\ta: string // c⏎): string {}`)

```
// tsv                       // prettier
function f(a: string) // c   function f(
: string {                       a: string // c
	return a;                ): string {
}                                return a;
                             }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy): a comment parked after `)` is a trailing comment in that gap, and
moving it onto a parameter is a syntactic-position move. tsv preserves it in
place, which is idempotent.

A `//` there can't stay inline — it would swallow the return type
(`f(a: string) // c : string {}` — the `: string {}` absorbed into the comment,
invalid and non-idempotent); forcing `:` onto the next line keeps the comment
where the author wrote it and the return type intact. This is the same rule tsv
applies at every keyword→value single-slot gap: a line comment hangs the operand
on its own line rather than swallowing it. It is the return-type-`:` counterpart
of the function-type `)`→`=>` gap
([pre_arrow_param_line_comment](../../../types/function_type/pre_arrow_param_line_comment_prettier_divergence/)).

Covers function declarations, class methods, function expressions, object
methods, and type-member method signatures — all funnel through the one `)`→`:`
comment emission. A block comment in the same gap stays inline
(`f(a: string) /* c */: string`), matching prettier.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
