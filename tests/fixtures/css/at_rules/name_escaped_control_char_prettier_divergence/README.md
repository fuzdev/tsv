# name_escaped_control_char_prettier_divergence

An escape *inside the at-rule name token* (`@m\A x`), whose escaped code point is a
control character (`\A` = U+000A, a newline).

`@m\A x` is a single `<at-keyword-token>`: css-syntax-3 §4.3 consumes the whole run —
`m`, the escape `\A ` (the space is the escape's terminator), then `x` — as one name.
Svelte's `parseCss` decodes it to the name `"m\nx"` with an **empty prelude**, and tsv
matches that AST (verified: tsv and parseCss both report `name: "m\nx"`).

tsv emits the name from **source**, so the escape is preserved and `input.svelte`
formats to itself. Emitting the *decoded* name (the prior bug) put the raw newline into
the output:

| pass | prior (buggy) output | problem |
| --- | --- | --- |
| `format(input)` | `@m⏎x {` | a raw newline is injected into the name — corrupted output |
| `format(format(input))` | `@m x {` | the escape is dropped on reparse (name `m\nx` → `m`) — content loss + non-idempotent |

**Prettier splits the name at the escape** into a name plus a prelude, so it prints
`@m \A x {` (name `m`, prelude `\A x`). Its postcss parser also rejects a name that
*starts* with an escape outright (`@\6D edia` → `At-rule without name`). tsv can't adopt
that split without contradicting its own parse (parseCss's whole-run name), so the two
diverge — but both outputs re-parse to their own formatter's AST. This is the same
escape-preservation tsv (and prettier) already apply to escaped **property** names
(`css/declarations/escaped_property_name`, no divergence); the at-rule name was the one
decoded-name site that emitted the decoded form.

The end-of-name subcase (`@n\A {`) is the at-rule-name face of the separator rule the
prelude form already catalogs: the escape needs a terminator space, and that space also
serves as the separator before `{`, so tsv adds no second one (`@n\A {`, not
`@n\A  {`) — the same absorption as
[layer_escaped_whitespace](../layer_escaped_whitespace_prettier_divergence/).

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules).
