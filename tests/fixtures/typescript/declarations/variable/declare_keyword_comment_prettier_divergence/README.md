# Divergence: `declare`→kind keyword-interior comment (preserve)

A block comment between `declare` and the declaration's kind keyword
(`declare /* c */ const a: number;`), for every kind. tsv keeps it after `declare`; prettier
**relocates** it past the kind keyword onto the binding.

```ts
// tsv (preserve)                  // prettier (relocate past the keyword)
declare /* c */ const a: number;   declare const /* c */ a: number;
```

**Why tsv preserves:** `declare` is an ambient modifier and the kind keyword is the declaration —
two separable things, so the gap between them is a position an author can mean (a comment there
plausibly annotates the ambient-ness). A keyword is not a *pure separator*, the one sanctioned
reason to trail.

Only the **block** form exists: `declare // c⏎const x` ASI-splits into two statements (`declare;`
then `const x: number;`) in both formatters, so there is no line-comment gap to preserve — see
[contextual_keywords/declaration_keyword_own_line](../../../syntax/contextual_keywords/declaration_keyword_own_line/).

The rule is not the variable kinds': **every** head `declare` takes answers the same way, pinned
together in
[typescript_specific/declare/keyword_gap_comment](../../../typescript_specific/declare/keyword_gap_comment_prettier_divergence/).
This fixture keeps the variable-specific coverage the family one does not — `let`/`var`, the
`export` prefix, and a multi-declarator list. The **keyword→name** gap a step later
(`declare function /* c */ f()`) is a different position and preserves in both formatters
([declarations/function/declare_keyword_comment](../../function/declare_keyword_comment/)).

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
