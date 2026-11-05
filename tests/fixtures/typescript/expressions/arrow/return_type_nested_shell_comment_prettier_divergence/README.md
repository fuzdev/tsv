# Arrow return type, comment in a shell down the outer shell's right edge

A comment written last inside a redundant pair down the right edge of an arrow's
parenthesized return type — a union's or intersection's last member, a `keyof` operand, a
conditional's false branch, a union member that is itself a `keyof` or `readonly` type, a
constructor type's return type, a doubled pair (`(a): (B | (C /* x⏎ */)) => 1`) — stays
inside the author's OUTER pair, which opens around it, the form the single-pair spelling
`(a): (B | C /* x⏎ */) => 1` takes (`unformatted_ours_shell` → `input`). A `//` after the
`:` hangs the kept pair below it, as it hangs the single pair; a `//` glued to a stripped
inner `(` keeps the outer `(` line, as it keeps the single pair's; and a pair the printer
adds down the edge (`B | (C & D)`) holds the comment after it. A format-ignore directive
written in a member's stripped pair leads the member, which it freezes, and the kept pair
still holds the comment after it (`x16`); a directive written ahead of the member's own
pair freezes that pair as written, comments and all, and the outer pair strips (`x17`).
A doubled pair around a generic reference (`((G<T> // x⏎))`) keeps the outer pair too, since `(G<T>)` cannot read
as a parameter list; a type that can takes a second pair inside the outer one, in
[return_type_param_shaped_shell_comment](../return_type_param_shaped_shell_comment_prettier_divergence/).

tsv: `): (⏎\tB | C /* x⏎\t */⏎) => 1`, the comment inside the kept pair
Prettier: `): B | C /* x⏎ */ => 1`, the pair stripped and the block's line break left
ahead of the `=>`, which is `[no LineTerminator here]` (ECMA-262 `ArrowFunction`): acorn
and tsv reject that form (`output_prettier`), and tsc reports it (TS1200); prettier's own
TypeScript parser tolerates it and reaches a second-pass fixed point (`audit_signature`,
`audit_signature_shell`). A `//` moves past the statement's `;` (`B | C => 1; // x`)

## Reason

The block's line break is legal only inside a pair, so a pair has to survive with the
comment in it, and the outer pair is the one the author wrote around the whole type. The
inner pairs carry no authoring signal of their own: the inner and outer spellings are the
same program, so both converge on one form. That holds for an inner pair that could stay
too (`(B | (C[] // x⏎))`, the `x7` case), which tsv keeps the outer pair for in the same
way; a `//` takes the same form as a block, the comment kind the single-pair spelling
already shares.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Arrow return type, comment in a shell down the type's right edge).
