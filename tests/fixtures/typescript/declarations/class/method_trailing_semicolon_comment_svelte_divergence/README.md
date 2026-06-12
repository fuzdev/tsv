# Parser divergence: comment duplication in root comments array

Same as `types/type_members/trailing_semicolon_comment_svelte_divergence`.

Acorn-typescript duplicates `trailingComments` on type annotations in
abstract/declare class method and property signatures. Our parser does not
duplicate — each comment appears once. ASTs are semantically equivalent.
