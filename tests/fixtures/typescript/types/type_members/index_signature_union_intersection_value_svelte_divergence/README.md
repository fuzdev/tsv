# Parser divergence: comment duplication in a type literal's `{ … }` body

This fixture covers how an index signature's union/intersection value formats
(short stays inline; long intersection hugs the colon and wraps with trailing
`&`; long union breaks after the colon to leading-pipe form). It also carries a
`_svelte_divergence`: the first label comment (`// short intersection …`) sits
between the enclosing type literal's `{` and its first member, a region
acorn-typescript re-parses — its backtrack-and-reparse fires the `onComment`
callback twice, so that comment is duplicated in the root `comments` array. Our
parser keeps a single entry (`expected_ours.json` vs `expected_svelte.json`);
the set of distinct comments is identical — only multiplicity differs — and
`ast_diff` confirms semantic equivalence.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
