# after_comma_block_then_line_prettier_divergence

An object-literal property with a block comment **after** the comma plus a line comment
after it (`p1: 1, /* c1 */ // c2`). tsv keeps the block on the comma line where the author
wrote it, in front of the line comment; prettier relocates the block to **before** the
comma.

```
// tsv                          // prettier
const a = {                     const a = {
	p1: 1, /* c1 */ // c2           p1: 1 /* c1 */, // c2
	p2: 2                           p2: 2
};                              };
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy), and here
the placement is also the only one that keeps the run in **source order**. The line
comment defers through `line_suffix`; a block left to lead the next property would render
after it, so the authored pair `/* c1 */ // c2` would come back as `// c2` … `/* c1 */` on
two lines. Keeping the block between the comma and the suffix reproduces the authored
arrangement exactly, and is idempotent.

A block with **no** line comment after it has nothing to defer behind, so it keeps leading
the next property (`d`) — the array-literal rule for an end-of-line block
([end_of_line_block_comment](../../arrays/end_of_line_block_comment_prettier_divergence/)),
unchanged. A block on the *before*-comma side stays there in both formatters (`c`).

The same after-comma preservation applies across every comma-separated site — call
arguments
([nonlast_arg_after_comma_block_then_line](../../calls/nonlast_arg_after_comma_block_then_line_prettier_divergence/))
and destructuring patterns
([after_comma_block_then_line](../../destructuring/after_comma_block_then_line_prettier_divergence/)),
which shares this printer's element-comma emitter.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
