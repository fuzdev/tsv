# Divergence: line comment on the computed-key `[` line

A line comment the author wrote on the same line as a computed property key's
opening `[` (`[ // c`).

## Formatter — prettier divergence

Prettier relocates the comment out of the brackets to the property's own leading
line, keeping the key inline (`// c⏎[foo]`); tsv keeps it on the `[` line and
forces the bracket to break.

```ts
// prettier (relocates)      // tsv (preserves placement)
const o = {                  const o = {
	// c                          [ // c
	[foo]: 1,                         foo
};                               ]: 1,
                             };
```

This is the same preserve-in-place rule tsv applies to every other open
delimiter (object `{`, array `[`, block `{`, type-param `<`, function-type `(`,
index-signature `[`, …) via the shared `build_computed_key_bracket_doc`. See
[conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
("Object/array/block open-delimiter trailing") and §Comment
Position Philosophy. A comment the author wrote on its own line is likewise kept
in place (on its own line inside the broken bracket), and a block comment hugging
`[` (`[/* d */ bar]`) stays inline — both unchanged.
