# text_form_feed_prettier_divergence

A **form feed** (U+000C) is rendered content, not collapsible whitespace, so tsv preserves it
verbatim wherever it appears in template text. Prettier rewrites every one of them to a plain
space.

## Reason

CSS white-space processing "affects only the document white space characters: spaces (U+0020),
tabs (U+0009), and segment breaks" (CSS Text 3 §White Space Processing), and in HTML the segment
break is U+000A — the DOM normalizes CR/CRLF away, and U+000C is not in the set at all. So a form
feed survives collapsing and reaches the rendered text.

Svelte's compiler implements exactly that: `clean_nodes` classifies whitespace-only text with
`regex_not_whitespace` = `/[^ \t\r\n]/`, which does **not** include `\f`, and the trim/collapse
replacements (`regex_starts_with_whitespaces` / `regex_ends_with_whitespaces`) use the same set —
so `<span>␌<code>a</code>␌</span>` compiles with both form feeds intact. (Two of Svelte's other
whitespace regexes do include `\f`; the ones `clean_nodes` uses do not.)

prettier-plugin-svelte instead splits on `[\t\n\f\r ]`, so it treats a form feed as collapsible and
prints a space in its place — a content change: the compiled component's text differs. Because both
formatters shared that wider class, no corpus diff against prettier could surface it; the oracle
here is the compiler, not prettier. See
[conformance_prettier.md §Whitespace: Form feed](../../../../../docs/conformance_prettier.md#whitespace-form-feed)
and [conformance_svelte.md §Template Whitespace](../../../../../docs/conformance_svelte.md#template-whitespace-clean_nodes).

## Cases

One case per position where ASCII whitespace would be rewritten or removed, so each pins that the
form feed is exempt from that rule rather than from whitespace handling generally:

- **content boundary** — a render-free boundary run is trimmed whole; a form feed there is content,
  so it stays (prettier keeps a space, its own boundary-preservation divergence compounding the
  rewrite).
- **inter-sibling separator** — a whitespace run between two siblings collapses to one space; a form
  feed is not part of that run and is emitted as-is.
- **sole content** — a whitespace-only element collapses to `<span></span>`; an element holding a
  form feed is not whitespace-only.
- **inside prose** — a space run inside text collapses to one space; the form feed is a word
  character to that collapse.
- **boundary run around it** — the ASCII half of a boundary run still trims. The trim stops at the
  form feed, exactly as it stops at an NBSP, which is what "it is content" means operationally.
- **root-level text** — the root fragment's edges trim like any other fragment's, and the interior
  form feed is untouched.

## Related

- [text_non_breaking_whitespace](../text_non_breaking_whitespace_prettier_divergence/) — the other
  character class that looks like whitespace and is content; prettier agrees there, since NBSP is
  outside its `[\t\n\f\r ]` set too
- [inline_boundary_whitespace](../inline_boundary_whitespace_prettier_divergence/) — the boundary
  trim these cases are measured against
