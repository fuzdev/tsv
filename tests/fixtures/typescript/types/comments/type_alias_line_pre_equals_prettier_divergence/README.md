# type_alias_line_pre_equals_prettier_divergence

Line comment between a type alias head (name + optional type parameters) and the
`=` (`type A<X> // c⏎= B | C`), with an **inline union** value.

**tsv**: keeps the comment trailing the head and drops `= value` to a
continuation line **indented one level** — the uniform forced-continuation
indent, the same shape as the other before-`=` initializer sites (declarators,
enum members, class properties):

```ts
type A<X> // c
	= B | C;
```

**Prettier**: with an inline union value it **relocates** the comment past the
value to end-of-statement (`type A<X> = B | C; // c` — `output_prettier.svelte`,
one pass; the merge-prone destination the declarator family pins). With a
non-union value it instead hangs the comment leading the RHS — see
[name_before_eq_line_comment](../../aliases/name_before_eq_line_comment_prettier_divergence/),
which pins that chain.

The **own-line** authoring (`type A<X>⏎// c⏎= B | C`) pulls up to trail the head
and reaches input under tsv — one pass (`unformatted_ours_own_line.svelte`).
Prettier instead crosses the `=` and hangs the comment leading the value
(`type A<X> =⏎\t// c⏎\tB | C;` — `variant_own_line.svelte`, dual-stable: the
comment then leads the value, a position both formatters preserve).

This fixture keeps the with-type-params coverage (the comment here was once
dropped entirely when type parameters were present — content loss); the
break-forced union sibling is
[type_alias_line_pre_equals_break](../type_alias_line_pre_equals_break_prettier_divergence/).
See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
