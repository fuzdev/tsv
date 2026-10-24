# Parser correction: what follows `export default @dec class Foo {}`

tsc's parser reads a decorated `export default class` as a class *declaration*, which ends at
its `}` with no `;`: what follows is the next statement, an operator (`{} + 1` is the export,
then `+1;`) or not (`{} foo();`). prettier prints each on its own line, and
`export default⏎@dec⏎class Foo {}⏎+1;` is its own fixed point. acorn-typescript reads the
decorated value as a class *expression*, the export's value, so an operator continues it
(`(@dec class Foo {}) + 1`, one value — `expected_svelte.json`'s module script) and a statement
on the same line after it is a syntax error ("Unexpected token" at `foo`). Across a line break,
a follower that continues no expression ends acorn's value by ASI, so the instance script's
`{}⏎foo();` reads the same to both.

tsv takes tsc's reading (`expected_ours.json`), as it does for the `abstract` form
([after_export_default_abstract](../after_export_default_abstract_svelte_divergence/)): the
class ends the export at its `}`, and the wire keeps the `ClassExpression` acorn builds for
the value alone. Both same-line forms (`unformatted_inline`) and a spaced twin
(`unformatted_spaces`) format to the two-statement input.

A module may have only one `export default`, so the two followers ride in two scripts, the
module script's and the instance script's.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§TypeScript Corrections.
