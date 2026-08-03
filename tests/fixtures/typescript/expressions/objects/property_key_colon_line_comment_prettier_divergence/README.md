# Divergence: object property before-`:` line comment indents the continuation

A line comment between an object property key and its `:` (`{ a // c⏎: 1 }`). tsv
keeps the comment after the key and drops `: value` to a continuation line
**indented one level** (uniform forced-continuation indent). Prettier **relocates**
the comment — here hoisting it to its own line **before** the key (`// c⏎a: 1`).

```ts
// tsv (preserve + continuation indent)   // prettier (hoist before key)
const o = {                               const o = {
	a // c                                    // c
		: 1                                   a: 1
};                                        };
```

**No merge here** (unlike the trailing `=`/`?` relocations): object-key hoists to a
*leading* position, so a second comment stacks on its own line rather than colliding
on one. With a leading comment already present (`{ // leading⏎b // c1⏎: 2 }`), prettier
stacks the hoisted `// c1` above the key (`// leading⏎// c1⏎b: 2`) — both distinct, no
information loss. That's why object-key is the lone before-delimiter family where
prettier's relocation isn't lossy; the divergence is purely position.

The **own-line** authoring (`a⏎// c⏎: 1`) pulls up to trail the key and reaches
input under tsv — one pass (`unformatted_ours_own_line.svelte`). Prettier
instead crosses the `:` and hangs the comment leading the value
(`a:⏎\t// c⏎\t1`) — stable in one pass, and dual-stable
(`variant_own_line.svelte`): the comment now leads the value, a position both
formatters preserve — the same landing its own-line **block** sibling
[property_key_colon_own_line_block_comment](../property_key_colon_own_line_block_comment_prettier_divergence/)
pins. Note prettier's two destinations from this one gap: from the forced-break
form (input) it hoists *before the key*; from the own-line authoring it hangs
*after the `:`*.

The object-property face of the cross-construct before-`:`/`=` line comment. tsv
preserves the comment at its authored position rather than relocating it. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
