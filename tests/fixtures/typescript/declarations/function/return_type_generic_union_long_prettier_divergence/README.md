# return_type_generic_union_long_prettier_divergence

Prettier has inconsistent special-casing for `null` and `void` in union types within generic return types at the print_width boundary.

tsv: breaks consistently at 101+ chars inside the return type generic
Prettier: function declarations and class methods exceed print_width; arrow functions use assignment break instead

Prettier's behavior varies by declaration kind:
- Function declarations with `null`/`void`: allows line to exceed print_width
- Arrow functions with `null`/`void`: breaks at `=` instead of in the return type
- Class methods with `null`/`void`: allows line to exceed print_width

## Reason

tsv breaks inside the return type generic based on line width, not type keyword. Consistent across function declarations, arrow functions, and class methods.

## Related

- `return_type_generic_union_long/` — non-diverging cases (with `B` instead of `null`)
