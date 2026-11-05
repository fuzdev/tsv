# Arrow return type a lone pair would read as parameters, comment kept in two pairs

The companion of
[return_type_nested_shell_comment](../return_type_nested_shell_comment_prettier_divergence/)
for a return type one pair cannot hold before `=>`: a name or a keyword type spelled as one
(`string`), `this`, a type led by `{` or `[`, a `readonly` tuple — and `void` / `null` /
`true` too, whose lone pair tsv's own parser rejects (a known gap). `(a): (C /* x⏎ */) => 1`
reads `(C /* x⏎ */)` as a function type's parameter list, so a single pair around the type
would change the program. A comment written last inside the author's pairs
(`(a): ((C /* x⏎ */)) => 1`) stays in place inside two: the type in a pair of its own,
which nothing reads as parameters once `=>` does not follow it, and the outer pair around
it, which holds the comment's line break ahead of the `=>` (`unformatted_ours_shell` →
`input`).

The inner pair is the one the type needs, so it hugs the type and holds no comment: every
comment of the shells sits in the outer pair around it, in source order, the way a pair
the printer adds down a union's edge holds none in the companion. A comment the author
wrote after an inner `)` keeps trailing the tight one, and an inner pair the author opened
around its comment takes the tight pair too. A comment inside the type stays inside it,
and a `//` after the `:` hangs the pairs below it, as it hangs the companion's single
pair. A comment that spans no line holds no line break ahead of the `=>`, so both pairs
strip, as on any other type (the `c14` and `l15` cells), and a format-ignore directive
after the `:` freezes the pairs as the author wrote them (`x19`).

tsv: `): (⏎\t(C) /* x⏎\t */⏎) => 1`, `): (⏎\t({ a: C /* i */ }) // x⏎) => 1`
Prettier: strips both pairs and leaves the block after the type, its line break ahead of
the `=>`, which is `[no LineTerminator here]` (ECMA-262 `ArrowFunction`): acorn and tsv
reject that form (`output_prettier`), and tsc reports it (TS1200); prettier's own
TypeScript parser tolerates it and reaches a second-pass fixed point (`audit_signature`).
A `//` moves past the statement's `;` (`C => 1; // x`). Under a `//` after the `:`
prettier sets the type flush with the `:` line where tsv indents it one level, the
`:`→type divergence the bare spelling already takes.

## Reason

The comment's line break is legal only inside a pair, and the one pair that holds it
cannot sit directly around a type that opens like a parameter, so the type takes a pair
of its own inside it. The comments stay where the author wrote them, inside the pair
that holds them and ahead of the `=>`; only the pairs the program does not need strip.
The shells carry no authoring signal of their own (both spellings are the same program),
so both converge on that form.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Arrow return type, comment in a shell down the type's right edge; Annotation `:`→type
own-line comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
