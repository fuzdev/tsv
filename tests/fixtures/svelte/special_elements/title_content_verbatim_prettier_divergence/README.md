# title_content_verbatim_prettier_divergence

A `<title>` that is a (transparent) child of `<svelte:head>` parses as a **TitleElement**, and
the compiler pushes its children **verbatim** — both `TitleElement` visitors walk
`node.fragment.nodes` directly (the server one into `$$renderer.title(…)`, the client one into a
`document.title` assignment), so **`clean_nodes` never runs over them**. That puts the kind in
the `<pre>`/`<textarea>` class: the run at each content boundary, the runs around an `{expr}`
tag, and a whitespace-only body are all bytes that reach the page, and tsv prints them as
authored:

```
<title>  text2
  text3  </title>   compiles to `<title>  text2\n  text3  </title>`   ← byte-for-byte
<div>  text5  </div>  compiles to `<div>text5</div>`                  ← control, the edges go
```

Prettier applies the ordinary boundary trim and inter-node collapse anyway
(`<title>text2 text3</title>`).

The bytes are observable, so this is content preservation and not layout taste. Only
`document.title`'s **getter** launders them — HTML's `Document.title` getter strips and collapses
ASCII whitespace, and nothing else in the path does: its setter is a plain string replace,
`HTMLTitleElement.text` returns the child text content unchanged, and `<title>` is deliberately
outside the `pre` / `listing` / `textarea` set whose leading newline the HTML parser drops. The
served markup differs too, and `render_compare` grades the pair **VISIBLE**.

## Cases

- **content boundaries + interior** — `<title>  text2⏎  text3  </title>`: leading run, the
  interior newline and its indent, and the trailing run all stand. The interior line keeps its
  authored column — verbatim content is never reindented to the tag.
- **around an `{expr}` tag** — `<title>  {expr}  text4  </title>` inside a block: the runs
  between the tag and its neighbours are content too, and a block is transparent for the
  head-child classification, so this `<title>` is a `TitleElement` like the direct child.
- **whitespace-only body** — `<title>   </title>`: the body is not empty, so the element never
  reaches the empty-tag layout, and prettier collapses it to `<title></title>`. HTML's content
  model does not sanction this shape (`title` takes "Text that is not inter-element
  whitespace", just as a document takes no more than one `title` element, which the head here
  also exceeds) — but a non-conformant document is not a licence to silently repair it, and
  Svelte's own validator polices neither, rejecting only attributes and non-`Text` /
  non-`ExpressionTag` children.
- **control — a non-`<title>` head child** — `<div>text5</div>` in the same `<svelte:head>`:
  ordinary `clean_nodes` applies and both formatters delete its boundary runs, so the rule is
  the element's, not the head's. `unformatted_ours_div_padding.svelte` pads it and tsv
  normalizes back to `input`.
- **sibling outside the head** — `<p>text1</p>`: without one, the whole component is the head
  and the root-edge trim absorbs the difference, so the render oracle would grade the mangled
  form merely cosmetic.

`unformatted_ours_head_glued.svelte` pins the container half: preserved content carrying a
newline expands its parent, so a glued `<svelte:head><title>` authoring normalizes to `input` —
the same behavior a glued `<div><pre>` already has, which is what the kind inherits by joining
that family. Prettier's answer there is its delimiter dangle (`<svelte:head⏎\t><title>…`), so
that variant is `unformatted_ours_*` rather than a plain one, and
`divergent_variant_head_dangled.svelte` pins that form: prettier holds it stable, and tsv
reproduces its trimmed titles rather than restoring them — once prettier has run, the bytes
are gone and no formatter can put them back.

The positional contrast is the **RegularElement** form — a `<title>` outside `<svelte:head>`,
where `clean_nodes` does run and tsv trims — pinned by
[`elements/title_boundary_whitespace`](../../elements/title_boundary_whitespace_prettier_divergence/);
the head form's positional classification is pinned by [`title_in_head`](../title_in_head/). The
**hoist** is a separate fact that still holds: `clean_nodes` lifts a `TitleElement` out of its
parent fragment, so the run between it and a *sibling* is a fragment edge and is deleted —
[`blocks/hoisted_boundary_convergence`](../../blocks/hoisted_boundary_convergence_prettier_divergence/).
Only the element's own interior is at stake here.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements)
and the general boundary rule this is the exception to,
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
— whose rule 3 (`<pre>`/`<textarea>` are exempt from the compiler's whitespace model) is the one
a head `<title>` joins.
