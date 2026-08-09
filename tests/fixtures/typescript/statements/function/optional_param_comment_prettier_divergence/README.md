# optional-parameter comment with no annotation - prettier divergence

A comment in the gap between a parameter name and its optional marker
(`function fn(a /* c */?) {}`), where the parameter has **no type annotation** — in both
the function-declaration and arrow spellings.

## Why tsv Differs

**Prettier crashes on this input.** Its TypeScript printer loses track of the comment
and throws:

```
Comment "c" was not printed. Please report this error!
```

so there is no prettier oracle to format against — which is exactly what
`prettier_rejects.txt` records (its trimmed content is the expected-error substring, so
a prettier version that *fixes* the bug fails this fixture and flags the case for
promotion to an ordinary one). tsv parses the input, preserves the comment where the
author wrote it, and is stable.

The gap is real authorial territory: a comment there binds the name to its marker, and
dropping it is content loss. Cataloged beside its rest-parameter sibling in
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript);
the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Expected behavior

- **tsv**: parses, keeps the comment before the `?`, and the input is a fixed point
- **acorn**: accepts (so `expected.json` is an ordinary oracle file — the divergence here
  is with prettier alone, not with the parser)
- **prettier**: throws; no formatted output exists

**Contrast — the annotated and rest spellings do NOT crash prettier.** With a type
annotation prettier formats normally, and the *rest*-parameter version
(`(...a /* c */?)`) is its own fixture,
[rest_optional_param](../../../typescript_specific/rest_optional_param_prettier_divergence/),
where prettier survives but strips the `?` instead. The crash is specific to a
**non-rest, unannotated** parameter.
