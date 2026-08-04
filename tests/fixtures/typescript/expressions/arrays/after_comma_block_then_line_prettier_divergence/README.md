# after_comma_block_then_line_prettier_divergence

An array element with a block comment **after** the comma plus a line comment after it
(`'aaaa', /* c1 */ // c2`). tsv keeps the block on the comma line where the author wrote
it, in front of the line comment; prettier relocates the block to **before** the comma.

```
// tsv                          // prettier
const a = [                     const a = [
	'aaaa', /* c1 */ // c2          'aaaa' /* c1 */, // c2
	'bbbb'                          'bbbb'
];                              ];
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy), and here
the placement is also the only one that keeps the run in **source order**. The line
comment defers through `line_suffix`; a block left to lead the next element would render
after it, so the authored pair `/* c1 */ // c2` would come back as `// c2` … `/* c1 */` on
two lines.

That is the one thing the sibling
[end_of_line_block_comment](../end_of_line_block_comment_prettier_divergence/) rule cannot
do here. It keeps an after-comma block leading the next element — still true when nothing
defers behind it (`e`) — but with a line comment in the same gap, leading the next element
and preserving the run are no longer the same thing, and the run wins.

A block on the *before*-comma side stays there in both formatters (`c`). Object literals,
destructuring patterns and import/export specifiers answer this identically through the
shared element-comma emitter — see
[objects/after_comma_block_then_line](../../objects/after_comma_block_then_line_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
