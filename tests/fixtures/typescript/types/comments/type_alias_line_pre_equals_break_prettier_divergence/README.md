# type_alias_line_pre_equals_break_prettier_divergence

Line comment between a type alias head (name + optional type parameters) and the
`=`, where the value is a **break-forced union** (`type A<E, EM, D> // c0⏎= | a | b | c`
with interleaved member comments). The break variant of the inline
[type_alias_line_pre_equals](../type_alias_line_pre_equals_prettier_divergence/)
(the same head→`=` divergence, but there the value stays inline).

**tsv**: keeps the comment trailing the head (the uniform forced-continuation
indent, like every other before-`=` initializer site); the `=` drops to a
continuation line and the union members hang at the **`=` level** — byte-identical
to prettier's union block with `=` lifted above it:

```ts
type A<E, EM, D> // c0
	=
	| { type: 'a' }
	// c1
	| { type: 'b'; error: E; msg: EM }
	// c2
	| { type: 'c'; data: D };
```

**Prettier**: relocates the head comment across the `=`, associating it with the
value — reached non-idempotently: its first pass glues the comment to the `=`
(`type A<E, EM, D> = // c0⏎| …` — `output_prettier.svelte`, unstable; the fixed
point is pinned by `audit_signature.txt`), its second drops it to its own line
leading the first member (`variant_own_line.svelte`, dual-stable — the comment
then leads the value, a position both formatters preserve).

The **own-line** authoring (`type A<E, EM, D>⏎// c0⏎= | …`) pulls up to trail
the head and reaches input under tsv — one pass
(`unformatted_ours_own_line.svelte`); prettier takes it to the hang in one pass.
`unformatted_ours_double_indent.svelte` (members one level deeper) normalizes to
input under tsv too. A non-hugged union is the only value kind whose broken form
leads every member with `|` on its own line, so `=` correctly drops onto its own
line rather than hugging a first member; the members then hang at the `=` level,
not one deeper. The same layout applies whether the union breaks from
interleaved member comments (shown here) or from print width.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
