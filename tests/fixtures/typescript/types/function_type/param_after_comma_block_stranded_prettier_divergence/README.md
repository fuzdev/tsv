# param_after_comma_block_stranded_prettier_divergence

A function/constructor **type**'s parameter block comment **stranded** after the comma —
the author left a newline before the next param (`(a, /* c */⏎ b)`). tsv respects that
newline and keeps the comment where it was written (trailing the comma line); prettier
attaches it to the preceding param and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)       // prettier (relocate)
type Fn = (                     type Fn = (              type Fn = (
	a, /* c */                      a, /* c */               a /* c */,
	b                               b                        b
) => void                       ) => void                ) => void
```

This is the **type-level** counterpart of the value-level function-parameter case
([param_after_comma_block_stranded](../../../syntax/comments/param_after_comma_block_stranded_prettier_divergence/)):
function/constructor types use a separate parameter printer, but follow the same single
rule. A block **hugging** the next param (`(a, /* c */ b)`, no newline between them) leads
it and both formatters agree; a **stranded** block stays trailing the comma. The stranded
form is stable only once the params wrap (they collapse inline when they fit). The `Combo`
case pairs a **before-comma** block with a stranded after-comma block in the same gap
(`a /* c1 */, /* c2 */⏎ b`): each stays on its own side of the comma while prettier
relocates **both** before it. `Ctor` covers the constructor-type (`new (…)`) form.

The type-level member of the `is_stranded_after_comma_block` family — see the
value-level [declarator](../../../declarations/variable/multiple/after_comma_block_stranded_prettier_divergence/)
and call-argument
[nonlast_arg](../../../expressions/calls/nonlast_arg_after_comma_block_stranded_prettier_divergence/)
siblings.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
