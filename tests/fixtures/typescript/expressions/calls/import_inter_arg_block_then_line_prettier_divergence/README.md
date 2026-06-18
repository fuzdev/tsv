# import_inter_arg_block_then_line_prettier_divergence

A dynamic `import()`'s inter-argument gap holds a block comment **after the comma**
followed by a same-line line comment (`import('x', /* b */ // l⏎ opts)`). tsv keeps the
block where the author wrote it — on the comma line, after the comma; prettier relocates
the block **before** the comma (the line comment stays after it, since a `//` moved ahead
of the comma would comment out the comma).

```
// input (tsv preserves)        // prettier (relocate block)
import(                         import(
	'aaaa…', /* b */ // l           'aaaa…' /* b */, // l
	bbbb…                           bbbb…
);                              );
```

This is the dynamic-`import()` instance of the call-argument
[after-comma block + same-line line comment](../nonlast_arg_after_comma_block_then_line_prettier_divergence/)
divergence — `import()` shares the same comment-position rules across every argument path.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
