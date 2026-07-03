# nth_leading_plus_prettier_divergence

A leading `+` sign on an `An+B` term inside functional pseudo-class args. Svelte's
`parseCss` reads the whole term as one `Nth` (`+2n + 1`, `+3`, `+n`), so tsv keeps the
`+` glued to its coefficient and only normalizes the interior operator spacing.
Prettier's selector parser reads the leading `+` as a combinator and spaces it out.

tsv: `:foo(+2n + 1)`, `:is(+3)`, `:not(+n)`, `:nth-child(+2n + 1)` (leading sign glued)
Prettier: `:foo(+ 2n + 1)`, `:is(+ 3)`, `:not(+ n)`, `:nth-child(+ 2n + 1)` (leading `+` spaced)

The `:nth-child(+2n + 1)` case exercises the dedicated `:nth-child` An+B reader
(`parse_nth_args`), distinct from the bare-`An+B` reader (`match_nth_value`) the
`:foo`/`:is`/`:not` args use — the same leading-`+` divergence holds through both paths.

## Reason

Stable quirk. Per [Selectors 4 §the-nth-child-pseudo](https://drafts.csswg.org/selectors/#the-nth-child-pseudo)
the argument is a single `<an-plus-b>` term, whose optional leading sign binds to the
coefficient — it is not a combinator. tsv follows Svelte's `Nth` production (the `+` is
part of the value) and normalizes only the `A±B` interior spacing, uniform with the
dedicated `:nth-child()` path. A leading `-` does not diverge (both formatters keep
`-n + 3` glued, since `-` is unambiguously the sign of a negative coefficient); only the
`+` sign is read differently by prettier. See
[conformance_prettier.md §CSS: Selectors](../../../../../../docs/conformance_prettier.md#css-selectors).
