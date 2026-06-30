# media_grouped_feature_case_prettier_divergence

A media-feature **name** is ASCII case-insensitive, and tsv lowercases it to match
prettier (`@media (MIN-WIDTH: 1px)` → `(min-width: 1px)`; covered by the regular
`media_feature_case` fixture). But that only applies to a **simple** feature
expression — a `(...)` with no nested `(`.

For a **grouped** condition (Media Queries 4 §"grouping" — a `(...)` that itself
contains nested feature expressions, like `((MIN-WIDTH: 1px) and (MAX-WIDTH: 2px))`),
the two formatters diverge:

- **tsv** leaves the whole grouped condition **verbatim** — both feature names keep
  their case.
- **prettier** lowercases the **first** nested feature only (`MIN-WIDTH` → `min-width`)
  and preserves the rest (`MAX-WIDTH` stays). This is an artifact of prettier's
  media-query parser, which only partially descends into a grouped condition (the
  remainder parses as `media-unknown`).

So `output_prettier.svelte` lowercases one of the two; tsv (the `input`) preserves both.

## Reason

**Parser compat / consistency.** Replicating prettier's "lowercase the first nested
feature but not the others" behavior would mean matching a parser inconsistency.
tsv instead treats a grouped condition as an opaque unit and leaves it verbatim, so
its two nested features are handled identically. Grouped media conditions are rare
(MQ4 grouping syntax) and an uppercase feature name inside one rarer still. See
[conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules)
(`Media grouped feature case`, Parser compat).
