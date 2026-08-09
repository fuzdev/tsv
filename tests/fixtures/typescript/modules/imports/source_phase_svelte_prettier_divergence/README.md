# source-phase import - Svelte + prettier divergence

The static source-phase import (`import source x from 'x'`), including the form whose
binding is a contextual keyword (`import source type from 'y'` — a binding *named*
`type`, not a type-only import).

## Why tsv Differs — from BOTH oracles

This is a TC39 proposal tsv supports and neither oracle does, so both halves of the
usual pair are missing at once:

- **Acorn-typescript** has no source-phase grammar and rejects the input, so there is no
  `expected.json` — the parser claim is carried by `expected_ours.json` plus an
  `expected_svelte.json` holding `{"error": "failed to parse"}`.
- **Prettier** reads `source` as an ordinary default-import binding and then throws
  `'=' expected.` — so there is no formatted output either, which is what
  `prettier_rejects.txt` records (trimmed content = the expected-error substring, checked
  live, so a prettier release that adds the proposal fails this fixture and flags it for
  promotion).

The *parser* is graded by test262; what this fixture adds is the **printer** claim —
that tsv keeps the syntax stable — against a live oracle rather than a hand-written
string. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections)
and [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript).

## Expected behavior

- **tsv**: parses both forms and the input is a fixed point
- **Svelte/acorn**: fails to parse (`expected_svelte.json`)
- **prettier**: throws `'=' expected.`; no formatted output exists

**Boundary.** `import source ImportedBinding FromClause` takes exactly **one** binding —
no namespace, no named clause, no second specifier, and no import-equals form. Those
rejections, the `import.source(…)` / `import.defer(…)` dynamic forms, and the
spec-valid-but-unsupported `import source from from 'm'` stay in
`tests/import_phase.rs`, which is where a *rejection* claim belongs.
