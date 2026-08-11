# async_function_keyword_comment_prettier_divergence

A comment between a function head's opening modifier — `async` or `declare` — and
the `function` keyword is preserved in the gap the author wrote it in. Prettier
relocates it, and where it lands depends on whether the function is named.

- Input: `async /* a */ function a() {}` — Prettier: `async function /* a */ a() {}` (after the keyword, before the name)
- Input: `async /* c */ function () {}` — Prettier: `async function () /* c */ {}` (into the body)
- Ours: both preserve between the two keywords

`declare` behaves identically (`declare /* f */ function f(): void;`), which is what
makes this the modifier→keyword rule rather than an `async` special case.

The same rule the modifier→keyword sibling
`../declaration_keyword_name_prettier_divergence/` states for `abstract /* b */
class B {}`, and the same one the `async`→parameters gap of an async arrow states
(`../../../expressions/arrow/async_before_params_comment_prettier_divergence/`).
Prettier's two destinations here are keyed on whether a name follows, so its
placement is no oracle.

The declaration, the expression and the bodiless overload signature are printed by
three different builders, and they used to answer this gap three ways: the
declaration preserved, while the other two emitted a bare `async ` and let the
gap's comments fall through to whichever emitter came next — the keyword→name gap
for a named function, the parameter list for an anonymous one — which is how the
comment ended up on prettier's side of the `function` keyword. All three share one
head emitter now (`Printer::push_function_keyword_head`), and each caller reads its
following gap from the cursor that emitter returns: reading it from the node's span
start instead re-claimed the region and printed the comment twice.

Both positions are dual-stable in our formatter — prettier's output is a fixed
point of tsv too, pinned by `variant_after_keyword.svelte`, so a file already run
through prettier does not churn on the way back.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
