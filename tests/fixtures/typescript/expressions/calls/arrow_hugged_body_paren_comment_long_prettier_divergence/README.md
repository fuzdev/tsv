# Hugged arrow argument, body paren comment — width-reached states

The sibling
[arrow_hugged_body_paren_comment](../arrow_hugged_body_paren_comment_prettier_divergence/)
pins the hug states a short line already selects. Two more are reachable **only once a
line crosses the print width**, so they get their own boundary fixture: the chain's
forced-expansion argument builder — which a head call carrying its own arguments selects,
not the `obj.a().b().m(…)` shape — and the expand-last state of a multi-argument call.

Each case is a 100/101 pair. At **100** the arguments stay inline and the whole arrow is
printed, so the authored parens and their comment ride along for free. At **101** the
argument list expands and the layout reassembles the argument from a signature doc and a
body doc, skipping the arrow's body-end→arrow-end gap — the region where the authored `)`
and any comment before it live. Both halves are here because only the 101 side exercises
the reassembly; the 100 side is the control that proves the width, not the comment, is
what selects it.

tsv keeps the parens so the comment stays where the author wrote it. Prettier strips them
for a call body, and for the **object** body — where the parens are grammar-**required**
rather than authored — keeps the parens but moves the comment outside them
(`(x) => ({ k: x }) /* c */`), re-associating it from the object to the whole body.
Prettier is also non-idempotent on its own output for the 101 object row: a second pass
expands the object's properties, so `audit_signature.txt` pins the chain.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Arrow body stripped parens, hugged call argument) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
