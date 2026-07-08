# Divergence: line comment on a class computed-key `[` line

A line comment the author wrote on the same line as a class computed member's
opening `[` (`[ // c`), for both a computed method and a computed property.

## Formatter — prettier divergence

Prettier keeps the comment on the `[` line but glues the key (and `]`) flush to
it (`[// c⏎foo]`); tsv keeps it on the `[` line, breaks the bracket, and indents
the key one level.

```ts
// prettier                  // tsv (preserves placement)
class C {                    class C {
	[// c                         [ // c
	foo]() {}                         foo
}                                ]() {}
                             }
```

This is the same preserve-in-place rule tsv applies to every other open
delimiter (object `{`, array `[`, block `{`, type-param `<`, function-type `(`,
index-signature `[`, …) via the shared `build_computed_key_bracket_doc`. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation ("Object/array/block open-delimiter trailing") and §Comment
Position Philosophy. A comment on its own line, and a block comment hugging `[`
(`[/* c */ foo]`), are unchanged — both match prettier.
