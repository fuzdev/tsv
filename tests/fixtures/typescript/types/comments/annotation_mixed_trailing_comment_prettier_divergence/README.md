# annotation_mixed_trailing_comment_prettier_divergence

The mixed / trailing extension of
[annotation_continuation_indent](../annotation_continuation_indent_prettier_divergence/):
a redundant paren shell around a `: T` annotation type whose leading gap holds a
**block before the line comment** (mixed), or whose trailing gap holds a **block
after the type** (trailing).

**tsv**: strips the shell and hangs the run at the same fixed point the bare
authoring settles on — the block trails `:` inline, the line comment forces the
type onto the next line (the uniform forced-continuation indent), and a trailing
block trails the type before the member `;`:

```
interface I {
	a: /* b */ // c
		A;
	b: // c
		B /* t */;
}
```

**Prettier**: relocates the block before `:` (`a /* b */: A`) and floats the line
comment to trail the member `;` (`; // c`); the trailing block stays inline:

```
interface I {
	a /* b */: A; // c
	b: B /* t */; // c
}
```

Per Comment Position Philosophy, tsv keeps the comments after `:`, associated with
the type. The `unformatted_ours_*` variants verify the paren shells are idempotent
under tsv.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
