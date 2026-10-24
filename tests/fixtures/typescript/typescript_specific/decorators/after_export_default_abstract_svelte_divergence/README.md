# Parser correction: `export default @dec abstract class Foo {}`

acorn-typescript **rejects** an `abstract` modifier between a decorator list and the
`class` keyword of an `export default` value ("Unexpected token" at `class`): it reads
the decorated default value as a class expression, which takes no modifier. tsc's parser
**accepts** it — `export default` takes a decorated class *declaration*, whose modifiers
include `abstract` — and prettier prints it (`export default⏎@dec⏎abstract class Foo {}`,
the layout of the undecorated `export default abstract class` and the decorated
`export default @dec class` alike). tsv parses it (`expected_ours.json`; the canonical
parser errors → `expected_svelte.json`) as the class expression it builds for
`export default @dec class Foo {}`, with `abstract: true` — and, as tsc's declaration does,
the value ends at the class's `}` with no `;` required: what follows is the next statement, an
operator (`{} + 1` is the export, then `+1;`) or not (`{} foo();`), each printed on its own line
as prettier prints it. A `;` right after the `}` is the export's own and ends it there, one
statement (tsc reads an empty statement after the declaration). A follower that begins no statement is a syntax error, as for tsc
(`{}.x`, `input_invalid_member_follower`). The decorated class without `abstract` ends there
too, tsc's reading where acorn's expression would take the operator
([after_export_default_next_statement](../after_export_default_next_statement_svelte_divergence/)).

The admission is owed to the `export default` position alone, under the same
`[no LineTerminator here]` gate the decorators-first form applies: `x = @dec abstract
class {}` (prettier rejects) and `export default @dec abstract⏎class Foo {}` (tsc
rejects, TS1146) stay rejected.

A module may have only one `export default`, so the two followers ride in two scripts, the
module script's and the instance script's.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§TypeScript Corrections.
