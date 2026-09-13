# double_dash_word_extent_prettier_divergence

A `--` word with **nothing after its two dashes** followed by a `/` or by a `-` that signs
a number — `--/2.50`, `---2.50` — and the same two runs behind the `+` a value's head
welds onto them (`+--/2.50`, `+---2.50`).

tsv: one word, the same reading it gives every other word — a `/` and a `-` are word
content, so nothing inside it normalizes (`--/2.50`)
Prettier: ends the word at the `/` and at the signing `-`, so the run is three members and
the number normalizes (`-- / 2.5`, `---2.5`)

`◆design_choice`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Bare `--` word extent".

## Where the two readings part

tsv's word extent is **uniform**: `/` and `-` are content in every word, which is postcss's
own rule at every spelling but this one and the reason `a/1.50` is a single token whose
`1.50` neither formatter touches. postcss-values-parser's extent is uniform too *until* the
word is the bare `--`, and there its scan stops at the next `-` or `/`:

| authoring | tsv | prettier | postcss's members |
| --- | --- | --- | --- |
| `--/2.50` | `--/2.50` | `-- / 2.5` | `word --`, `operator /`, `number 2.50` |
| `---2.50` | `---2.50` | `---2.5` | `word --`, `operator -`, `number 2.50` |
| `+--/2.50` | `+--/2.50` | `+-- / 2.5` | the same, behind the welded sign |
| `+---2.50` | `+---2.50` | `+---2.5` | the same |
| `--a/1.50` | `--a/1.50` | `--a/1.50` | one `word` — a single ident code point after the dashes and the `/` is content again |
| `--a-2.50` | `--a-2.50` | `--a-2.50` | one `word` |
| `+--a/1.50` | `+--a/1.50` | `+--a/1.50` | one `word`, sign included |
| `+--a-2.50` | `+--a-2.50` | `+--a-2.50` | one `word` |

The last four are the agreement cells, and they are what makes the exception an exception:
the `/` that ends the word in row 1 is content in row 5, and the byte that decides it is the
one **inside** the token, two positions back.

## Why tsv keeps one reading

The two consequences of postcss's stop are a normalization (`2.50` → `2.5`) and a gap, and
taking them would mean a word whose extent depends on what its own first two bytes spelled.
tsv reads `--` as [`is_content_pair`](../../../../../../crates/tsv_css/src/parser/value/operators.rs)'s
custom-property-shaped ident wherever it stands — which is the reading that keeps
`var(---a)` and `---a-1` whole, keeps the welded `+--a` of
[unary_plus](../unary_plus/) the same token as `+a`, and leaves one rule to state.

⚠️ **The convergence count runs the other way here**, which is why this is not a
`◆stable_quirk`: prettier folds `--/2.50`, `-- /2.50`, `--/ 2.50` and `-- / 2.50` onto the
single `-- / 2.5`, where tsv holds three forms (`--/2.50`, `--/ 2.5`, `-- / 2.5`) — the
authored gap survives because the glued spelling is one token and the spaced ones are not.
tsv normalizes fewer authorings of a run no corpus contains, and in exchange every word has
one extent.

## Related

- [unary_plus](../unary_plus/) — the `+` these cells carry: it welds onto a `--` word
  exactly as onto a letter, and the two `+` rows here diverge only in the extent, not in
  the weld
- [trailing_operator](../trailing_operator_prettier_divergence/) — the other place a
  glued `/` and a trailing `-` decide a run's members
