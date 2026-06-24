# optional_marker_comment_svelte_prettier_divergence

Prettier relocates a block comment written after the optional `?` marker in
type-literal members; tsv preserves the user's placement (after `?`).

**Property** (`a? /* c1 */ : number;`):

- Prettier: `a /* c1 */?: number;` (moves before `?`)
- Ours: `a? /* c1 */ : number;` (preserves after `?`)

**Method** (`m? /* c2 */(x: number): void;`):

- Prettier: `m?(/* c2 */ x: number): void;` (moves inside the parens)
- Ours: `m? /* c2 */(x: number): void;` (preserves between `?` and `(`)

Both positions are dual-stable in our formatter. Per the comment-position
policy, we preserve the user's original comment position. This matches the
interface arm (`types/type_members/modifier_after_comment_prettier_divergence`) —
type-literal members now split around `?` the same way.

A comment written *before* `?` (`a /* c1 */?: number`) is a match in both
formatters — see `types/type_literal/optional_marker_before_comment_svelte_divergence`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.

## Parser — svelte divergence

The same in-construct comment is also duplicated in acorn-typescript's root
`comments` array (its backtrack-and-reparse fires `onComment` twice); our parser
keeps a single entry (`expected_ours.json` vs `expected_svelte.json`). The set of
distinct comments is identical — only multiplicity differs — and `ast_diff`
confirms semantic equivalence. See
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
