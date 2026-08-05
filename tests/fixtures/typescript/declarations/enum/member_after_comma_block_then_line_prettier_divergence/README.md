# member_after_comma_block_then_line_prettier_divergence

An enum member with a block comment **after** the comma plus a line comment after it
(`A, /* c1 */ // c2`). tsv keeps the block on the comma line where the author wrote it,
in front of the line comment; prettier relocates the block to **before** the comma.

```
// tsv                          // prettier
enum E1 {                       enum E1 {
	A, /* c1 */ // c2               A /* c1 */, // c2
	B                               B
}                               }
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy), and here
the placement is also the only one that keeps the run in **source order**: the line
comment defers through `line_suffix`, so a block left to lead the next member would
render after it and the authored pair `/* c1 */ // c2` would come back as `// c2` …
`/* c1 */` on two lines.

The enum-member loop shares the object literal's element-comma emitter, so the two answer
this identically — see
[objects/after_comma_block_then_line](../../../expressions/objects/after_comma_block_then_line_prettier_divergence/)
for the literal's cases, including the contrast case: a block with no line comment after
it has nothing to defer behind and keeps leading the next element. A block on the
*before*-comma side stays there in both formatters (`E3`'s `c0`).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
