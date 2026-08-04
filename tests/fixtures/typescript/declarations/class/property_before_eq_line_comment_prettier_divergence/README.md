# Divergence: class property before-`=` line comment (preserve, lossless)

A line comment between a class property name (or its type annotation) and its `=`
initializer (`a // c⏎= 1;`). tsv keeps the comment after the name and drops
`= value` to a continuation line **indented one level** (uniform forced-continuation
indent). Prettier **relocates** the comment past the value to end-of-line
(`a = 1; // c`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate to end-of-line)
class C {                                  class C {
	a // c                                     a = 1; // c
		= 1;                               }
}
```

**Why tsv preserves rather than trails:** when a *second* comment already trails the
member (`b // c1⏎= 2; // c2`), prettier's relocation **merges both onto one line** —
`b = 2; // c1 // c2`, where `// c2` becomes text inside `// c1` (information loss).
tsv keeps the two comments distinct. Trailing the before-`=` comment would re-import
that loss, so tsv preserves position.

The **own-line** authoring (`a⏎// c⏎= 1;`) pulls up to trail the name and
reaches input under tsv — one pass (`unformatted_ours_own_line.svelte`).
Prettier instead crosses the `=` and hangs the comment leading the value
(`a =⏎\t// c⏎\t1;`) — stable in one pass, and dual-stable
(`variant_own_line.svelte`): the comment now leads the value, a position both
formatters preserve — the same landing its own-line **block** sibling
[property_before_eq_own_line_block_comment](../property_before_eq_own_line_block_comment_prettier_divergence/)
pins — distinct from the trailing float it applies to input's authoring
(`output_prettier.svelte`).

The class-property face of the cross-construct before-`=` initializer line comment
(also enum members and variable declarators). Unlike the before-`:`
[continuation indent](../../variable/binding_key_colon_line_comment_prettier_divergence/)
(where prettier keeps the continuation flush), prettier here moves the comment
entirely. See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
