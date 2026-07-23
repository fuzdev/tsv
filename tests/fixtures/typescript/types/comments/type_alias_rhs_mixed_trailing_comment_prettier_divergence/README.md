# type_alias_rhs_mixed_trailing_comment_prettier_divergence

A redundant paren shell around a type-alias RHS (`type A = (…)`) whose leading gap
holds a **block before the line comment** (mixed), or whose trailing gap holds a
**block after the value** (trailing). The type-alias `=` layout has its own builder
(`build_type_alias_eq_value_doc`), but routes the shell through the shared
keyword→value seam so the paren form settles on the same fixed point the bare
authoring does.

**tsv**: strips the shell — the block hugs the `=` line, the line comment forces
the value onto its own line below, and a trailing block trails the value:

```
type A = /* b */
	// c
	X;
type C = // c
	Y /* t */;
```

**Prettier**: breaks after `=` and drops the whole run onto its own line(s) below,
relocating the block off the `=` line:

```
type A =
	/* b */
	// c
	X;
```

Per Comment Position Philosophy, tsv keeps the leading block on the `=` line where
the author wrote it. The `unformatted_ours_*` variants verify the paren shells are
idempotent under tsv.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
