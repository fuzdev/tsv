# Divergence: computed-key `]`→`=` line comment (class, preserve, lossless)

A line comment in a class computed property key's `]`→`=` gap
(`[y] // c⏎= 2;`). tsv keeps the comment after the `]` and drops `= value` to a
continuation line **indented one level** (uniform forced-continuation indent).
Prettier **relocates** the comment past the value to end-of-line
(`[y] = 2; // c`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate to end-of-line)
class C {                                 class C {
	[y] // c                              	[y] = 2; // c
		= 2;                              }
}
```

**Why tsv preserves rather than trails:** when a *second* comment already trails
the member (`[z] // c1⏎= 3; // c2`), prettier's relocation **merges both onto one
line** — `[z] = 3; // c1 // c2`, where `// c2` becomes text inside `// c1`
(information loss). tsv keeps the two comments distinct. Trailing the before-`=`
comment would re-import that loss, so tsv preserves position.

The **own-line** authoring (`[y]⏎// c⏎= 2;`) pulls up to trail the `]` and
reaches input under tsv — one pass (`unformatted_ours_own_line.svelte`).
Prettier instead crosses the `=` and hangs the comment leading the value
(`[y] =⏎\t\t// c⏎\t\t2;`) — one pass, and dual-stable
(`variant_own_line.svelte`): the comment now leads the value, a position both
formatters preserve — the same landing its own-line **block** sibling
[computed_key_bracket_own_line_block_comment](../computed_key_bracket_own_line_block_comment_prettier_divergence/)
pins — distinct from the trailing float it applies to input's authoring
(`output_prettier.svelte`).

The computed-key face of the cross-construct before-`=` initializer line comment
(the plain-name face is
[property_before_eq_line_comment](../../../declarations/class/property_before_eq_line_comment_prettier_divergence/));
the object `]`→`:` sibling is
[computed_key_bracket_colon_line_comment](../../../expressions/objects/computed_key_bracket_colon_line_comment_prettier_divergence/).
See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
