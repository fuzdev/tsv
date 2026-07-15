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
The `declare`→name gap of `function`/`class`/`enum` already preserves in both formatters
([declarations/function/declare_keyword_comment](../../function/declare_keyword_comment/)).

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
