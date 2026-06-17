# Parser divergence: arrow return-type comment duplication (untyped params)

A comment between an arrow function's return type and its `=>`
(`(): T /* c */ => 0`, `(x): T /* c */ => 0`) is duplicated in acorn-typescript's
root `comments` array — acorn parses the parens as an expression and backtracks
on `=>`, re-parsing the return-type region. This happens whether the params are
empty, untyped, or typed. A comment **inside** the return-type annotation
(`(): /* d */ T => 0`) is parsed once and is the non-diverging sibling here. Our
parser keeps a single entry for every position (`expected_ours.json` vs
`expected_svelte.json`); `ast_diff` confirms semantic equivalence and formatting
is unaffected.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
