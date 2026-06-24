# import_inter_arg_stranded_prettier_divergence

A dynamic `import()`'s inter-argument block comment **stranded** after the comma —
the author put a newline between the comment and the options argument
(`import('x', /* c */⏎ opts)`). tsv respects that newline and keeps the comment where
it was written (trailing the comma line); prettier attaches it to the preceding source
argument and relocates it **before** the comma.

```
// input (author's placement)   // tsv (preserve)        // prettier (relocate)
import(                         import(                  import(
	'aaaa…', /* c */                'aaaa…', /* c */         'aaaa…' /* c */,
	bbbb…                           bbbb…                    bbbb…
);                              );                       );
```

This is the dynamic-`import()` counterpart of the call-argument
[stranded](../nonlast_arg_after_comma_block_stranded_prettier_divergence/) divergence —
`import()` shares the same respect-the-newline rule across every argument path. When the
comment instead **hugs** the options argument (`import('x', /* c */ opts)`, no newline
between them), tsv leads the options with it and both formatters agree — see the
plain-match sibling [import_inter_arg_comment](../import_inter_arg_comment/). The single
rule: *a comment hugging the next arg leads it; a stranded comment stays on the comma
line.*

Reason: Comment relocation. See
[conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
