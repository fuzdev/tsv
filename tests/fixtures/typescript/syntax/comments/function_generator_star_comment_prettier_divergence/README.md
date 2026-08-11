# function_generator_star_comment_prettier_divergence

A comment between the `function` keyword and a generator's `*` normalizes to after
the `*` (`function* /* a */ a()`), from either authoring — spaced or glued —
pinned by `unformatted_ours_before_star.svelte`. Prettier agrees for a function
**declaration** and relocates for a function **expression**, carrying the comment
into the body.

- Input: `function /* a */ *a() {}` — Prettier: `function* /* a */ a() {}` — Ours: the same
- Input: `const b = function /* b */ *() {}` — Prettier: `function* () /* b */ {}` (into the body) — Ours: `function* /* b */ ()`

The glued spelling (`function/* b */*()`) used to **drop** the comment outright: the
printer stepped its cursor one byte past `function` for the `*` instead of finding
it, so with a comment in between the cursor landed *inside* the comment and the
emitter whose range started there skipped it. The spaced spelling survived only
because one byte happened to stop short of the comment. The `*` is found now.

Keeping the comment *before* the `*` would be the comment-position default, and it
is what the `async`→`*` method gap already does
(`../../../expressions/objects/async_star_comment_prettier_divergence/`); it is not
taken here because it would turn the declaration's prettier match into a divergence
— a taste verdict rather than the drop fix this fixture pins.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
