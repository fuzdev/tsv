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

The object-property face of the cross-construct before-`:`/`=` line comment. tsv
preserves the comment at its authored position rather than relocating it. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
