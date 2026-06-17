# Divergence: line comment on the index-signature `[` line

A line comment the author wrote on the same line as an index signature's opening
`[` (`[ // b`).

## Formatter — prettier divergence

Prettier relocates the comment to its own line as the key's leading comment; tsv
keeps it on the `[` line and forces the bracket to break.

```ts
// prettier (relocates)      // tsv (preserves placement)
[                            [ // b
	// b                         key: string
	key: string              ]: number;
]: number;
```

This is the same preserve-in-place rule tsv applies to every other open
delimiter (object `{`, array `[`, block `{`, type-param `<`, function-type `(`,
…). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation ("Object/array/block open-delimiter trailing") and §Comment
Position Philosophy. A comment the author wrote on its own line, and a block
comment hugging `[` (`[/* b */ key]`), are unchanged — both match prettier.

## Parser — svelte divergence

acorn-typescript's backtrack-and-reparse duplicates a before-key comment in the
root `comments` array; tsv keeps a single entry (`expected_ours.json` vs
`expected_svelte.json`). The set of distinct comments is identical — only
multiplicity differs, and `ast_diff` confirms semantic equivalence. See
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment
Attachment Differences.
