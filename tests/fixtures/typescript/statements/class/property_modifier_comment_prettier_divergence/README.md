# property_modifier_comment_prettier_divergence

Both formatters preserve comment position relative to `?`/`!` modifiers.
Both positions are dual-stable in both formatters.

- Before modifier: `a /* c */? = 1;` (prettier's canonical form)
- After modifier: `a? /* c */ = 1;` (our canonical form)

Per comment placement policy, we preserve the user's chosen position — the
comment stays where the user placed it.

Note: when a type annotation is present (e.g., `a /* c */?: number = 1;`),
this becomes a SAFETY bug (comment dropped) — see `property_modifier_type_comment`.
