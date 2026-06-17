# optional_marker_line_comment_prettier_divergence

A **line** comment in the gap between an optional `?` marker and the member's
`:` annotation. tsv preserves the comment after `?`; because a line comment must
end its line, the `: type` annotation drops to a continuation line **indented one
level** (the uniform forced-continuation indent — it reads as part of the member,
not a sibling). Prettier relocates the comment to trail the member's `;`.

Applies to all three type-element contexts — interface members, type-literal
members, and class properties:

```
interface I {        type T = {           class A {
  a? // c1             b? // c2             c? // c3
    : number;           : number;            : number = 1;
}                    };                   }
```

- Prettier: `a?: number; // c1` (relocates the comment after `;`)
- Ours: keeps `// c` after `?`, the `: type` on a continuation line indented one
  level (above)

This is also a **content-loss fix**: rendering the line comment inline would
swallow the `: number` annotation as comment text (`a? // c : number` →
non-idempotent, the annotation is lost). Preserving the comment must force a
break, not consume the rest of the line.

Both positions are dual-stable in our formatter. Per the comment-position
policy, we preserve the user's original comment position. The block-comment
counterpart (which stays inline) is documented per context in
`types/type_literal/optional_marker_comment_prettier_divergence`,
`types/type_members/modifier_after_comment_prettier_divergence`, and
`declarations/class/optional_marker_comment_prettier_divergence`.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
