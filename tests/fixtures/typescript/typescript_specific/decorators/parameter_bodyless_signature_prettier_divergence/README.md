# parameter_bodyless_signature_prettier_divergence

Parameter decorators on a **bodyless function signature** — an ambient
`declare function` and an overload signature, the two spellings of a
`TSDeclareFunction` — with a comment in each decorator's own `(`.

The sibling [parameter](../parameter/) fixture covers the class positions, where a
signature is printed by the class-member path. These two are the positions that route
through the *signature* parameter printer instead, and nothing else in the corpus
reaches it with a decorated parameter.

## What the variant pins

`unformatted_ours_decorator_head_comment.svelte` writes each comment in the gap the
author is most likely to leave it in — glued after the decorator's name, before its
`(` — and it must normalize to `input.svelte`, where the comment sits inside the
parens. That is the uniform opening-delimiter rule (see
[comments.md §The delimiter-line question](../../../../../../docs/comments.md#the-delimiter-line-question-one-rule-read-at-three-points)),
and it is the same relocation the value-parameter path already makes.

The variant is the fixture's teeth. A parameter's leading-comment gap must open at the
parameter's **first decorator**, not at its binding: the decorators are printed by the
parameter's own doc, so a gap reaching past the `@` claims a comment that doc will
print again — [comments.md §The five hazards](../../../../../../docs/comments.md#the-five-hazards)
hazard 3, a region lifted out of its container still sitting inside the container's gap.
Both parameters are decorated so the claim covers the list gap as well as the opening
one.

## Why tsv Differs

Prettier's `typescript` parser rejects the input outright — tsc reports TS1206
(`Decorators are not valid here`) for a parameter decorator anywhere but a class
method, and every reproduction of this shape is therefore prettier-rejected:

```
Decorators are not valid here.
```

tsv parses parameter decorators in exactly the positions acorn's
`parseAssignableListItem` reaches — function declarations and expressions, class
methods and the constructor, object-literal methods, and ambient `declare function`s —
deferring TS1206 to the diagnostics layer with its other placement rules (see
[checklist_typescript.md §Decorators](../../../../../../docs/checklist_typescript.md)).
So there is no `output_prettier.*`; `prettier_rejects.txt` pins the first error the
file draws and rule F6 live-verifies that prettier still rejects it.

**Svelte's parser** accepts the whole file, so `expected.json` is the ordinary
canonical AST and this is not a Svelte divergence.

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input),
and the frame's decision rules in
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md).
