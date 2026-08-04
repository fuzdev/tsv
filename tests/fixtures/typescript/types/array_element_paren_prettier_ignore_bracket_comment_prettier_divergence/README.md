# array_element_paren_prettier_ignore_bracket_comment_prettier_divergence

The bracket suffix sits **outside** the frozen slice, so a comment in it is printed by
the suffix, not swallowed by (or lost to) the freeze. The freeze itself is the sibling
fixture's subject —
[array_element_paren_prettier_ignore_interior](../array_element_paren_prettier_ignore_interior_prettier_divergence/);
this one adds the `[/* c */]` the frozen element carries.

tsv: directive own-line inside the parens, comment inside the brackets
Prettier: directive relocated out to the `=` line, comment hoisted before `[`

```
// tsv                          // prettier
type A = (                      type A = // prettier-ignore
	// prettier-ignore            ((a:  string) => void) /* c */[];
	(a:  string) => void
)[/* c */];
```

## Reason

The paren-interior freeze re-synthesizes the `)` and `[]` outside the frozen slice, so
the suffix is ordinary printed output and its comment follows the ordinary rule — it
stays inside the brackets the author wrote it in
([array_paren_bracket_comment](../array_paren_bracket_comment_prettier_divergence/)).
Prettier freezes the coarser `((…) => void)[]` unit and reaches its own fixed point in
two passes (pinned via `audit_signature.txt`), relocating both the directive and the
comment along the way.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Format-ignore directive.
