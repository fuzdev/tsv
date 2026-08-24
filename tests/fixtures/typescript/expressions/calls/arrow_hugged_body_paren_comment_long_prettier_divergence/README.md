# Hugged arrow argument, body paren comment — width-reached states

The sibling
[arrow_hugged_body_paren_comment](../arrow_hugged_body_paren_comment_prettier_divergence/)
pins the hug states a short line already selects. Several more are reachable **only once a
line crosses the print width**, so they get their own boundary fixture: the chain's
forced-expansion argument builder — which a head call carrying its own arguments selects,
not the `obj.a().b().m(…)` shape — the expand-last state of a multi-argument call, and the
**object/array-terminal** state of that same expand-last layout, in both the plain-call and
the member-chain spelling.

Each case is a 100/101 pair. At **100** the arguments stay inline and the whole arrow is
printed, so the authored parens and their comment ride along for free. At **101** the
layout changes, and which way it changes is what each row pins.

For a **call** or **ternary** body the argument list expands, because the layout there
reassembles the argument from a signature doc and a body doc and so skips the arrow's
body-end→arrow-end gap — the region where the authored `)` and any comment before it live.
The **object/array** terminal is the opposite: its states render the argument's own
expand-last printing, which emits that gap, so the head arguments stay inline and only the
terminal expands. Those rows exist because refusing there too is
visible twice over — the single-argument spelling of the same layout hugs while the
multi-argument one breaks every argument out, and the broken-out form is not even a fixed
point (the object it leaves written multi-line reads as a source-multiline break on the
next pass, and the inline state wins). `unformatted_ours_compact` carries the one-line
authoring, which is where that shows: the hug's own output is multi-line either way.

tsv keeps the parens so the comment stays where the author wrote it. Prettier strips them
for a call body, and for the **object** body — where the parens are grammar-**required**
rather than authored — keeps the parens but moves the comment outside them
(`(x) => ({ k: x }) /* c */`), re-associating it from the object to the whole body. So the
object rows differ in the comment's position alone; the layout agrees.
Prettier is also non-idempotent on its own output for the 101 chain-builder object row: a
second pass expands the argument onto its own line, so `audit_signature.txt` pins the chain.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Arrow body stripped parens, hugged call argument) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
