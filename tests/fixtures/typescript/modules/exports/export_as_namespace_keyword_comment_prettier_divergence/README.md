# Divergence: `export as namespace` keyword-interior comments (preserve)

Block comments in the two keyword-interior gaps of a UMD global export header
(`export /* c1 */ as /* c2 */ namespace Foo;`). tsv keeps each where the author wrote it; prettier
**relocates both** past the whole keyword, stacking them before the name.

```ts
// tsv (preserve each)                        // prettier (both collapse onto the name)
export /* c1 */ as /* c2 */ namespace Foo;    export as namespace /* c1 */ /* c2 */ Foo;
```

**Why tsv preserves:** prettier's relocation **collapses two distinct positions into one**. After
it, "before `as`" and "before `namespace`" are indistinguishable — the text survives, the
association does not. A keyword's words are not a *pure separator* (the one sanctioned reason to
trail), so each gap is a position an author can mean.

The third gap — `export as namespace /* c */ Foo` — is **not** a divergence: the comment is already
in prettier's final position and both formatters keep it there. It lives in the regular sibling
[export_as_namespace_name_comment](../export_as_namespace_name_comment/). (tsv dropped it too, but
that was plain content loss, not a difference of opinion.)

The same shape as the export-all header ([all_keyword_comment](../all_keyword_comment_prettier_divergence/)),
where prettier likewise relocates every header comment to one position and tsv preserves each.

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
