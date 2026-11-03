# Import fixtures

Fixtures for `tsv_ts::parse_imports` and `tsv_svelte::parse_imports`: a source in, its import
records out — every module request (static import, re-export, dynamic `import()`,
`import x = require()`), where it is, its specifier decoded, whether it is type-only, its
phase and its attributes clause, plus whether the source uses module syntax. This is its own
tree, apart from `tests/fixtures/`, because its claim is a record list, not a parse tree or
a formatting fixed point; the root `.formatignore` prunes it.

## Layout

- `static/` — `import … from` / `import '…'`, the type-only forms included
- `re_export/` — `export … from` / `export * from`, the type-only forms included
- `module_syntax/` — what makes `hasModuleSyntax` true (a top-level import or export
  declaration, `import.meta`, a top-level `await` / `for await` / `await using`), what does
  not (an `await` in a function, a namespace body or an enum initializer), and the
  Script-goal retry
- `dynamic/` — `import(…)`: literal, template and computed specifiers, an options argument,
  and the positions one can sit in
- `import_equals/` — `import x = require('y')` and its variants; the entity-name form is no
  record, and neither is one inside a namespace body (tsc's TS1147) — the rule `static/`
  and `re_export/` pin for theirs too, while a dynamic import there is a record
- `attributes/` — the `with {…}` clause's span
- `escapes/` — decoded specifier values, and UTF-16 offsets around non-ASCII text and a BOM
- `phases/` — `import source` / `import defer` and their dynamic forms
- `refused_subtrees/` — positions the type strip refuses without walking (an enum
  initializer, a value namespace body, an `export =` operand, the operand of an `as` /
  `satisfies` whose blanking would regroup its expression), whose imports are records all
  the same; and an angle-bracket assertion, which the strip refuses but walks
- `ambient/` — `declare`d code and bodiless signatures, which hold no records, each beside
  the runtime position it would otherwise be
- `types/` — `import(…)` inside a type (a computed key in an interface, a type literal, an
  annotation or an assertion's type), which is no record, and a type-position import (a
  known gap: no record)
- `not_imports/` — `import` that is not a module request (`import.meta`, a key, a member
  name, text in a string or comment)
- `svelte/` — the two top-level scripts in document order, and the places that are not the
  component's code (a nested `<script>`, a template expression)

## Files

Every fixture directory holds:

- `input.ts` or `input.svelte` — the source, parsed at the unset goal (Module, retried as
  Script);
- `expected.json` — the records as tsv's bindings return them:

  ```json
  {"hasModuleSyntax": true, "imports": [{"kind": "static", "start": 0, "end": 23,
    "specifier": {"value": "./a.js", "start": 15, "end": 21}, "typeOnly": null,
    "phase": null, "attributes": null}]}
  ```

  `kind` is `static`, `re_export`, `dynamic` or `import_equals`; `typeOnly` is `null`,
  `statement` or `specifiers`; `phase` is `null`, `source` or `defer`. Every offset is a
  UTF-16 code unit offset in the language's own coordinates — a TypeScript source's count
  a leading BOM, as acorn does; a Svelte document's elide it, as Svelte's `parse` does.
  `specifier` spans the literal's contents, its quotes excluded; `attributes` spans a
  static clause from `with` through `}`, or a dynamic import's options argument;
- `README.md` — what the fixture pins;
- `oracle.txt`, only where the canonical parser rejects the input (below).

## Oracle

`expected.json` is generated, never hand-written:

```bash
deno task imports:fixtures:update [pattern...]     # write expected.json
deno task imports:fixtures:validate [pattern...]   # oracle freshness + tsv (-v for diffs)
cargo test --test imports_fixtures_tests           # tsv against the committed files
```

The oracle walks the canonical parser's tree — acorn-typescript's for `input.ts` (at Module
goal, or at Script goal when Module rejects it), Svelte's `parse` for `input.svelte` (its
`instance` and `module` script programs) — and never tsv's AST, so it grades tsv's
collector independently. The generator runs the same walk over tsv's own parse wire too
and fails unless the two agree, so a fixture pins only what both parsers' trees say. Its
rules, stated once, are `RULES` in `crates/tsv_debug/src/imports_fixtures.rs`; tsv's
collector implements the same ones.

Where the canonical parser rejects valid input, the fixture's `oracle.txt` holds `tsv-wire`
on its first line and the reason after it, and the same walk reads tsv's own parse wire
instead (graded against the canonical parsers by `tests/fixtures/`). Those are the import
phases, which acorn-typescript does not parse; a trailing comma in `import(…)` and
`await using`, which it rejects too; an `await` in a namespace body, which it rejects as it
would in a class static block; and an `assert` call on the line after an import, which it
reads as a legacy import-assertion clause. The generator fails on a rejected input with no
marker, and on a marker whose input the canonical parser accepts.

`imports:fixtures:validate` is the only place the committed files' freshness against the
oracle is graded; `cargo test` compares tsv with the committed files alone.
