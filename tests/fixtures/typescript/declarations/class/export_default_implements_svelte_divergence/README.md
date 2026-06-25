# Parser correction: `export default class implements I {}`

acorn-typescript **rejects** an anonymous default-export class whose first
heritage clause is `implements` ("The keyword 'implements' is reserved") — it
tries to read `implements` as the (optional) class name and trips the
reserved-word check. The form is **spec-valid**: an `export default` class may
be anonymous and still carry an `implements` clause. tsv parses it
(`expected_ours.json`; the canonical parser errors → `expected_svelte.json`),
emitting `id: null` for the anonymous class as it does everywhere.

This mirrors the class-*expression* fix
([expression_implements](../expression_implements_svelte_divergence/)) on the
declaration path; a `name_required` declaration (`class implements Foo {}` as a
statement) still errors. prettier formats the input identically (parse
correction only — no `output_prettier.svelte`).

A module may have only one `export default`, so the fixture is a single
statement.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§TypeScript Corrections.
