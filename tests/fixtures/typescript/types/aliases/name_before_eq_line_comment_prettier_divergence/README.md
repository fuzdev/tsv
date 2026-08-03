# Divergence: type-alias name→`=` line comment (preserve, lossless)

A line comment between a type alias's name and its `=` (`type A // c⏎= number;`).
tsv keeps the comment after the name and drops `= type` to a continuation line
**indented one level** (uniform forced-continuation indent). Prettier **relocates**
the comment across the `=` and hangs it leading the RHS, reached non-idempotently:
its first pass glues the comment to the `=` (`type A = // c⏎\tnumber;` —
`output_prettier.svelte`, unstable), its second drops it to its own line
(`type A =⏎\t// c⏎\tnumber;` — the fixed point, pinned by `audit_signature.txt`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate past `=`, hang)
type A // c                               type A =
	= number;                             	// c
                                          	number;
```

Unlike the declarator/enum/class-property faces of this family, prettier's
landing here **hangs the RHS rather than floating the comment to end-of-line**,
so a second trailing comment stays distinct in both formatters (`type B` case) —
the divergence is comment **position** (the author bound the comment to the
name; prettier re-binds it to the value), not information loss.

- `unformatted_ours_spaces.svelte` — the flush authoring: tsv normalizes it to
  input; prettier walks it to the hang in two passes, the unstable first pass
  pinned as `prettier_intermediate_to_variant_spaces.svelte`.
- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv pulls the
  comment up to trail the name and reaches input — one pass. Prettier reaches
  the hang in one pass (no intermediate).
- `variant_own_line.svelte` — prettier's landing form, dual-stable: the comment
  sits after the `=`, leading the RHS — a different syntactic position, which
  both formatters preserve (the value-gap own-line rule pinned in
  [rhs_leading_comment](../rhs_leading_comment/)).

The type-alias face of the cross-construct before-`=` initializer line comment
([declarator](../../../declarations/variable/declarator_before_eq_line_comment_prettier_divergence/),
[enum member](../../../declarations/enum/member_before_eq_line_comment_prettier_divergence/),
[class property](../../../declarations/class/property_before_eq_line_comment_prettier_divergence/)).
The own-line **block** sibling is
[name_before_eq_own_line_block_comment](../name_before_eq_own_line_block_comment_prettier_divergence/).
See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
