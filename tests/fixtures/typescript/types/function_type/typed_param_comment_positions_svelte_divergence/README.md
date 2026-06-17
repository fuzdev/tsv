# Parser divergence: function-type param comment duplication

A comment in a function type's parameter parentheses **before the colon** —
before the param (`(/* a */ x: T)`) or after the param name (`(x /* b */ : T)`) —
is duplicated in acorn-typescript's root `comments` array: the
`tsIsUnambiguouslyStartOfFunctionType` lookahead scans `( param :` before the
real parse, firing `onComment` twice. A typed param does not avoid this (the
lookahead runs regardless). A comment **after the colon** (`(x: T /* d */)`) is
parsed once and is the non-diverging sibling here. Our parser keeps a single
entry for every position (`expected_ours.json` vs `expected_svelte.json`);
`ast_diff` confirms semantic equivalence and formatting is unaffected.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
