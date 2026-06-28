# branch_comment_long_prettier_divergence

Block and line comments after the `?`/`:` operators of a long conditional type,
across the branch shapes that break (intersection continuations, union
leading-pipe members) and stay inline.

## Divergence — `LongBlockFalseU`

The single divergence is the false branch whose value is a **breaking union**
preceded by a block comment (`: /* c */ A | B | C`, where the union is too wide
and explodes to leading-pipe members):

```
// tsv                                  // prettier 3.9
: /* c */ Aaaa…                         : | /* c */ Aaaa…
  | Bbbb…                                 | Bbbb…
  | Cccc…                                 | Cccc…
```

tsv keeps the comment where the author wrote it — **before** the union, on the
`:` line, ahead of the first member's `|`. Prettier 3.9 relocates it **across
the `|` separator, into the first union member** (`| /* c */ Aaaa`), changing the
comment's association from the union as a whole to its first member. Per the
[Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
tsv treats the authored position as intentional and does not move the comment
across the member boundary.

Prettier here is authoring-dependent: the same logical comment re-attaches
differently depending on surrounding whitespace (inline vs own-line, before vs
inside the member), so `unformatted_ours_compact.svelte` normalizes to `input`
under tsv but not under prettier (prettier settles on `output_prettier.svelte`'s
relocated form).

The other five cases match prettier: a block comment on a short/long intersection
true branch stays glued (`? /* c */ A & B`), and line comments after `?`/`:` drop
the branch to its own indented line in both formatters.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
