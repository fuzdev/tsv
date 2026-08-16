# Divergence: a pattern's bracket→`:` line comment indents the continuation

A line comment between a destructuring pattern's closing `}`/`]` and its `:` annotation.
tsv keeps the comment after the bracket and drops the `: type` to a continuation line
**indented one level** (uniform forced-continuation indent). Prettier relocates it *into
the brackets*, trailing the last element.

```ts
// tsv (comment keeps its gap)   // prettier (into the brackets)
const {                         const {
	a                             a // c1
} // c1                         }: T = x;
	: T = x;

const [b] // c2                 const [
	: U = y;                      b // c2
                                ]: U = y;
```

Relocating re-associates the comment with the last element — `// c1` reads as a comment
about `a` rather than about the binding — so tsv preserves the authored position. An
object pattern's braces still expand under the forced break (both formatters expand
them); an array pattern's brackets do not, since only prettier's relocated comment
forces them open.

The pattern spelling of the cross-construct
[before-`:` key/binding gap](../../../declarations/variable/binding_key_colon_line_comment_prettier_divergence/)
(index signatures, property signatures, class properties, variable bindings, function
parameters, destructuring renames, named tuple members). The block-comment sibling in
the same gap is a match — [pattern_bracket_colon_comment](../pattern_bracket_colon_comment/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent and §Comment Position Philosophy.
