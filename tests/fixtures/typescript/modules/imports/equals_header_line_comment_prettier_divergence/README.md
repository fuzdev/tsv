# Divergence: import-equals header line comments (preserve, one indent level)

A *line* comment in the gaps of the import-equals header. tsv keeps each where the author wrote it;
prettier **relocates** them — `// c3` past the `=` to trail the whole statement, `// c4` down onto
its own line, and `// c` in the `=`→module-reference gap onto the `require(` line.

```ts
// tsv (preserve)              // prettier (relocate)
export // c1                   export // c1
	import // c2               import // c2
	A // c3                    A = require('./a'); // c3
	= // c4                    // c4
	require('./a');
```

The line-comment counterpart of [equals_header_comment](../equals_header_comment/) (the
block-comment form, a *regular* fixture — prettier preserves four of the five header gaps, so there
is no opinion to state there).

Like [export_as_namespace_line_comment](../../exports/export_as_namespace_line_comment_prettier_divergence/),
it pins the **rendering**: a header with several broken gaps continues at **one** indent level, not
a staircase. These two headers are the only three-word ones, so no other keyword can show it.

What this fixture pins that its `export as namespace` sibling **cannot**: the module reference is
the one such tail that can break *internally*. Its own line breaks have to nest inside the
continuation level the gap established — a reference sitting at one level with its contents at that
same level, and its `)` a level *above* the `require(` it closes, would be wrong. So the gaps are
emitted flat and the whole tail — reference included — is wrapped once.

Prettier is **non-idempotent** on its own output here: a second pass moves `// c` off the `require(`
line onto its own line, which is why `output_prettier.svelte` carries an `audit_signature.txt`.

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
