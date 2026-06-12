# empty_keyword_comment — prettier divergence

Prettier relocates comments between the `export` keyword and empty specifier braces
to after `from` in re-exports. tsv preserves comments where the user placed them.

- `export /* c */ {} from './a'` → prettier: `export {} from /* c */ './a'`
- `export // c\n{} from './a'` → prettier: `export {} from // c\n'./a'`

Same pattern as `imports/empty_keyword_comment_prettier_divergence`.

Reason: Comment preservation (see conformance_prettier.md §Comment Position Philosophy).
