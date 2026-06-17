# Divergence: value-`:`→type line comment under a `]`→`:` comment

A line comment in an index signature's value-`:`→type gap **when a `]`→value-`:`
comment is also present** (`[k: T] /* x */: // c⏎V`). The value type is built by
the shared `build_type_annotation_doc`, so this is the same **uniform
forced-continuation indent** as every other `: Type` context: the `// c` stays
after the value `:` and the type drops to a continuation line **indented one
level**; prettier keeps the type flush.

```ts
// tsv (continuation indents one level)   // prettier
[k: string] /* x */ : // c                [k: string] /* x */ : // c
	number;                               number;
```

The `]`→`:` block comment (`/* x */`) stays after `]` in both formatters (prettier
relocates a *line* comment there into the brackets — covered separately by
[index_signature_bracket_colon_line_comment](../index_signature_bracket_colon_line_comment_prettier_divergence/)).

Previously the `]`→`:`-comment branch built the value type manually and **swallowed**
this `// c` (rendering `// c number;` on one line — content loss, non-idempotent);
delegating the value type to `build_type_annotation_doc` (what the no-`]`→`:`-comment
path already did) fixes the swallow and gives the value type the proper union/intersection
break layout for free. The all-match (no `:`→type comment) companion — simple and
long-union values under a `]`→`:` comment — is the regular fixture
[index_signature_bracket_colon_union_value](../index_signature_bracket_colon_union_value/).
See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform forced-continuation indent and §Comment Position Philosophy.
