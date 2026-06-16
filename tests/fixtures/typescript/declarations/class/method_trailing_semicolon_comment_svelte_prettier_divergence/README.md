# method_trailing_semicolon_comment_svelte_prettier_divergence

Both a parser divergence (Svelte) and a formatter divergence (Prettier) on the
same four class-member signatures with a trailing `/* comment */` before the `;`.

## Parser divergence (Svelte)

Same as `types/type_members/trailing_semicolon_comment_svelte_divergence`.

Acorn-typescript duplicates `trailingComments` on type annotations in
abstract/declare class method and property signatures. Our parser does not
duplicate — each comment appears once. ASTs are semantically equivalent.

## Formatter divergence (Prettier)

Prettier relocates the comment **past the semicolon** for the **abstract method**
signature only — `abstract a3(): number /* comment */;` → `abstract a3(): number;
/* comment */`. The other three forms (declare method `a1`, declare property `a2`,
abstract property `a4`) keep the comment before the `;` in both formatters, so
Prettier's relocation is specific to abstract method signatures (inconsistent — a
Prettier quirk). tsv preserves the user's placement before the `;` in all four,
per the [Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

`unformatted_ours_compact.svelte` normalizes to `input.svelte` under tsv;
Prettier normalizes it to `output_prettier.svelte` (the a3-relocated form).
