# Divergence: `export`→`default` keyword-interior comment (preserve)

A block comment inside the `export default` keyword (`export /* c */ default 1;`). tsv keeps it
after `export`; prettier **relocates** it past `default` onto the value.

```ts
// tsv (preserve)            // prettier (relocate past the keyword)
export /* c */ default 1;    export default /* c */ 1;
```

**Why tsv preserves:** the sibling gap decides it. `export /* c */ const x = 1` and
`export /* c */ function fn() {}` keep the comment after `export` in **both** formatters — only
`export /* c */ default` relocates. Preserving makes the whole `export`→X family read one way
instead of splitting it on the follower. A keyword's words are not a *pure separator* (the one
sanctioned reason to trail), so the position is one an author can mean.

The value-side gap is a separate fixture:
[default_value_same_line_comment](../default_value_same_line_comment_prettier_divergence/)
(which carries every authoring of that gap as variants). The line
form is [default_keyword_line_comment](../default_keyword_line_comment_prettier_divergence/); the
decorated-class path is
[default_keyword_decorator_comment](../default_keyword_decorator_comment_prettier_divergence/).

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
