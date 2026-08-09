# Async generic arrow param decorator (`async <T>(@dec a) => a`) — Svelte Divergence

A parameter decorator is invalid on an arrow function in every form, and **prettier
rejects all four spellings** — `(@dec a) => a`, `<T>(@dec a) => a`,
`async (@dec a) => a` and `async <T>(@dec a) => a`. **tsv rejects all four too**
(`Decorators are not valid here`), uniformly.

⚠️ **The oracles do not draw that line in the same place, and tsc's parser is not
where it is drawn.** tsc splits on the type parameters: it raises a *parse*
diagnostic only on the two non-generic forms (**TS1109** `Expression expected.`),
and its parser **accepts both generic forms outright** — `parseDiagnostics` and the
syntactic pass are both empty on `<T>(@dec a) => a` and `async <T>(@dec a) => a`.
The familiar *"Decorators are not valid here"* is **TS1206, a *semantic* diagnostic**
raised by the checker's grammar pass, which is why prettier (which runs those grammar
checks) surfaces it where tsc's parser does not. acorn-typescript rejects three forms
(*"Leading decorators must be attached to a class declaration"* on the two non-generic
ones, *"Unexpected token"* on the plain generic one) and accepts the async-generic one
alone.

| form | tsc parser | tsc checker | acorn | prettier | tsv |
| --- | --- | --- | --- | --- | --- |
| `(@dec a) => a` | **TS1109** | — | reject | reject | **reject** |
| `async (@dec a) => a` | **TS1109** | — | reject | reject | **reject** |
| `<T>(@dec a) => a` | accept | TS1206 | reject | reject | **reject** |
| `async <T>(@dec a) => a` | accept | TS1206 | **accept** | reject | **reject** |

So the rejection tsv keeps here rests on **prettier**, not on tsc's parser: a
construct prettier cannot parse is one tsv rejects, and TS1206 is an
*unconditional-local* grammar error — a parameter decorator on an arrow is invalid in
every context, not in some mode or scope — which is the bucket tsv rejects rather
than defers to a diagnostics layer (root `CLAUDE.md` §Strict Mode Only). The three
non-fixture rows are the ordinary drop-in rejections pinned by the `input_invalid_*`
cases in
[typescript_specific/decorators/parameter_arrow](../../../../typescript_specific/decorators/parameter_arrow/).

What makes the **async generic** form need a fixture of its own is acorn, not tsc:
that form takes a separate path through acorn's arrow parsing where the decorator
check every other arrow form applies is never reached, so acorn builds a `Decorator`
node on the parameter. The inconsistency is acorn's — its own verdict on the plain
generic form is a rejection.

Because the canonical parser accepts this input, the rejection cannot be an
`input_invalid_*` fixture (which requires *both* parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts (recording the decorated parameter it builds).

**Upstream candidate**: @sveltejs/acorn-typescript — the async-generic arrow path
should reject a parameter decorator like every other arrow form does.

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Async generic arrow param decorator).
