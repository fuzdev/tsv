# Hugged arrow argument, body paren comment

The call-argument hug states — prettier's `couldExpandArg` layouts for an arrow
whose body is a **ternary** or a **call** (through a trailing `!`) — reassemble the
argument from its signature doc and its body doc rather than printing the whole
arrow. That reassembly skips the arrow's body-end→arrow-end gap, which is where an
author-parenthesized body's `)` and any comment before it live. This fixture pins
that the comment survives every one of those states: the sole-argument arm, the
`new` arm, the member-callee and expanded-chain arms, the chain's forced-expansion
builder — which a head call carrying its own arguments selects, not the
`obj.a().b().m(…)` shape — and the last-argument arm of a multi-argument call, in
both the flat and the broken layout. Two further states are reached only once a line
crosses the print width; they live in
[arrow_hugged_body_paren_comment_long](../arrow_hugged_body_paren_comment_long_prettier_divergence/)
with their 100/101 boundary controls.

The **ternary** rows match prettier — it prints its own layout paren around a
ternary body, so both formatters keep the comment inside it.

The **call**-body rows are the divergence: prettier strips the authored parens and
trails the comment past the body (`f((x) => g() /* c */)`), while tsv preserves them
to keep the comment where the author wrote it — the arrow-body rule
[arrows/body_paren_comment](../../arrows/body_paren_comment_prettier_divergence/)
already states, reached here through the hug arms instead of the ordinary body
cascade.

The **object**-body rows sit on the same rule's second half: those parens are
grammar-**required** rather than authored, and the hug state synthesizes them. tsv keeps
the comment inside them, bound to the object; prettier moves it outside
(`f(1, (x) => ({ k: 1 }) /* c */)`), re-associating it with the whole arrow body.

A **line** comment diverges for both body kinds: it forces the retained parens open,
and the argument breaks out with them — the layout the identifier-body sibling
already gives (`f(⏎ (x) => (⏎ x // c⏎ )⏎);`). Prettier drops the parens and hugs the
signature instead. Prettier is non-idempotent there — its second pass floats the `//`
past the body and re-parenthesizes the ternary, detaching the comment from the body
entirely (`f(⏎ (x) => (x ? a : b) // c⏎);`) — so `audit_signature.txt` pins the whole
chain.

The `new` printer's multi-argument path is the one exception to that break-out: it has
no broken-out state to fall to, so the retained parens open in place and the signature
stays hugged to `new F(1, ` (`new F(1, (x) => (⏎ g() // c⏎));`). Both layouts keep the
comment inside the parens the author wrote; only where the argument lands differs.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Arrow body stripped parens) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
