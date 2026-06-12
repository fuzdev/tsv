# path_calls_long_prettier_divergence

Prettier has special handling for module path calls that differs from tsv.

**Plain `require()`:**
tsv: wraps arguments at print_width (101+ chars)
Prettier: never wraps plain `require(string)` calls

**`require.resolve.paths()` and `import.meta.resolve()`:**
tsv: expands call arguments to multiple lines
Prettier: breaks at the member chain (`.paths` / `.resolve`)

Matching patterns (both formatters agree): `require.resolve(string)` and `await import(string)` break at assignment.

## Reason

tsv wraps consistently — lines should respect print_width, and function calls should wrap the same way regardless of the callee. Consistent with tsv's handling of single-specifier imports.
