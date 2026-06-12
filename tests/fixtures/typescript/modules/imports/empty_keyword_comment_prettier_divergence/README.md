# empty_keyword_comment — prettier divergence

Prettier relocates comments between the `import` keyword and empty specifier braces
to after `from`. tsv preserves comments where the user placed them.

- `import /* c */ {} from './a'` → prettier: `import {} from /* c */ './a'`
- `import // c\n{} from './a'` → prettier: `import {} from // c\n'./a'`

Reason: Comment preservation (see conformance_prettier.md §Comment Position Philosophy).
