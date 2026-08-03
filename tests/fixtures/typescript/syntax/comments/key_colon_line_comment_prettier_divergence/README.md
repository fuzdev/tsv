# key_colon_line_comment_prettier_divergence

A **line** comment in the gap between a (non-optional) property key and its `:`
annotation. tsv preserves the comment after the key; because a line comment must
end its line, the `: type` annotation drops to a continuation line **indented one
level** (the uniform forced-continuation indent — it reads as part of the member,
not a sibling). Prettier relocates the comment to trail the member's `;`.

Applies to all three type-element contexts — interface members, type-literal
members, and class properties:

```
interface I {        type T = {           class A {
  a // c1             b // c2             c // c3
    : number;          : number;           : number = 1;
}                    };                   }
```

- Prettier: `a: number; // c1` (relocates the comment after `;`)
- Ours: keeps `// c` after the key, the `: type` on a continuation line indented
  one level (above)

This is also a **content-loss fix**: rendering the line comment inline would
swallow the `: number` annotation as comment text (`a // c : number` →
non-idempotent, the annotation is lost). Preserving the comment must force a
break, not consume the rest of the line.

A **block** comment in the same gap stays inline in both formatters
(`a /* c */: number`), so it is not a divergence — only a line comment forces
the break. The optional-marker counterpart (a line comment between `?` and `:`)
is documented in `syntax/comments/optional_marker_line_comment_prettier_divergence`.

Both positions are dual-stable in our formatter. Per the comment-position
policy, we preserve the user's original comment position.

The **own-line** authoring of the same line comment (`a⏎// c1⏎: number`) pulls
up to trail the key and reaches input under tsv — one pass, the same
continuation form (`unformatted_ours_own_line.svelte`). Prettier instead
crosses the `:` on its first pass (`a: // c1⏎number`, the
`prettier_intermediate_to_variant_own_line.svelte` form) and floats the comment
to trailing on its second — `variant_own_line.svelte`, the same form as
`output_prettier.svelte`, dual-stable. The own-line **block** sibling is
[key_colon_own_line_block_comment](../key_colon_own_line_block_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
