# key_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in the key→`:` annotation gap of a property
signature, type-literal member, or class property (`a⏎/* c */⏎: number`). A
single-line block forces nothing, and a comment in this gap trails the key (a
trailing position), so tsv collapses the authored breaks and keeps the comment
inline in its authored syntactic slot — `a /* c1 */: number`, the class
property keeping the spaced inline form `c /* c3 */ : number` both formatters
use for that site — reaching in **one pass** the fixed point prettier itself
converges to **non-idempotently over three passes** (pass 1 relocates the
comment across the `:` — `a: /* c1 */⏎number` — pass 2 pulls it back inline
glued, pass 3 settles the class property's spacing).

The end states agree, so this is a pass-count divergence, not an end-state one:
`unformatted_ours_own_line.svelte` normalizes to input under tsv only (the
one-pass claim; prettier's first pass lands elsewhere), and no
`output_prettier.svelte` exists because input is prettier-stable. The chain is
too long for a `prettier_intermediate_*` pin (two distinct intermediates), so
the README records it.

The same-gap **line** comment (which forces the break → continuation indent) is
[key_colon_line_comment](../key_colon_line_comment_prettier_divergence/); the
inline-authored block is a plain match in both formatters, documented there.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
