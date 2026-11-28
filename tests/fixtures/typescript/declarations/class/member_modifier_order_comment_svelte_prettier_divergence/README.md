# member_modifier_order_comment_svelte_prettier_divergence

A class member's modifiers print in the canonical order whatever order the source wrote them
in (`unformatted_ours_reordered`), and each comment in the run stays after the keyword it
followed in the source. That is what keeps the meaning when the run is reordered: every
member modifier but `static` is line-guarded, so a `//` or a multi-line block comment can only
follow `static`, and it must stay there.

- Source: `override public static // l⏎x = 1;`
- Ours: `public static // l⏎override x = 1;` (the break stays after `static`)
- Prettier: `public static override // l⏎x = 1;`, which reparses as a field named `override`
  and a field `x` (its next pass prints `public static override; // l`), so prettier changes
  the program; `audit_signature.txt` pins that chain
- Two block comments in one run: `readonly /* a */ static /* b */ t` → ours
  `static /* b */ readonly /* a */ t`, prettier `static readonly /* b */ /* a */ t`
- Decorated: `@dec static // c⏎public s` → ours `@dec public static // c⏎s`, prettier
  `@dec // c⏎public static s`

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

The parse diverges too: acorn-typescript splits `static` from the member across a line break
(`static // l⏎override x` reads as a field named `static` and a field `x`), where tsc,
prettier and ECMAScript read one static member. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
