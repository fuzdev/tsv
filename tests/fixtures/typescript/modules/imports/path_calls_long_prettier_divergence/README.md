# path_calls_long_prettier_divergence

Prettier has special handling for module path calls that differs from tsv.

**Plain `require()`:**
tsv: wraps arguments at printWidth (101+ chars)
Prettier: never wraps plain `require(string)` calls

**`require.resolve.paths()` and `import.meta.resolve()`:**
tsv: expands call arguments to multiple lines
Prettier: breaks at the member chain (`.paths` / `.resolve`)

Matching patterns (both formatters agree): `require.resolve(string)` and `await import(string)` break at assignment.

## Reason

Print width. tsv wraps consistently — lines should respect printWidth, and function calls should wrap the same way regardless of the callee. Consistent with tsv's handling of single-specifier imports.

See [conformance_prettier.md §TypeScript](../../../../../../docs/conformance_prettier.md#typescript) (Module path calls) and [§Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy).
