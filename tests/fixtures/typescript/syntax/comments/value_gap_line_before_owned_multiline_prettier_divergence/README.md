# value_gap_line_before_owned_multiline_prettier_divergence

A `//` in a value gap with an **owned multiline block** below it — the block the author
glued to the value, which the value's own doc prints. Prettier reorders the block ahead
of the `//`, and because the block spans lines the `//` lands **inside it**:

```ts
const a = // c1          →   const a = /* owned // c1
	/* owned                              ⏎
                                          */ 1;
	*/ 1;
```

`// c1` stops being a comment and becomes text of `/* owned */` — two comments in, one
out. The form is a fixed point in prettier, so the loss is silent and permanent. tsv
keeps both, in the lines and the order the author wrote them.

| row | prettier | lossless? |
| --- | --- | --- |
| declarator `=`, **multiline** block | merges `// c1` into the block | **no** |
| declarator `=`, single-line block (control) | moves `// c1` to trail the statement | yes |
| class property `=` | merges | **no** |
| enum member `=` | merges | **no** |
| assignment `=` (control) | agrees with tsv | — |
| arrow body `=>` (control) | agrees with tsv | — |

The single-line row is the null control: it varies only the block's own line count, and
that alone decides whether prettier's reorder destroys the comment or merely moves it.
The two agreeing rows are the same authoring at gaps prettier does not reorder at all.

## Reason

◆content_preservation. Relocating a comment across a syntactic boundary is what tsv
declines by default; here the relocation additionally **loses a comment**, which is the
argument the whole comment-position stance rests on rather than a taste call.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
