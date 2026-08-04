# Divergence: binding name→annotation-`:` multiline block, authored break kept

A **multiline** block comment between a variable binding and its `:` type
annotation that the author **broke after** (`let x /* a⏎b */⏎: string;`). The
break after a multiline block is authoring signal — the same rule the value gap
applies — so tsv keeps it: the comment trails the name and `: type` drops to a
continuation line **indented one level** (the uniform forced-continuation
indent, the same landing as the line-comment sibling
[binding_key_colon_line_comment](../binding_key_colon_line_comment_prettier_divergence/)).
Prettier also keeps the comment after the name and the annotation on the next
line, but leaves the continuation **flush** (`output_prettier.svelte`) — the
same indent-only divergence as the line-comment sibling.

```ts
// tsv (continuation indent)              // prettier (flush continuation)
let x /* a                                let x /* a
b */                                      b */
	: string;                             : string;
```

A multiline block whose `:` shares its closing line (`let y /* c⏎d */ : number;`)
stays glued — the not-broke-after form, kept by both formatters (the second
case). Only the authored break distinguishes the two, exactly as at the value
gap. A single-line block's breaks stay unforced and collapse either way (the
own-line-block sibling
[binding_key_colon_own_line_block_comment](../binding_key_colon_own_line_block_comment_prettier_divergence/)).

The binding face of the rule; property signatures, class properties, function
parameters, and index-signature keys share the emitter
(`build_marker_colon_line_continuation`). See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
