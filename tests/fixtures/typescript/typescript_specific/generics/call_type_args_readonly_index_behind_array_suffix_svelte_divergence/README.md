# Parser correction: `readonly` over an index behind an array suffix, as a type argument

`fn<readonly (typeof a)[]['length']>(x)` is a generic call to tsc: its parser reads `readonly`
over any type and leaves the array-or-tuple rule to its checker (TS1354). acorn-typescript
**rejects** it: it refuses `readonly` over an indexed access while parsing the type
arguments, falls back to a comparison chain, and the empty `[]` there is no member access,
so the chain does not parse either. tsv reads the type (`expected_ours.json`; the canonical
parser errors → `expected_svelte.json`).

The rule is the one every `readonly` operand already follows: where the operand's postfix run
holds an empty `[]`, no value reading of it exists, so the list is read as a type and the
follow token decides whether it closes. The run need not END in the `[]`: an index behind it
(`[]['length']`) leaves the comparison unreadable just the same. Prettier prints a type query
in a pair ahead of `[]`, so `unformatted_bare_query` (`readonly typeof a[]['length']`) is
the authored spelling tsv read already, and this input is its printed form.

An index AHEAD of the suffix (`readonly (typeof a)['k'][]`) is an array type acorn-typescript
reads too, pinned by
[call_type_args_readonly_index_ahead_of_array_suffix](../call_type_args_readonly_index_ahead_of_array_suffix/).

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
