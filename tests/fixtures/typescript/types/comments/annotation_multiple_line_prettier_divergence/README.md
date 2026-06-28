# annotation_multiple_line_prettier_divergence

Multiple line comments between a destructuring binding's `:` annotation and a
multi-member **union** type (`}: // c1\n\t// c2\n\tA | B = init`).

**tsv**: keeps the first comment trailing the `:`; the remaining comments and the
union type indent one level:

```
}: // union one
	// union two
	A | B = init1;
```

**Prettier 3.9**: relocates every comment to its own line after the `:`:

```
}:
	// union one
	// union two
	A | B = init1;
```

Both forms are stable under their respective formatters; each comment stays a
distinct node (none merge) in both.

## Reason

The multi-comment counterpart of
[annotation](../annotation_prettier_divergence/). Per Comment Position
Philosophy, tsv keeps the first comment trailing the `:` (the uniform
forced-continuation indent), while prettier 3.9 drops all of the comments onto
their own lines. The fixture also covers three leading comments and a block
comment followed by a line comment (`/* lead */ // trail`), which prettier 3.9
likewise pushes to its own line.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
