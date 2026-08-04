# array_paren_bracket_comment_prettier_divergence

A comment inside the `[]` suffix of a **parenthesized** array element
(`(X & Y)[/* c */]`) stays inside the brackets. Prettier hoists it out in front of
them, re-binding it from the suffix to the element type — the parenthesized face of
[array_bracket_comment](../array_bracket_comment_prettier_divergence/).

tsv: keeps the comment where the author wrote it
Prettier: moves it before `[`, or (a line comment) out of the statement entirely

```
// tsv                          // prettier
type A = (X & Y)[/* c */];      type A = (X & Y) /* c */[];

type B = (X & Y)[               type B = (X & Y)[];
	// c                          // c
];
```

## Reason

The parens change what precedes the suffix, not what the suffix is: the brackets are
still the array suffix, and a comment the author parked inside them is about the
suffix, not about the element type. Both bracket pairs therefore answer one question
one way — the pair routes through the same empty-brackets emitter the empty **tuple**
type uses, so `[/* c */]` stays inline and a `//` breaks it open (see the reasoning in
[array_bracket_comment](../array_bracket_comment_prettier_divergence/)).

Only the bracket interior diverges. The `)`→`[` gap is a match in both formatters and
lives in its own fixtures —
[array_paren_before_bracket_comment](../array_paren_before_bracket_comment/) and, for
the 100/101 layout boundary,
[array_paren_before_bracket_comment_long](../array_paren_before_bracket_comment_long/).
A chain of suffixes keeps each comment with its own pair (`type D`), and a comment in
the gap stays distinct from one in the brackets — prettier collapses the two positions
onto one (`type E`).

A **hugged** union element (`type C`) takes this route too. The one place prettier
answers the same brackets differently is an element whose union *expands*, where it
moves the comment into the parens instead —
[array_paren_union_bracket_comment](../array_paren_union_bracket_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
