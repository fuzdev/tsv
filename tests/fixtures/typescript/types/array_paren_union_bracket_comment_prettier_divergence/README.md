# array_paren_union_bracket_comment_prettier_divergence

A comment in the `[]` suffix of a parenthesized **union** array element stays where
the author wrote it. Prettier moves it into the parens, trailing the last union
member — the opposite side of the `)` from where it sends the same comment on every
other parenthesized element
([array_paren_bracket_comment](../array_paren_bracket_comment_prettier_divergence/)).

tsv: keeps the comment where the author wrote it
Prettier: moves it inside the parens, after the last member

```
// tsv                            // prettier
type A = ('a' | 'b')[/* c */];    type A = ('a' | 'b' /* c */)[];

type B = ('a' | 'b') /* c */[];   type B = ('a' | 'b' /* c */)[];
```

## Reason

Both authorings — inside the brackets and in the `)`→`[` gap — collapse onto one
prettier form, so each loses a distinction the author drew, and the landing re-binds
the comment to the union's last member. tsv keeps the bracket comment inside the
brackets (the suffix is what it was written about) and the gap comment in the gap.

Prettier's destination here is keyed on the element's own layout, not on its kind: a
union that prints **hugged** (`(T | null)[]`) takes the sibling entry's route, out in
front of the brackets, while a union that expands takes this one. tsv answers both the
same way, so the two fixtures differ only in what prettier does.

A line comment inside the brackets breaks them open (`type C`), as it does for every
other bracket pair; prettier expands the parens instead and parks it after the members.
A comment already written inside the parens stays a separate position from the suffix's
own — prettier lands both on one line (`type D`). A union whose parens already expand
for width changes nothing (`type E`).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
