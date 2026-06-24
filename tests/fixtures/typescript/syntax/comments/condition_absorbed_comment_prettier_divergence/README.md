# condition_absorbed_comment_prettier_divergence

Comments between `)` and `{` or between keyword and `(` are preserved
in their original position instead of being absorbed into the condition.

- Switch: `switch (x) /* c */ {}` — Prettier absorbs into `switch (x /* c */) {}`
- Catch: `catch /* c */ (e) {}` — Prettier absorbs into `catch (/* c */ e) {}`

Per comment placement policy, the user's chosen position is preserved.
Both positions are dual-stable in our formatter.

Reason: Comment relocation (comment position). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
