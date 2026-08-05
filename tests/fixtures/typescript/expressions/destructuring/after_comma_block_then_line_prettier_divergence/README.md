# after_comma_block_then_line_prettier_divergence

A destructuring-pattern element with a block comment **after** the comma plus a line
comment after it (`{ a1, /* c1 */ // c2 }`). tsv keeps the block on the comma line where
the author wrote it, in front of the line comment; prettier relocates the block to
**before** the comma.

```
// tsv                          // prettier
const {                         const {
	a1, /* c1 */ // c2              a1 /* c1 */, // c2
	b1                              b1
} = o;                          } = o;
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy), and here
the placement is also the only one that keeps the run in **source order**: the line
comment defers through `line_suffix`, so a block left to lead the next element would
render after it and the authored pair would come back reversed on two lines.

Object and array patterns share the object literal's element-comma emitter, so the two
answer this identically — see
[objects/after_comma_block_then_line](../../objects/after_comma_block_then_line_prettier_divergence/)
for the literal's cases, including the contrast case: a block with no line comment after
it has nothing to defer behind and keeps leading the next element. A block on the
*before*-comma side stays there in both formatters (`a4`).

The same after-comma preservation applies at every comma-separated site — call arguments
([nonlast_arg_after_comma_block_then_line](../../calls/nonlast_arg_after_comma_block_then_line_prettier_divergence/)).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
