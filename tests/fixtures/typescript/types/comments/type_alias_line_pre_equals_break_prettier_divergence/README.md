# type_alias_line_pre_equals_break_prettier_divergence

Line comment between a type alias head (name + optional type parameters) and the
`=`, where the value is a **break-forced union** (`type A<E, EM, D>\n// c0\n= | a | b | c`
with interleaved member comments). The break variant of the inline
[type_alias_line_pre_equals](../type_alias_line_pre_equals_prettier_divergence/)
(the same head→`=` divergence, but there the value stays inline on the `=` line).

**Prettier**: relocates the head comment to after `=`, associating it with the value,
and breaks the union with members at one indent level:
```
type A<E, EM, D> =
	// c0
	| { type: 'a' }
	// c1
	| { type: 'b'; error: E; msg: EM }
	// c2
	| { type: 'c'; data: D };
```

**tsv**: keeps the comment before `=`, associating it with the declaration head. The
comment forces `=` off the LHS line, and the union members sit at the **`=` level** — the
same indent the author wrote, and byte-identical to prettier's union block with `=` lifted
above it:
```
type A<E, EM, D>
	// c0
	=
	| { type: 'a' }
	// c1
	| { type: 'b'; error: E; msg: EM }
	// c2
	| { type: 'c'; data: D };
```

Per Comment Position Philosophy: the comment sits before `=`, so tsv keeps it on the head
side rather than relocating it across the operator (see the inline sibling). A non-hugged
union is the only value kind whose broken form leads every member with `|` on its own line,
so `=` correctly drops onto its own line rather than hugging a first member; the members
then hang at the `=` level, not one deeper. The same layout applies whether the union
breaks from interleaved member comments (shown here) or from print width.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
