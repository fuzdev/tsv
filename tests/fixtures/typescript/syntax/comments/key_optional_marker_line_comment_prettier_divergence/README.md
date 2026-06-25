# key_optional_marker_line_comment_prettier_divergence

A **line** comment in the gap between a member's key and its optional `?` marker
(`a // c⏎?: number`). tsv keeps the comment after the key; because a line comment
must end its line, the `?` marker (and the `: type` / `(params)` the printer
appends after it) drops to a continuation line **indented one level** (the uniform
forced-continuation indent — it reads as part of the member, not a sibling).
Prettier relocates the comment to trail the member's `;` (`a?: number; // c`).
That relocation is **lossy when a second comment already trails the member**
(`g // c5⏎?: number; // c6`): prettier merges both onto one line
(`g?: number; // c5 // c6`, the second `//` becoming text); tsv keeps them distinct.

```ts
// tsv (preserve + continuation indent)   // prettier (relocate after `;`)
interface I {                             interface I {
	a // c                                    a?: number; // c
		?: number;                        }
}
```

The key→`?` face of the optional-marker comment family (the `?`→`:` gap is the
sibling [optional_marker_line_comment](../optional_marker_line_comment_prettier_divergence/)).
Shared across property signatures (interface, type-literal), class properties, and
method signatures via `push_modifier_marker_doc`. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
