# last_arg_arrow_comment_convergence_long_prettier_divergence

A member chain whose last call's last argument is an arrow with one single-line block comment
glued to `=>` and a newline after it, where the chain reaches the two-state group (its head does
not merge, or it sits in a value position). Both formatters share one fixed point — the chain
flat, the comment on its own line under `=>` — but prettier needs 2 passes to reach it from the
glued authoring; tsv converges in one.

## Pattern

Source: `const v1 = a.b().fnName((p) => /* c */⏎\tx ? y : z);`

- **Prettier pass 1** (unstable): a block comment with no newline BEFORE it is separated from
  the body by a soft line, so the arrow carries no forced break; the chain's one-line
  measurement reads the whole flat content, overflows, and the chain expands — the comment,
  unfittable on the expanded line, breaks onto its own line inside it.
- **Prettier pass 2** (stable): the comment now has a newline before it, so its separator is a
  hard line; the one-line measurement truncates there, sees only the head up to `=>`, which
  fits, and the chain collapses back to flat with the comment on its own line.

tsv prints the pass-2 form directly — in a declarator, an assignment, a `return`, a property
value, a call argument, a statement whose head does not merge, and a template expression — for
a body that hugs the call's parentheses (a conditional, a call) and for one that breaks the
arguments out (a binary, a sequence, an identifier), keeping a blank line the author left after
the comment. `unformatted_ours_comment_ends_arrow_line` is the
glued authoring (ours normalizes in one pass; prettier's first pass is pinned by
`prettier_intermediate_comment_ends_arrow_line`).

The window is exact and the fixture pins both of its boundaries: when the arrow fits flat on the
expanded chain's line that carries it — the broken-out argument line for a body that breaks the
arguments out, the last call's own line for one that hugs its parentheses (each a 100-char
case) — the broken chain keeping the comment inline is the shared stable form; one char wider
(101) and the chain stays flat. Controls outside it: a short-head statement merges
its head, so the chain never reaches the two-state group and both formatters print the comment
own-line in one pass; a second comment anywhere in the arguments — in the `=>` run, ahead of the
first argument, or trailing the body — keeps the first comment's soft separator in both passes,
so both keep the expanded chain; and an earlier call taking a function argument keeps the chain
expanded in prettier's second pass too.

## Reason

Prettier bug (non-idempotent): from the glued authoring prettier's first pass is not a fixed
point. tsv prints prettier's own settled form in one pass — a convergence-speed divergence with
a single authoring-independent fixed point.

See [conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript)
(Member-chain arrow-comment convergence) and
[conformance_prettier_ts_comments.md](../../../../../../../docs/conformance_prettier_ts_comments.md).
