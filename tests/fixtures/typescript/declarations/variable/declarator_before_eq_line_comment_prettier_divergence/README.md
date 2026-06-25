# Divergence: variable declarator before-`=` line comment (preserve, lossless)

A line comment between a variable binding and its `=` initializer (`const a // c⏎= 1;`).
tsv keeps the comment after the name and drops `= value` to a continuation line
**indented one level** (uniform forced-continuation indent). Prettier **relocates**
the comment past the value to end-of-line (`const a = 1; // c`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate to end-of-line)
const a // c                              const a = 1; // c
	= 1;
```

**Why tsv preserves rather than trails:** when a *second* comment already trails the
construct (`const b // c1⏎= 2; // c2`), prettier's relocation **merges both onto one
line** — `const b = 2; // c1 // c2`, where `// c2` becomes text inside `// c1`
(information loss). tsv keeps the two comments distinct (`const b // c1⏎= 2; // c2`).
Trailing the before-`=` comment would re-import that loss, so tsv preserves position.

The variable-declarator face of the cross-construct before-`=` initializer line
comment (also enum members and class properties). Unlike the before-`:`
[continuation indent](../binding_key_colon_line_comment_prettier_divergence/)
(where prettier keeps the continuation flush), prettier here moves the comment
entirely. See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
