# intersection_redundant_paren_member_line_comment_prettier_divergence

A leading **line** comment inside a **redundant** parenthesized member of an
**intersection** — one whose parens the comment-free rule strips, so the comment cannot
stay "inside" them the way it does for a member whose pair survives (see
[intersection_retained_paren_first_member_leading_line_comment](../intersection_retained_paren_first_member_leading_line_comment_prettier_divergence/)
and
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/)).
This is the intersection counterpart of the union family's
[union_redundant_paren_member_line_comment](../union_redundant_paren_member_line_comment_prettier_divergence/),
and it makes the same claim: **the comment never changes which parens are retained, only
where it renders once they are.**

**tsv** keeps the comment with the member it leads — on its own line under the `&` where
the intersection has one, and on the line the position already gives it where the
intersection collapses to its member:

```ts
type Mid = a &
	// c1
	b &
	d;
```

**Prettier** lifts it onto the `&`'s line at `Mid` / `Nested` and drops it to its own line
everywhere else — `divergent_variant_redundant_paren.svelte`, which prettier holds stable
and tsv rewrites (a third form), so it is a `divergent_variant_*` rather than a dual-stable
`variant_*`. `unformatted_ours_redundant_paren.svelte` is the authored input, with every
paren present; tsv normalizes it to `input.svelte`.

## Which pair is redundant: the member-parens rule, not the shape

The shell holding the run is a parenthesized **union**, and a paren-union member of an
intersection is normally *retained* — the union's own required pair, which is exactly what
lets the retained-paren fixtures above keep the comment inside it. What decides is the
intersection's own member-parens rule (`union_member_parens`), and two shapes fail it:

- **`Mid` / `Nested`** — a **single-member** union (the leading-`|` spelling, `(// c⏎| b)`).
  It is semantically just its member, prettier collapses it, and so does tsv — so the pair
  is redundant and does not survive. `Nested` is the same shell one redundant layer further
  out (`((// c⏎| q))`).
- **`Single*`** — a **one-member intersection** (`& (// c⏎| a | b)`), where even a *real*
  two-member union's pair is redundant: the lone member prints in the intersection's own
  position, so any parens it needs come from the intersection's parent, one level up.

Asked as "is this member a parenthesized union?" instead, both **retained a paren because
of the comment**, which the reparse then strips: pass 1 printed `a &⏎( // c1⏎b⏎)` and pass 2
`a &⏎// c1⏎b`. An **F1** violation, not a divergence.

The `Single*` cases sweep the positions, because where the run lands is the *position's*
answer once the intersection collapses: the alias `=` (`Single`), an object type's value
(`SingleValue`), a type argument (`SingleTypeArg`), a parameter annotation (`SingleParam`),
a tuple element (`SingleElement`), and an array element (`SingleArray`), whose required
pair opens over the run instead. Each keeps the line the author gave the comment — the
opening-delimiter and trailing-`=` rules — where prettier drops every one of them onto a
line of its own.

The intersection's **first** member in a 2+-member intersection is rescued from the same
premise by a different route (an enclosing seam claims its run — `TransparentShell` in
[intersection_retained_paren_first_member_leading_line_comment](../intersection_retained_paren_first_member_leading_line_comment_prettier_divergence/)),
which is why only the later members and the one-member intersection reach it here.

## Reason

Per
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv associates the comment with the member it documents rather than lifting it onto the
operator or delimiter that precedes it — the same answer the structurally identical
bare-paren authoring (`a & (// c⏎b)`) already gives, and the same answer the union family
gives in
[union_redundant_paren_member_line_comment](../union_redundant_paren_member_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
