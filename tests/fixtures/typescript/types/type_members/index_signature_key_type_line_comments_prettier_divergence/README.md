# Divergence: line comment between an index-signature key's `:` and its type

A line comment in the key-`:`→key-type gap of an index signature
(`[k: // c\nKeyType]`). The key is a parameter whose type annotation is built by
the shared `build_type_annotation_doc`, so this is the same **uniform
forced-continuation indent** as every other `: Type` context: the comment trails
`:` where the author wrote it and the key type drops to a continuation line
**indented one level**.

```ts
// tsv (continuation indents one level)   // prettier
[                                         [
	k: // e                               	k: // e
		string                            	string
]: boolean;                               ]: boolean;
```

- **Union key** (`string | number`, cases A/D) — **matches** prettier: both indent
  the continuation one level.
- **Simple key** (`string`, cases B/C/E) — **diverges**: tsv indents, prettier
  leaves the type flush. Case B also carries a pre-`:` block comment
  (`k /* x */ :`), which both formatters keep inline; only the post-`:` line
  comment's continuation indent differs.

A **block** comment in this gap stays inline in both formatters and is not a
divergence — only a line comment (which runs to EOL, forcing the type onto its
own line) differs. Same rule across every `: Type` annotation context — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform forced-continuation indent and §Comment Position Philosophy.
