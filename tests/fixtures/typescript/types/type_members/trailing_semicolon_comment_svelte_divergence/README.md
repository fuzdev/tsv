# Parser divergence: comment duplication in root comments array

Acorn-typescript duplicates a comment on a return/property type annotation that
is immediately followed by `;`, in the root `comments` array. This is a side
effect of acorn's backtrack-and-reparse when parsing the member's type. Here the
method, type-literal method, call-signature, and construct-signature comments
each duplicate; the bare property-signature comment (`a3: number /* comment */;`)
does not.

Our parser does not duplicate these comments — each comment appears exactly
once in the root comments array. The AST structure is semantically equivalent
(`ast_diff` confirms match). The difference is only in the root `comments`
array count (our: 10, canonical: 14).

This does not affect formatting — the formatter finds comments by position,
not by their count in the root array.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
