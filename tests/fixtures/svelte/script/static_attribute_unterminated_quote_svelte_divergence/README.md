# Unterminated quoted value in a static tag head — Svelte Divergence

A top-level `<script>` / `<style>` head reads its attributes with Svelte's
`read_static_attribute`, whose value regex tries three alternatives in order:
`"([^"]*)"`, `'([^']*)'`, then `([^>\s]+)`. On an **unterminated** quote
(`<script a="b>`) the first two fail and the third matches the raw run `"b`.
Svelte then tests only the run's *first* character to decide it was quoted —
`raw[0] === '"'` — and strips a character off each end, so `"b`.slice(1, -1)
yields the empty string: the `b` is **deleted from the AST**, and with it the
rest of the tag head.

**tsv** rejects instead (`Unterminated string literal in template`). The
one-sided `raw[0]` test is an implementation slip, not a behavior worth
matching: the value is unterminated, so every reading of it loses source, and a
formatter that reproduced Svelte's would print `<script a=""></script>` —
silently discarding the author's bytes.

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). The
`tsv_rejects.txt` marker pins tsv's rejection while `expected_svelte.json`
proves Svelte still accepts and records exactly what it keeps (a zero-width
`Text` at the quote, and an empty `Program`).

The rest of the static reader's grammar — the value alternatives tsv *does*
follow, and the head shapes both parsers reject — is pinned by the sibling
[static_attribute_grammar](../static_attribute_grammar/) fixture.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §Static
Attribute Reader Corrections (Unterminated quoted value).
