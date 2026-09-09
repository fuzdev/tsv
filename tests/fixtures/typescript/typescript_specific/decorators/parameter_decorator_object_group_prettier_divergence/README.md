# parameter_decorator_object_group_prettier_divergence

A **decorated** sole parameter declines the hug in both formatters, but prettier still prints
its object-type annotation group-less, so the annotation expands whenever the decorator's own
group breaks. tsv gives the annotation a group of its own, and it stays flat while it fits.

- **tsv**: `@dec⏎a: { aaa: Aaaa }` — the annotation keeps the author's flat form
- **Prettier**: `@dec⏎a: {⏎\taaa: Aaaa;⏎}` — the annotation expands with the decorator's break

Both spellings of the decorator's own line are covered: the **authored** one (a newline after
the decorator, prettier's `hasNewlineBetweenOrAfterDecorators`) and the **width** one (the
decorator and its binding do not fit together on the broken list's line).

The `authoredObject` cell reads the dropped group in the **other direction**: an annotation the
author expanded. There prettier's collapse is *authoring-dependent* and tsv's preservation is
not. With the decorator authored on its own line the decorator group breaks, the group-less
annotation inherits that break, and both formatters print the expansion — that is the cell in
`input`. Move the decorator back onto the binding's line and prettier collapses the whole
parameter to `authoredObject(@dec a: { aaa: Aaaa }) {}`, discarding an expansion the author
wrote, while tsv keeps it and lets its break carry the decorator back to its own line. That
flat authoring is `unformatted_ours_compact` (which also carries the flat authoring of the
`width` and `pattern` cells) and prettier's dual-stable answer to it is
`variant_inline_decorator_collapse`.

Two controls stay here: a decorated **pattern** (prettier's `printObject` excludes a decorated
`ObjectPattern` from the group drop, so both formatters keep it flat) and a **second
parameter** (which takes the hug out of the question for both).

## Reason

The same dropped group as
[sole_param_hug_authored_expansion](../../../declarations/function/sole_param_hug_authored_expansion_prettier_divergence/),
read at the other end. `printObject` (`print/object.js`) returns bare `content` for an object
type under a `typeAnnotation → typeAnnotation → shouldHugTheOnlyParameter` path match, and
`shouldHugTheOnlyParameter` never asks about decorators — where the *sibling* `ObjectPattern`
arm one line above does (`!isNonEmptyArray(node.decorators)`). So in the one position where the
hug is declined *because of* a decorator, prettier still drops the group that the hug was the
whole justification for, and the annotation inherits a break decision it has no reason to
share. The result **fabricates** an expansion the author did not write.

tsv routes a decorated parameter through its ordinary builder, where the annotation keeps its
own group: the hug's group-less arm (`build_hugged_literal_param_doc`) is reached only when the
parameter actually hugs.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Hugged sole-parameter object type.
