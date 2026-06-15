# key_colon_line_comment_prettier_divergence

A **line** comment in the gap between a (non-optional) property key and its `:`
annotation. tsv preserves the comment after the key; because a line comment must
end its line, the annotation is forced onto the next line. Prettier relocates the
comment to trail the member's `;`.

Applies to all three type-element contexts — interface members, type-literal
members, and class properties:

```
interface I {        type T = {           class A {
  a // c1             b // c2             c // c3
  : number;          : number;           : number = 1;
}                    };                   }
```

- Prettier: `a: number; // c1` (relocates the comment after `;`)
- Ours: keeps `// c` after the key, the annotation on the next line (above)

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

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
