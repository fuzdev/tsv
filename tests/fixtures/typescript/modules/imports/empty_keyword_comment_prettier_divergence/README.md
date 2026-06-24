# empty_keyword_comment — prettier divergence

Prettier relocates comments between the `import` keyword and empty specifier braces
to after `from`. tsv preserves comments where the user placed them.

- `import /* c1 */ {} from './a'` → prettier: `import {} from /* c1 */ './a'`
- `import // c2\n{} from './a'` → prettier: `import {} from // c2\n'./a'`

tsv keeps each comment between `import` and the empty braces; the line comment (c2)
forces `{} from …` onto the next line, indented one level (the uniform
module-header rule). Both positions are dual-stable in our formatter. The non-type
sibling of `empty_type_keyword_comment_prettier_divergence`.

Reason: Comment position — the user's chosen placement is preserved.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
