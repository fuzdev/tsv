# atword_escape_prettier_divergence

An `@`-word carrying an **escape** — `@a\62 c (2.5)`, `@a\41 (2.5)`, `@a\g (2.5)`.

tsv: keeps the word's bytes and gives the group after it the ordinary member gap, exactly as
it does for a plain `@a` (`@a\62 c (2.5)`)
Prettier: deletes the gap after the escaped word, and does it **again on its own output** —
one space per pass until the run welds (`audit_signature.txt`)

## Reason

Two prettier readings compound, and both are visible in the chain:

- **The at-word's extent.** postcss-values-parser ends an at-word at whitespace with no
  regard for escapes, so `@a\62 c` is the at-word `@a\62` plus the word `c` — where CSS
  Syntax 3 §"Consume an escaped code point" makes that space the hex escape's own optional
  **terminator**, part of the escape, so the ident is `@` + `bc`. Prettier then re-emits the
  at-word as `"@" + value`, dropping the terminator.
- **The glue.** `printCommaSeparatedValueGroup`'s "Ignore escape `\`" arm prints any node
  whose value holds a backslash glued to whatever follows it. That is what closes the gap the
  extent rule just opened (`@a\62` + `c` → `@a\62c`), and what deletes the **authored** gap
  before a group (`@a\41 (2.5)` → `@a\41(2.5)`, `@a\g (2.5)` → `@a\g(2.5)` — a literal escape
  has no terminator, so only the glue is at work there).

The two together are not idempotent and are not lossless. `\62 c` spells `bc` and `\62c`
spells U+062C — a different character — and `@a\62 2c` (`b2c`) becomes `@a\622c` (U+622C). On
prettier's **second** pass the at-word's new value still holds the backslash, so the arm fires
again and eats the next gap: `@a\62c (2.5)` → `@a\62c(2.5)`, and `@a\62c d` → `@a\62cd`, which
welds two value tokens into one ident.

tsv steps every escape whole (`escapes::escape_len`) and reads the whole `@a\62 c` as one
at-word, so nothing in it moves; the group beside it is a member, as it is beside `@a`.

A `-` after the escape changes nothing about either reading — it is at-word content to both
(`@a\62 c-d (2.5)`), so the disagreement stays the terminator and the glue: prettier drops the
space inside the escape and keeps the member gap (`@a\62c-d (2.5)`), where tsv keeps both.

The controls are in the fixture: a plain `@`-word (`@a (2.5)`, `@ab (2.5)`) carries no
backslash, neither arm fires, and the two formatters agree —
[atword](../atword/) pins that agreement across the rest of the word's surface.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Escape in an `@`-word").
