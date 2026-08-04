# specifier_after_comma_block_then_line_prettier_divergence

An import/export specifier with a block comment **after** the comma plus a line comment
after it (`a1, /* c1 */ // c2`). tsv keeps the block on the comma line where the author
wrote it, in front of the line comment; prettier relocates the block to **before** the
comma.

```
// tsv                          // prettier
import {                        import {
	a1, /* c1 */ // c2              a1 /* c1 */, // c2
	b1                              b1
} from './a';                   } from './a';
```

## Reason

tsv treats comment placement as intentional (see Comment Position Philosophy), and here
the placement is also the only one that keeps the run in **source order**: the line
comment defers through `line_suffix`, so a block left to lead the next specifier would
render after it and the authored pair would come back reversed on two lines.

A block with **no** line comment after it has nothing to defer behind and keeps leading
the next specifier (`a4`), where both formatters agree
([specifier_comma_comment](../specifier_comma_comment/)). A block on the *before*-comma
side stays there in both
([specifier_multiline_comma_comment](../specifier_multiline_comma_comment/)).

The specifier list shares the object literal's element-comma emitter, so the two answer
this identically — see
[objects/after_comma_block_then_line](../../../expressions/objects/after_comma_block_then_line_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
