# scope_to_case_prettier_divergence

`@scope (.a) to (.b)`'s `to` scope-limit keyword is a case-insensitive CSS
grammar keyword, so `@scope (.card) TO (.footer)` is valid.

**tsv** accepts the uppercase form and canonicalizes `to` to lowercase;
**prettier** accepts it too but preserves the author's case. So
`prettier_variant_uppercase` is the form prettier keeps stable that tsv folds to
the lowercase `input`. (tsv previously rejected the uppercase `TO` — a
correctness bug; accepting it is the fix.)

## Reason

**Design choice.** `to` is a **delimiter** keyword — it marks the scope limit, a
position marker, not a boolean operator. tsv canonicalizes delimiter keywords to
lowercase, exactly as it lowercases the keyframe `from`/`to` selectors (which
prettier also lowercases). Prettier is inconsistent here: it lowercases keyframe
`from`/`to` but **preserves** `@scope`'s `TO`. tsv lowercases both, uniformly.

This is tsv's only deliberate keyword-case *canonicalization* divergence from
prettier: every case-insensitive keyword tsv lowercases matches prettier except
this `to`. The boolean **operators** `and`/`or`/`not` are instead *preserved*
(their case is a logical-skeleton readability signal), matching prettier; `to`
carries no such signal, so it's canonicalized. (Separately,
`media_grouped_feature_case` diverges on a feature *name*'s case, but that's a
parser-compat consequence of leaving grouped conditions opaque, not a
canonicalization choice.) See
[conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules)
(`@scope to keyword case`, Design choice) for the full canonicalize/preserve line.
