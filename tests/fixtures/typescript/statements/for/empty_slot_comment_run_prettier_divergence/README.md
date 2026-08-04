# empty_slot_comment_run_prettier_divergence

Two or more comments written in the **same empty clause slot** of a C-style `for`
header all stay in that slot, in order, separated by the slot's own `line` — a space
while the header fits, a fresh line once it breaks. Prettier moves the whole run out,
exactly as it moves a single comment: into the next clause across the `;`, or before
the preceding `;`.

tsv: keeps the run in the slot the author wrote it in
Prettier: relocates the run to the neighbouring clause

## Reason

The single-comment case and its reason are
[empty_slot_comment](../empty_slot_comment_prettier_divergence/); this fixture pins
that a *run* behaves the same, and what separates its members. A run is where the
separator becomes observable — with one comment there is nothing between, so the slot
could hold a run glued into one blob without any fixture noticing.

The separator is the slot's, not the comments' own: a line comment in the slot forces
the header open and each comment takes a line, while blocks in a fitting header stay
on the header line.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
