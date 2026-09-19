# shift_left_heritage_prettier_divergence

A heritage clause's type arguments — a class's `extends` and `implements`, an interface's
`extends` — whose first argument opens with a generic function type's `<` keep a space after
their own `<`:

```ts
// tsv                                      // prettier
class B2 extends a.b< <T>() => U, V> {}     class B2 extends a.b<<T>() => U, V> {}
class B3 implements I< <T>() => U> {}       class B3 implements I<<T>() => U> {}
```

## Why tsv differs

◆prettier_bug. The two `<` lex as one `<<` token, and tsc re-scans that token into `<` `<`
only where it parses type arguments with a rescan — a call, `new`, an instantiation, a tagged
template, a type reference. In a heritage clause it never does, so the glued spelling is
`',' expected.` to tsc and prettier's own second pass throws on its output (F4b tolerates the
missing `audit_signature.txt`). acorn-typescript — tsv's parse oracle — splits the token
in a class's `extends` alone, where tsv parses the glued form and repairs it (the template
cell of `unformatted_ours_glued`). At `implements` and an interface's `extends` acorn-typescript
rejects the glued form as tsc does, and so does tsv (`input_invalid_*`).

The separator is a **space**, not a paren shell: the tree is unambiguous and only the lexer's
longest-match is in the way, so the repair is lexical (as `a + +b` is) and owes nothing once
the list breaks — after the `<`'s line break no second `<` follows it, which is prettier's
form too. Every authoring reaches the one form: a redundant shell (`<(<T>() => R)>`) strips
as it does everywhere, a one-member union peels, a line break folds
(`unformatted_ours_*`). A comment ahead of the type already separates the two and takes no
extra space.

A CALLED superclass's list is a call's, and a nested list a type reference's — tsc splits the
`<<` at both, so they stay glued in both tools, as every position in
[shift_left_vs_type_args](../shift_left_vs_type_args/) does. The sibling no-split positions are
[shift_left_type_assertion](../shift_left_type_assertion_prettier_divergence/) and
[shift_left_typeof_query](../shift_left_typeof_query_prettier_divergence/).

## Reason

◆prettier_bug. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(`<` `<` kept apart where tsc never splits a `<<`) and
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
