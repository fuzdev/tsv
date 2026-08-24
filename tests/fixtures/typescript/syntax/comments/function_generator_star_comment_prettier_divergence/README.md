# function_generator_star_comment_prettier_divergence

A comment between the `function` keyword and a generator's `*` normalizes to after
the `*` (`function* /* a */ a()`), from either authoring — spaced or glued —
pinned by `unformatted_ours_before_star.svelte`. Prettier agrees for a function
**declaration** and relocates for a function **expression**, carrying the comment
into the body.

- Input: `function /* a */ *a() {}` — Prettier: `function* /* a */ a() {}` — Ours: the same
- Input: `const b = function /* b */ *() {}` — Prettier: `function* () /* b */ {}` (into the body) — Ours: `function* /* b */ ()`

The glued spelling (`function/* b */*()`) is where a printer stepping its cursor one
byte past `function` for the `*` instead of finding it **drops** the comment outright:
with a comment in between the cursor lands *inside* the comment and the
emitter whose range starts there skips it, and the spaced spelling survives only
because one byte happens to stop short of the comment. The `*` is found, not assumed.

Keeping the comment *before* the `*` would be the comment-position default, and it
is what the `async`→`*` method gap already does
(`../../../expressions/objects/async_star_comment_prettier_divergence/`); it is not
taken here because it would turn the declaration's prettier match into a divergence
— a taste verdict rather than the drop fix this fixture pins.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
