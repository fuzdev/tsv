# Post-arrow glued line comment divergence

A `//` the author glued to `=>` keeps that line in tsv; the body drops to the
continuation below it, the same hang the uncommented arm gives. Prettier moves the
comment down to its own line above the body.

```js
// tsv                          // prettier
const a = (e) => // c           const a = (e) =>
	e.prop;                       	// c
                                	e.prop;
```

Neither formatter changes the comment's **association** — it leads the body either
way — so the divergence is the comment's line alone, and nothing else in the layout
moves.

tsv keeps it because this is the `=>` spelling of
[§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent):
a `//` runs to end-of-line, so whatever the author wrote after it cannot stay on that
line, and tsv keeps the comment where it was written and drops the following token to a
continuation. Every sibling gap already answers this way — `new // c⏎\tA()`,
`await // c⏎\tfn()`, `keyof // c⏎\tB`, `: // c⏎\tT`, and the **function-type** arrow's
own `=> // c⏎\tT` — so before this rule reached here the two spellings of `=>`
disagreed: the type-level one kept the glued line, the value-level one did not.

The rule is the **gap's**, not the body's: an object, block or ternary body reads the
same, at both call-argument layouts, and a curried chain's head gap answers it too. A
run the author wrote together (`/* lead1 */ // lead2`) stays together on the `=>` line,
as it does at every sibling gap.

Own-line-ness is authoring signal for a leading position
([§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)),
so the two authorings are **both** stable under tsv and the author picks between them —
the `n` control case. Prettier collapses both to the own-line form.

Out of scope, and deliberately not exercised here: a block body behind a **parameter
list** (`(e) => // c⏎{ … }`) is a different divergence — prettier hugs `=> {` and
relocates the comment *inside* the block, which the `d` case's zero-parameter spelling
avoids. Prettier's answer there is parameter-dependent; tsv's is not.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Post-arrow glued line comment) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
