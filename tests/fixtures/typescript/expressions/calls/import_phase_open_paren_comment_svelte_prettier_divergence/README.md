# import_phase_open_paren_comment_svelte_prettier_divergence

A comment trailing a **phased** dynamic import's opening paren
(`import.source( // c`, `import.defer( // c`) is preserved on the `(` line. The phase
only changes the head; the `(`→specifier gap is the same one every call shape answers.

## Why tsv Differs — from BOTH oracles

- **Svelte's parser** (acorn) has no import-phase grammar for the *expression* form and
  rejects `import.source(…)` outright (`The only valid meta property for import is
  'import.meta'`), so there is no `expected.json` — the parser claim is carried by
  `expected_ours.json` plus an `expected_svelte.json` holding
  `{"error": "failed to parse"}`.
- **Prettier** parses it and relocates the comment to its own line as the specifier's
  leading comment, exactly as it does for an unphased `import(…)`, so there *is* a
  formatting oracle and `output_prettier.svelte` records it.

The formatting divergence itself is the call family's, not the phase's — see
[import_open_paren_comment](../import_open_paren_comment_prettier_divergence/) for the
unphased form and the reasoning. This fixture exists because the phased head is a
separate code path (`build_import_open_doc`'s dotted pair, and a scan anchored past the
phase word) that the unphased fixture cannot reach: its own oracle rejects the syntax.

The static source-phase *declaration* is a different construct with its own fixture —
[modules/imports/source_phase](../../../modules/imports/source_phase_svelte_prettier_divergence/),
where prettier throws rather than formatting.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections)
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
