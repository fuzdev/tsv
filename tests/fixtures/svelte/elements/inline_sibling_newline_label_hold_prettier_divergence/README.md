# inline_sibling_newline_label_hold_prettier_divergence

The sibling-newline flow rule's **prose gate**: a run holding a single word is a label, not
prose, so the authored newlines beside it hold. tsv and prettier agree on every label shape; the
divergence is the flowing controls, where tsv converges the newline and space authorings and
prettier keeps each.

## Reason

Design choice — the boundary of the authoring-independence
[inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/) argues for.
Flowing means reflowing a run per width into a text `fill`, and a fill needs a phrase: a run is
prose when **one of its text nodes carries two words**. One word beside an element, a void
element, a component or a tag has nothing to reflow, so its newlines are the author's structure —
an icon and its caption, a field and its unit, a label and its value — and are held exactly as a
prose-free run's are. The question is asked of every text node in the **run** (between two
run-bounding siblings), never of the node at the boundary alone: a one-word node that ends a real
sentence flows with the sentence node it belongs to, where a boundary-local test would hold
prettier's own one-word wrap leftovers alone on their lines — the accretion the flow rule exists to
heal. The count is the **most words any one node carries**, never a sum over the run: words in
two different text nodes are separated by a sibling, and that separation is the author's, so two
one-word captions in one run (`text1⏎<input />⏎text2⏎<input />`, an icon-and-caption list) are
two labels and hold — a sum packs every such list the moment it holds two entries. The cost is a
sentence spelled entirely as one-word fragments between siblings, which holds; real prose has a
two-word node somewhere in its run. Two other alternatives were measured and rejected: counting
an expression tag as a word packs a label beside its value (`chars⏎{n}`), and a sentence
heuristic (three words, or `.!?`) holds prettier's own two-word wrap tails.

## Cases

Every label shape is held, in the multiline form both formatters keep: a word before a void
element, and one after; a word before an inline element, and one after; a word between two
components; a word beside a `{@render}` tag and beside an `{@html}` tag; a word before a tag, and
one after; an element, a word and a tag; a word ending a run of tags; a word after a tag (the
separator between the two tags holds with it); a one-word tail after an element whose own content
is prose — that content is the element's own run, so the tail's run holds one word; two
one-word captions in one run, beside void elements and beside tags, and a list of three — the
count is per node, so a run of one-word labels is a list of labels however many it holds; and
the count's own cost, stated: a sentence spelled entirely as one-word fragments between siblings
(`text1⏎<span>inline1</span>⏎text2`) is three labels and holds. Real prose has a two-word node
somewhere in its run — the corpus put one such sentence in 1105 files, and holding it returned
the file to its authored bytes.

## Controls — what still flows

- **The cliff** — a node carrying two words is prose, and the run flows. Two words is where label
  and prose blur, so the boundary is stated on purpose.
- **Run-level, not boundary-local** — `text1 text2 text3 <span>inline1</span> text4`: the
  trailing one-word node ends a sentence, and the run holds it, so it flows with it — and the
  phrase reaches every boundary in its run, however far from it
  (`text1 text2 <span>inline1</span> text3 <span>inline2</span> text4` packs whole).
- **The cliff in a list** — `<Comp1 /> text1 <Comp2 /> text2 text3`: one two-word caption is
  prose at the cliff, and it packs the one-word captions in its run with it. That is the cost of
  any count at the cliff (a per-node ≥3 was measured to hold prettier's own two-word wrap
  tails), stated here so the choice is recorded rather than discovered.

`prettier_variant_newline.svelte` is the isolated authoring of the four controls — prettier keeps
it stable, tsv normalizes it to `input.svelte`; the label shapes are identical in both files,
which is the point.

`variant_space.svelte` is the null control: the rule HOLDS an authored newline, it never FORCES
one, so the space spelling of every label shape stays inline under tsv. This file carries the
shapes prettier keeps inline too — beside an element, a void element, a component, a render/html
tag, or inside a content text — so it is dual-stable. The one space spelling it leaves in its
newline form is a whitespace-only space between two **tags**: tsv keeps that packed as well (the
separator defers to its per-width group as a prose run's does), but prettier's bare `line`
between two tags breaks with the container, so that pair is its own fixture,
[inline_sibling_newline_label_hold_tag_pair_space](../inline_sibling_newline_label_hold_tag_pair_space_prettier_divergence/).
Without the null control an implementation that always broke a one-word run would satisfy every
other file here.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
