# Async generic arrow param decorator (`async <T>(@dec a) => a`) — Svelte Divergence

A parameter decorator is invalid on an arrow function in every form — tsc and
prettier reject `(@dec a) => a`, `<T>(@dec a) => a`, and `async (@dec a) => a`
(*"Decorators are not valid here"*), and acorn-typescript rejects them too
(*"Leading decorators must be attached to a class declaration"*). **tsv rejects
all of them** (`Decorators are not valid here`) — the ordinary drop-in
rejections pinned by the `input_invalid_*` cases in
[typescript_specific/decorators/parameter_arrow](../../../../typescript_specific/decorators/parameter_arrow/).

The one exception is the **async generic** form (`async <T>(...)`), which
acorn-typescript alone accepts. That form takes a separate path through acorn's
arrow parsing, and the decorator check every other arrow form applies is never
reached on it — so acorn builds a `Decorator` node on the parameter while tsc
still rejects the input. tsv follows tsc and rejects, matching every other arrow
form; the inconsistency is acorn's.

Because the canonical parser accepts this input, the rejection cannot be an
`input_invalid_*` fixture (which requires *both* parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts (recording the decorated parameter it builds).

**Upstream candidate**: @sveltejs/acorn-typescript — the async-generic arrow path
should reject a parameter decorator like every other arrow form does.

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Async generic arrow param decorator).
