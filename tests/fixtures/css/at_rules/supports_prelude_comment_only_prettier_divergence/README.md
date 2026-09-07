# supports_prelude_comment_only_prettier_divergence

A `@supports` prelude that holds **only comments** — no condition part at all. The
comments are still content, and the prelude span is their only carrier, so tsv emits
them: `@supports /* c */;` is its own fixed point.

Prettier drops the last byte of the closing delimiter, emitting `@supports  /* c *;` —
an **unterminated** comment. Everything after it is swallowed, and prettier's own parser
then rejects the file (`CssSyntaxError: Unclosed comment`), so no prettier fixed point
exists for this input and no prettier-anchored claim is expressible
(`prettier_nonconvergent.txt`).

The `@container` spelling of the same prelude is a **match** — prettier gets that one
right — and is pinned by
[container_prelude_comment_only](../container_prelude_comment_only/).

## Reason

Content preservation, and a prettier bug. Output that does not re-parse is never the
defensible side. See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(`@supports comment-only prelude`).
