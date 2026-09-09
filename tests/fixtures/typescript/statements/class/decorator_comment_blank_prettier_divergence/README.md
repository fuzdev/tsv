# decorator_comment_blank_prettier_divergence

An author blank line ahead of an own-line comment run in a **decorator gap**. tsv keeps it at
every gap, on one rule with no conditions; prettier keeps it at most of them and drops it at
two, because its answer is a side effect of which of three comment-attachment handlers happened
to fire.

- **tsv**: `@fn1⏎⏎// c⏎@fn2⏎q: T` — the blank survives, wherever the run was written
- **Prettier**: drops it in exactly two of the gaps below (`c5`, `c7`), keeps it in the rest

## Reason

The blank is a property of the **comment run**, not of the decorator gap. The null control is
what says so: with no comment in the gap, a blank between two decorators collapses in both
formatters (`c = 1` and `class D` here, whose blank-bearing authoring is
`unformatted_ours_null_control`). So nothing preserves a blank the run does not carry, and the
only question is whether the run carries one.

Prettier answers that question through comment attachment. `printTrailingComment`'s own-line
branch emits `isPreviousLineEmpty ? hardline : ""` ahead of each comment, so the blank survives
exactly where the run attaches as the **preceding decorator's trailing** comments — and three
different handlers decide that:

| where | prettier | why |
| --- | --- | --- |
| class member, both gaps (`c1`–`c3`) | keeps | `isPropertyLikeNode` → trails the decorator |
| parameter property, both gaps (`c4`) | keeps | `TSParameterProperty` is property-like too |
| plain parameter, before the binding (`c8`) | keeps | no following node, so the fallback trails the decorator |
| plain parameter, between decorators (`c5`) | **drops** | the run leads the next Decorator |
| class-level, before the class (`c6`) | keeps | `handleClassComments` trails the last decorator |
| class-level, between decorators (`c7`) | **drops** | the follower is a Decorator, so the run leads it |

Four different verdicts for one authoring, and they do not line up with anything an author can
see: the same `@fn1⏎⏎// c⏎@fn2` keeps its blank on a class member, loses it one gap over on a
plain parameter, keeps it on a parameter property, and loses it at the class level. tsv does not
inherit that table. It prints the run **where the author wrote it** at every one of these gaps —
it implements none of prettier's three relocations — so the blank in front of the run is
authoring signal at every one of them, and the rule is a single unconditional emitter
(`Printer::push_decorator_run_blank`) with no host axis and no follower test.

The divergence is one of **normalization**, not of content: both forms are idempotent in both
formatters, the ASTs are identical, and no comment moves. The `c8` cell is the one the rule
*fixed* — tsv used to drop that blank while prettier kept it, because the old model keyed the
blank on `isPropertyLikeNode` and so missed prettier's no-following-node fallback.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and [comments.md](../../../../../../docs/comments.md) §Trailing and dangling
runs.
