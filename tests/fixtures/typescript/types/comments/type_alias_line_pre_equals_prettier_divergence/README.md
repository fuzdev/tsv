# type_alias_line_pre_equals_prettier_divergence

Line comment between a type alias head (name + optional type parameters) and the
`=` (`type A<X>\n// c\n= B | C`).

**Prettier**: relocates the comment to after `=`, associating it with the value:
```
type A<X> =
	// c
	B | C;
```

**tsv**: keeps the comment before `=`, associating it with the declaration head:
```
type A<X>
	// c
	= B | C;
```

Per Comment Position Philosophy: the comment sits before `=`, so tsv keeps it on
the head side rather than relocating it across the operator. The own-line
placement is preserved on both sides. Both positions are dual-stable in our
formatter.

Previously tsv **dropped** this comment entirely when type parameters were
present — a SAFETY/content-loss bug. Preserving it before `=` both fixes the loss
and keeps the user's chosen association. A single-line block comment before `=`
stays inline in both formatters (`type A<X> /* c */ = B | C`, see the regular
[type_alias_block_pre_equals](../type_alias_block_pre_equals/) fixture).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
