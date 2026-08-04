# Divergence: `export`→`=` gap comment (preserve)

A block comment between `export` and the `=` of an export-assignment (`export /* c */ = value;`).
tsv keeps it after `export`; prettier **relocates** it past the `=` onto the value.

```ts
// tsv (preserve)          // prettier (relocate past the `=`)
export /* c */ = value;    export = /* c */ value;
```

**Why tsv preserves:** the sibling gap decides it. `export /* c */ const x = 1` keeps the comment
after `export` in **both** formatters; `export /* c */ =` is the same gap after the same keyword,
so preserving keeps the `export`→X family reading one way. The `=` here is part of the `export =`
construct, not a list separator — the *pure separator* carve-out (which is what lets tsv trail past
a comma) does not reach it.

The operand-side gap already matches prettier (`export = /* c */ value` is stable in both) — see
[export_equals_operand_paren_comment](../export_equals_operand_paren_comment/) and the plain
[export_equals](../export_equals/).

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
