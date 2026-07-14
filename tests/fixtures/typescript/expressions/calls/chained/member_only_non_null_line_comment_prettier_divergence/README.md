# Member-only chain with an interior line comment and a trailing non-null `!`

The non-null variant of
[member_only_interior_line_comment](../member_only_interior_line_comment_prettier_divergence/):
a member-only chain (pure property access, **no calls**) whose interior line
comment forces the chain to break, where a member carries a trailing non-null
assertion (`.bar!`, `?.bar!`).

- **tsv**: keeps each comment where the author wrote it, breaks the chain at the
  member, and keeps the `!` **glued to its member** — `.bar!` / `?.bar!` on one
  line, never a break before the `!`.
- **prettier**: breaks after `=` and hoists the RHS onto its own line
  (`const a =⏎\t\tfoo // c⏎\t\t.bar!;`); a single own-line comment (case c) is
  hoisted after `=` and the chain stays inline (`const c =⏎\t\t// c⏎\t\tfoo.bar!;`).
  prettier also keeps `.bar!` glued.

The `!`-glue is the property under test. A non-null `!` binds to the token before
it under `[no LineTerminator here]`, so a break before `!` is a **syntax error** —
tsv must never emit `.bar⏎!`. The comment-forced break splits the chain across
lines (a `//` must end its line), and the trailing `!` has to ride along on its
member's line. This matches prettier's own `!` placement; the divergence is only
the comment position and the continuation layout, the same
[comment-position philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
as the sibling — comments stay where the author placed them.

Reason: Comment relocation. See
[conformance_prettier.md §Comment relocation](../../../../../../../docs/conformance_prettier.md#comment-relocation).
