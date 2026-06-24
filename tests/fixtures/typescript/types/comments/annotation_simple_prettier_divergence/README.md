# annotation_simple_prettier_divergence

A line comment between `:` and an inline-renderable type annotation in a
**property signature** — the context where prettier **relocates** the comment.
"Simple" here refers to the **layout** path (the non-union, non-intersection
branch of `build_type_annotation_doc`), not to identifier-only types — the same
divergence applies to optional, readonly, computed keys, and generics. In each
case prettier moves the comment past the implicit `;` to end-of-line; tsv keeps
it after the `:` and drops the type to a continuation line **indented one level**
(the uniform forced-continuation indent).

- tsv: `prop: // c\n\tX;` — comment stays after `:`, type on the next line,
  indented one level
- Prettier: `prop: X; // c` — comment moves past the `;` to end-of-line

Both formats are stable under their respective formatters.

This fixture is the **relocation** face of the rule. The other faces:

- prettier keeps the type **flush** (variable / class-prop / fn-param / return /
  intersection) — tsv still indents → see
  [annotation_continuation_indent](../annotation_continuation_indent_prettier_divergence/).
- prettier also **indents** (multi-member union) — a **match**, not a divergence
  — see the non-divergent [annotation](../annotation/) fixture for the boundary.

## Reason

**Preserves distinct comments.** Prettier's end-of-line motion is
information-destructive when more than one comment touches the property.
This fixture captures three families:

- Case `f` — leading line + trailing line:
  `f: // leading\n  X; // trailing` →
  prettier collapses both onto one line as `f: X; // leading // trailing`.
  Because line comments run to end-of-line, the second `//` becomes part of
  the first comment's text — two distinct user comments merged into one.

- Case `g` — two leading line comments:
  `g: // c1\n  // c2\n  X;` →
  prettier merges **and reverses** order: `g: // c2 // c1\n  X;`.

- Case `h` — leading line + trailing block:
  `h: // leading\n  X; /* trailing */` →
  prettier reorders to `h: X; /* trailing */ // leading` (block before line,
  because the line comment must end the line).

tsv keeps each comment at its authored position as a separate comment node,
preserving identity, order, and kind.

**One uniform rule.** Every `: Type` annotation — property signatures, variable
declarations, class properties, function params/returns, index signatures —
shares the same continuation layout via `build_type_annotation_doc`: the comment
stays after `:`, the type indents one level. Prettier's behavior splits by
context (relocate here, flush elsewhere, indent for unions), but tsv keeps one
layout everywhere. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent and §Comment normalization (stable quirks).
