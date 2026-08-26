# Brand check with no binding class — tsv defers the ecma262 early error

## Why tsv Diverges

`#x in y` is the sole production in which a private name stands as an operand rather than
as part of a member access or a declaration: ecma262
`RelationalExpression : PrivateIdentifier in ShiftExpression`. The production is in the
grammar unconditionally, and nothing in it — or in the class productions — confines it to a
class.

⚠️ **The confining rule is BINDING, not containment, and it is not local.** It is
`AllPrivateIdentifiersValid`, a static semantic threaded from the *Script* / *Module* early
error down the whole tree carrying a list of names, extended at each `ClassBody` by that
body's `PrivateBoundIdentifiers`; the `in` production returns `false` unless the list
already contains the name. So the question is not "is this inside a class body" but "did an
enclosing class body **declare** this name" — and the Script rule even carries a direct-eval
carve-out, `PerformEval` re-running it against the *caller's* private environment. There is
no reading of it a parser can answer from the production's own context, which is precisely
the bucket tsv defers to a future diagnostics layer rather than answering in the parser (see
[CLAUDE.md §Strict Mode Only](../../../../../CLAUDE.md#strict-mode-only)).

The oracles split along that same line, and tsv sides with the two that read the grammar:

- **tsc's parser accepts.** The rejection is `TS18016` ("Private identifiers are not allowed
  outside class bodies"), raised by `checkGrammarPrivateIdentifierExpression` in
  `checker.ts` — the checker's grammar pass, not a `parseDiagnostics` entry (the same shape
  as `TS1206` in the async-generic-arrow entry). ⚠️ `TS18016` *also* has a genuine
  `parser.ts` raise site, for a private name where an ordinary identifier was expected — the
  code alone is not evidence of which pass rejected, only its raise site is.
- **Prettier formats it**, byte-identically to tsv — it reaches this file's `<script>`
  through its own TypeScript parser, not through Svelte's.
- **acorn (and so Svelte) rejects**, tracking the early error in the parser — and tracking
  the *binding* form of it, which is why its two messages differ: `Unexpected token` with no
  class in sight, `Private field '#x' must be declared in an enclosing class` inside one.

Both forms are in `input.svelte`, because both are the same deferral: tsc accepts the second
too (`getContainingClass` is satisfied, and the `in` operand is exempt from `Cannot find
name`), leaving only a semantic `TS2339` on the property.

## Status

- **tsv parser**: accepts both forms — the early error is deferred, not the grammar widened
- **Svelte/acorn**: rejects both — a stricter reading, not a wrong one
- **Prettier**: formats both; `input.svelte` is its fixed point

## Scope

The acceptance is a property of the **expression parser**, so it is reachable from every
position that parses an expression — including template ones this fixture cannot hold:
`<div {...#x in y}></div>`, and a sequence's `{ #x in y}` where the marker is separated
from the `{` so [the placement guard](../../../svelte/elements/textarea_block_tag_placement/)
does not apply. Those spellings have no `input.*` form, since prettier reaches the
*template* through Svelte's parser and rejects them; only the `<script>` position has a
formatter to pin against.

The fixture directory is named for the headline form; the rule it records is the binding
one, so the in-a-class-but-unbound form belongs to it too and is pinned alongside.

The **glued** template spellings are not this divergence at all — `{#x in y}` in RCDATA
content or an attribute value is rejected by tsv as a block written where only a sequence
belongs, matching Svelte's own guard.

## References

- ecma262 §sec-static-semantics-allprivateidentifiersvalid — the SDO, including the
  `RelationalExpression : PrivateIdentifier in ShiftExpression` case and the `ClassBody`
  case that extends the name list
- ecma262 §sec-scripts-static-semantics-early-errors and
  §sec-module-semantics-static-semantics-early-errors — where it is invoked with an empty
  list (the Script one carrying the direct-eval carve-out)
- ecma262 §sec-relational-operators — the production itself

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
