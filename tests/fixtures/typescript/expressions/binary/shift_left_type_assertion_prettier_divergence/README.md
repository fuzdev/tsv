# shift_left_type_assertion_prettier_divergence

An angle-bracket type assertion whose asserted type opens with a generic function type's `<`
keeps a space after its own `<`:

```ts
// tsv                          // prettier
const a1 = < <T>() => R>x;      const a1 = <<T>() => R>x;
```

## Why tsv differs

◆prettier_bug. The two `<` lex as one `<<` token, and tsc re-scans that token into `<` `<`
only where it parses type arguments with a rescan — a call, `new`, an instantiation, a tagged
template, a type reference. At an assertion it never does, so the glued spelling is
`Expression expected.` to tsc and prettier's own second pass throws on its output (F4b tolerates the
missing `audit_signature.txt`). acorn-typescript — tsv's parse oracle — splits the token
here, so tsv parses the glued form and repairs it (the template cell of
`unformatted_ours_glued` — a `<script>` body prettier reads with tsc's parser and throws on).

The separator is a **space**, not a paren shell: the tree is unambiguous and only the lexer's
longest-match is in the way, so the repair is lexical (as `a + +b` is) and owes nothing once
the list breaks — after the `<`'s line break no second `<` follows it, which is prettier's
form too. Every authoring reaches the one form: a redundant shell (`<(<T>() => R)>`) strips
as it does everywhere, a one-member union peels, a line break folds
(`unformatted_ours_*`). A comment ahead of the type already separates the two and takes no
extra space.

The positions where tsc does split stay glued in both tools
([shift_left_vs_type_args](../shift_left_vs_type_args/)); the sibling no-split positions are
[shift_left_typeof_query](../shift_left_typeof_query_prettier_divergence/) and
[shift_left_heritage](../shift_left_heritage_prettier_divergence/), and the width boundary is
[shift_left_no_split_long](../shift_left_no_split_long_prettier_divergence/).

## Reason

◆prettier_bug. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(`<` `<` kept apart where tsc never splits a `<<`) and
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
