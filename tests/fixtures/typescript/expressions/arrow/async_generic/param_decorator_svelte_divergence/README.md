# Async generic arrow param decorator (`async <T>(@dec a) => a`) — Svelte Divergence

A parameter decorator is invalid on an arrow function in every form — tsc and
prettier reject `(@dec a) => a`, `<T>(@dec a) => a`, and `async (@dec a) => a`
(*"Decorators are not valid here"*), and acorn-typescript rejects them too
(*"Leading decorators must be attached to a class declaration"*). **tsv rejects
all of them** (`Decorators are not valid here`) — the ordinary drop-in
rejections pinned by the `input_invalid_*` cases in
[typescript_specific/decorators/parameter_arrow](../../../../typescript_specific/decorators/parameter_arrow/).

The one exception is the **async generic** form (`async <T>(...)`), and only
because of a separate acorn-typescript bug: acorn **drops every parameter** from
an async arrow that has type parameters (`async <T>(x: T) => x` → `params: []` —
see the sibling [basic_ts](../basic_ts_svelte_divergence/) /
[long](../long_svelte_divergence/) fixtures). The decorator rides along on the
dropped parameter, so acorn *accepts* the input while silently discarding both
the parameter and its decorator. tsc still rejects it (the decorator is invalid),
so tsv follows tsc and rejects, matching every other arrow form.

Because the canonical parser accepts this input (with empty `params`), the
rejection cannot be an `input_invalid_*` fixture (which requires *both* parsers to
reject). This `tsv_rejects.txt` fixture pins the divergence from the other side:
tsv rejects (`tsv_rejects.txt` substring), while `expected_svelte.json` proves
acorn still accepts (and drops the parameter).

**Upstream candidate**: @sveltejs/acorn-typescript — the async-arrow
param-dropping bug (same root cause as the sibling `async_generic` fixtures).

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Async generic arrow params).
