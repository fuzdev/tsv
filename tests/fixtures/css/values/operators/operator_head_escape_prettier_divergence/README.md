# operator_head_escape_prettier_divergence

A head `+` before a member whose ident is spelled with an **escape** — `+ \61`, where `\61`
is css-syntax-3 §4.3.7's hex escape for U+0061 and the value's tokens are therefore exactly
those of `+ a`.

tsv: `+ \61` — an escape is ordinary word content, so the head `+` takes the member gap it
takes before any other word
Prettier: `+\61` — the head-`+` arm is gated on `isWordNode`, which its value parser does not
answer for an escape-spelled token, so the gap it keeps before `+ a` disappears before
`+ \61`

`◆design_choice`. See
[conformance_prettier.md §Reasons tsv Differs](../../../../../../docs/conformance_prettier.md#reasons-tsv-differs)
for the tag and
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Head `+` before an escape-spelled member".

## Why this is a `◆design_choice` and not a `◆prettier_bug`

The bug tag's criteria are output that is non-idempotent, invalid, or meaning-changing, and
prettier's `+\61` is none of the three: it is prettier's own fixed point (this fixture's
`output_prettier.svelte` is what a second pass returns), it re-parses, and it lexes as the
same two tokens the author wrote — `<delim +>` then `<ident a>`. The glue loses nothing, so
tsv would be free to follow it, and the only thing wrong with it is narrower than a bug:
prettier does not apply it to the same ident written plainly, so `+ a` keeps its gap while
`+ \61` does not. One value's spacing then depends on how an author spelled a character,
which is a rule tsv cannot state — and a rule it cannot state is one it declines to copy.
Both formatters converge on one form per authoring and they pick a different representative
for the same token stream, which is the narrow sense of `◆design_choice`.

The inconsistency is real, and it is the reason this fixture exists rather than an agreement
cell; it is simply not what `◆prettier_bug` names.

## Where prettier's split runs

`printCommaSeparatedValueGroup`'s sign arm drops the separator for an addition operator only
when `requireSpaceAfterOperator` is false, and that flag is `isWordNode(iNextNode) ||
iNextNode.type === "value-func" || …` — with `isWordNode` a type test (`value-word`,
`value-atword`). A word prettier's value parser hands back with a word type keeps the gap; a
token it does not is glued. Measured, the discriminator is a **decimal** digit after the `\`,
wherever in the token it sits, and not a hex one: `\0`–`\9` glue (`+ \61`, `+ \41`,
`+ \0061`, `+ \61x`, `+ \1z`, `+ a\61`, `+ \5a`, `+ \2e`, `+ a\9b`, and `+ \\61`, whose
second `\` is followed by a `6`), while every other escape keeps the gap — the non-digit ones
(`+ \?`, `+ \z`, `+ \-a`, `+ \é`, `+ a\z`) and the hex-letter ones alike (`+ \a`–`+ \f`,
`+ \A`–`+ \F`, `+ \e9`, `+ \fe`, `+ \Ff`, `+ \cc`, `+ a\A`, `+ a\Ab`). `\61` and `a` name the
same ident and take opposite answers; so do `\5a` and `\e9`, two hex escapes that differ only
in which digit class opens them.

## Why tsv keeps the gap

tsv has no escape-shaped exception to make: `escape_len` steps an escape whole and the member
around it is a word like any other, so a head `+` before it takes the ordinary member gap —
the same answer tsv gives at `+ a`, `+ @a`, `+ [a]` and `+ f(2.5)`
([operator_head_glue](../operator_head_glue/) pins those). Following prettier here would mean
keying a *spacing* rule on how an author chose to spell a character, which is the one thing
the rule cannot be about: the two spellings produce the same token stream, so no downstream
reader can tell them apart.

The bound cells are the other half of the claim. `+ \e9` is the escape that separates the two
readings of prettier's split — hex digits alone, spelling `é` — and `+ \?` and `+ a\z` are
the same authoring with an escape that is not hex at all; all three keep their gap on both
formatters, as does the plain `+ a` that `\61` spells. `a \61` has no operator at all, and
`1.5 / \61` gives the operator an operand on its left, which takes it out of the head arm —
both agree too.

## Related

- [operator_head_glue](../operator_head_glue/) — the head rule where the two formatters agree,
  `+ a` included
- [operator_head_weld_prettier_divergence](../operator_head_weld_prettier_divergence/) — the
  head rule where prettier's glue **merges** two tokens into one and tsv refuses; a head `-`
  before an escape lands there (`- \61` → `-\61` is the single ident `-a`), because for `-`
  the merge, not the word test, is what the rule turns on
- [atword_escape_prettier_divergence](../atword_escape_prettier_divergence/) — the other place
  an escape's spelling changes what prettier's value parser reads
