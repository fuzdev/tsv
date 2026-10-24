# Parser correction: a member access after `export default @dec class Foo {}`

tsc's parser reads a decorated `export default class` as a class *declaration*, which ends
at its `}`: what follows is the next statement, and `.x` begins none, so tsc rejects
(TS1128 `Declaration or statement expected.`), and prettier, a tsc front end, throws.
acorn-typescript reads the decorated value as a class *expression*, the export's value, which
the member access continues: `(@dec class Foo {}).x`, one value — `expected_svelte.json`.

tsv takes tsc's reading, so the class ends the export and `.x` is a syntax error
(`tsv_rejects.txt`), as it is after the `abstract` form
([after_export_default_abstract](../after_export_default_abstract_svelte_divergence/)'s
`input_invalid_member_follower`, which acorn rejects too). A follower that begins a statement
parses as one ([after_export_default_next_statement](../after_export_default_next_statement_svelte_divergence/)).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§TypeScript Corrections.
