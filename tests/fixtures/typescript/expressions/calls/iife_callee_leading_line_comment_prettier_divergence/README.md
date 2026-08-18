# IIFE callee / template tag, comments in the pair's gaps

The IIFE position of the required-pair comment rule: the parens a function callee
(or a tagged template's tag) is required to carry own **both** their gaps — the
`(`→function run and the function→`)` run. The pair is tsv's, not the author's:
`() => {}()` does not parse, so whatever the comments do, the parens print.

- **tsv**: keeps every comment inside the parens, on the family's layout — a run
  that occupies a line takes the expanded shell (a `//` the author glued to the
  `(` stays on the `(` line, an own-line comment keeps its own line, the function
  goes one indent in and the `)` comes back out); an inline block run stays flat.
  The same answer every required pair in the family gives (the
  [assignment target](../../assignment/cast_target_leading_line_comment_prettier_divergence/),
  the [instantiation head](../../../typescript_specific/generics/instantiation_head_paren_leading_line_comment_prettier_divergence/),
  the [sealed optional chain](../../chain/optional_paren_non_null_sealed_leading_line_comment_prettier_divergence/)).

```
( // c1
	() => {}
)();
(() => {} /* c8 */)();
```

- **prettier**: also keeps both runs inside the pair, but renders it differently
  and binds a wider region to it:
  - it never glues the leading comment to the `(` — it pulls the run down to its
    own line first (`(⏎\t// c1⏎\t() => {}⏎)();`);
  - it expands the pair for an inline block run too, where tsv leaves it flat;
  - it pulls a comment written **after** the `)` back inside the parens
    (`(() => {}) /* c12 */()` → `(⏎\t() => {} /* c12 */⏎)()`), which tsv does not:
    that comment is outside the pair as authored and stays there.

  So no comment leaves the pair on either side; the divergence is the rendering,
  plus that one relocation.

With **both** gaps commented (c14–c17) the pair takes ONE expanded shell — the
leading run above the function, the trailing run beside it, the `)` back out —
and there tsv and prettier agree byte-for-byte. Two shells, or a fold with no
shell at all, are the two ways to get this wrong; the fold was what a leading
shell that declined on a commented trailing gap produced, leaving the `//` glued
to a `(` that never broke and the function at the *enclosing* indent.

The claim is the **function** callee/tag position, which is where prettier keeps
the runs inside. A required pair around any other callee hoists a leading run out
in front and leaves a trailing one past the `)`, and tsv matches — pinned here as
the controls: a `new` callee (`new // c6⏎\t(function () {})();`), a class expression
(`// c7⏎(class A {})();`), and a ternary callee (`(a ? b : c) /* c13 */();`).

⚠️ Those controls claim **where the run lands** — outside the pair — and nothing
about the *indent* of the continuation under it. That indent is the `new`→callee
gap's own rule, not this fixture's: the run lands outside the pair (matching
prettier) and the tail under it drops one level
([§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)),
where prettier keeps it flush — see
[await_new_operand_line_comment](../../await_new_operand_line_comment_prettier_divergence/).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
