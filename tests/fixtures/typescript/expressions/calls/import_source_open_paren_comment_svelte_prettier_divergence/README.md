# import_source_open_paren_comment_svelte_prettier_divergence

A comment trailing a **`source`-phased** dynamic import's opening paren
(`import.source( // c`) is preserved on the `(` line, as it is for every other call
shape — and here neither oracle has an answer to compare it with.

## Why tsv Differs — from BOTH oracles

- **Svelte's parser** (acorn) has no import-phase grammar for the *expression* form and
  rejects `import.source(…)` outright (`The only valid meta property for import is
  'import.meta'`), so there is no `expected.json` — the parser claim is carried by
  `expected_ours.json` plus an `expected_svelte.json` holding
  `{"error": "failed to parse"}`.
- **Prettier** throws: its TypeScript parser reports
  `'source' is not a valid meta-property for keyword 'import'`, so there is no formatted
  output either. `prettier_rejects.txt` records that (trimmed content = the
  expected-error substring, checked live, so a prettier release that formats the
  proposal fails this fixture and flags it for promotion).

What the fixture holds is the **printer** claim — the input is a fixed point, the `(`-line
comment stays, and the author blank below it survives — on the phased head's own code
path (`build_import_open_doc`'s dotted pair, and a scan anchored past the phase word).
The `defer` phase, which prettier does format, carries the same claims against a live
oracle in
[import_phase_open_paren_comment](../import_phase_open_paren_comment_svelte_prettier_divergence/);
the layout reasoning is the unphased
[import_open_paren_comment](../import_open_paren_comment_prettier_divergence/)'s.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections)
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
