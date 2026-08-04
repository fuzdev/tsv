# array_bracket_comment_prettier_divergence

A comment inside an array type's `[]` suffix (`string[/* c */]`) stays inside
the brackets. Prettier hoists it out in front of them, re-binding it from the
suffix to the element type.

tsv: keeps the comment where the author wrote it
Prettier: moves it before `[`, or (a line comment) out of the statement entirely

```
// tsv                          // prettier
type A = string[/* c */];       type A = string /* c */[];

type B = string[                type B = string[];
	// c                          // c
];
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). The brackets are the array suffix; a comment the author parked
inside them is about the suffix, not about the element type, so hoisting it in
front of `[` is a syntactic-position move. The empty **tuple** type is the same
shape one level over — `type A = [/* c */]` keeps its comment inside the
brackets, and a `//` there breaks them open — and both formatters already agree
there, so preserving here makes the two bracket forms answer one question one
way.

For a line comment the move is worse than a re-binding: prettier strands it on
its own line *after* the declaration (`type B = string[];⏎// c`), where it now
reads as leading whatever statement follows.

A comment written *before* the brackets stays before them in both formatters
(`type C`), so only the interior position diverges. A chain of suffixes keeps
each comment with its own pair (`type D`).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
