# name_before_eq_own_line_block_comment_prettier_divergence

An own-line **block** comment in a type alias's name→`=` gap
(`type A⏎/* c */⏎= number;`). A single-line block forces nothing, and a comment
in this gap trails the name (a trailing position), so tsv collapses the
authored breaks and keeps the comment inline in its authored syntactic slot
(`type A /* c */ = number;` — the form both formatters hold stable when
authored inline). Prettier instead **relocates** the comment across the `=` and
hangs it leading the RHS (`type A =⏎\t/* c */⏎\tnumber;`).

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier takes it to the relocated hang instead (one pass — no
  intermediate).
- `variant_own_line.svelte` — prettier's landing form, dual-stable: there the
  comment sits *after* the `=`, leading the RHS — a different syntactic
  position, which both formatters preserve (the value-gap own-line rule pinned
  in [rhs_leading_comment](../rhs_leading_comment/)).

A run of blocks collapses in order, each comment kept distinct — lossless. The
type-alias face of the before-`=` family
([declarator](../../../declarations/variable/declarator_before_eq_own_line_block_comment_prettier_divergence/),
[class property](../../../declarations/class/property_before_eq_own_line_block_comment_prettier_divergence/),
[enum member](../../../declarations/enum/member_before_eq_own_line_block_comment_prettier_divergence/)),
taking the same relocate-and-hang outcome as the declarator.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
