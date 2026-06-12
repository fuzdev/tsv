# annotation_simple_prettier_divergence

A line comment between `:` and an inline-renderable type annotation in a
property signature. "Simple" here refers to the **layout** path (the
non-union, non-intersection branch of `build_type_annotation_doc`), not to
identifier-only types — the same divergence applies to optional, readonly,
computed keys, and generics. In each case prettier moves the comment past
the implicit `;` to end-of-line; tsv keeps it inline after the `:`.

- tsv: `prop: // c\n X;` — comment stays inline after `:`, type on next line
- Prettier: `prop: X; // c` — comment moves past the `;` to end-of-line

Both formats are stable under their respective formatters.

When the property's type is a multi-member union or intersection,
**both formatters preserve the comment** in place — see the non-divergent
[annotation](../annotation/) fixture for the boundary.

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

**Internally consistent.** Variable declarations (`const e: // c\n X = ...`),
class properties (`class C { prop: // c\n X }`), and property signatures all
keep the line comment between `:` and the type. Prettier moves the comment
past the implicit `;` only for property signatures with an inline-renderable
type — a special case that doesn't apply to variable declarations or class
properties.

Matching Prettier here would require a property-signature-aware special case
in `build_type_annotation_doc`, recreating the inconsistency. Pre-1.0, tsv
keeps the simpler, uniform layout; if real-world corpus pressure shifts the
trade-off, this divergence is a candidate for revisiting.
