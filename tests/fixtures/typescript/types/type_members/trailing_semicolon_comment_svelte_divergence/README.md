# Parser divergence: comment duplication in root comments array

Acorn-typescript duplicates comments that appear as `trailingComments` on
type annotations in property signatures, method signatures, call signatures,
and construct signatures. This is a side effect of acorn's backtrack-and-reparse
behavior when parsing these constructs.

Our parser does not duplicate these comments — each comment appears exactly
once in the root comments array. The AST structure is semantically equivalent
(`ast_diff` confirms match). The difference is only in the root `comments`
array count (our: 10, canonical: 14).

This does not affect formatting — the formatter finds comments by position,
not by their count in the root array.
