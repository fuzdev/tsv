# relational_chain_type_arg_parens_kept_shell_prettier_divergence

The relational-chain pair reaches the `<` operand whose own paren shell the printer KEEPS: prettier prints the chain bare, tsv keeps a pair around the `<` operand.

tsv: `(x < (a = b)) > c`
Prettier: `x < (a = b) > c`, whose own second pass throws `'=>' expected` on the `(t, u)` follower

## Reason

**Semantic preservation.** A shell the printer **strips** is not part of the region's head — `a < (arr[b - 1]) > c` is graded as the arithmetic its paren-free twin is. A shell the printer **keeps** is the opposite case: the `(` is in the printed bytes, so the region a re-read grades opens on it, and a `(`-headed region is a type-argument list to any parser that grades bracket matching and the follow token rather than the body. Two triggers reach it, and neither is an over-rejection of anything an author wrote — each is a document the formatter itself prints:

- **A follower that commits with no break at all.** `x < (a = b) > (t, u)` and `` x < (a = b) > `t` `` are rejected flat, by tsv's own parser (`Expected ')', found '='`) and by prettier's (`'=>' expected.`) — `k14` and `k15`.
- **A width at which the `>` ends a line.** Any follower does it there; the same break the plain chain's width half turns on, in [relational_chain_type_arg_parens_long](../relational_chain_type_arg_parens_long_prettier_divergence/), which states where that boundary sits and why no input cell can carry it.

tsv keeps a pair around the `>`'s left operand — the same tree in the spelling every parser reads alike — and both synthesizes and retains it, on the same **layout-blind** rule the rest of the family follows: whether the `>` ends a line is not knowable where parens are decided, so the pair stands at every width, including the ones that never break.

## What the class is

Every operand whose shell survives printing and whose content is no type: assignment and compound assignment (`k1`, `k2`), a conditional and the three logical operators (`k3`–`k6`), equality and `^` (`k7`, `k8`), `in` / `instanceof` (`k9`, `k10`), `await` and `yield` (`k11`, `k19`), and `as` / `satisfies` (`k12`, `k13`). The first ten keep the shell because they bind looser than a relational operator; `await` and `yield` keep a clarity pair both formatters synthesize from the bare authoring; `as` and `satisfies` end on a type rather than on an expression.

The shell is printed from three places, and the rule counts all three: the pair the operand's own position derives (every cell above), a **JSDoc cast**, whose parens are semantically required and so printed by the cast's own doc rather than by that position (`k18` — the head scan steps over the comment to reach the `(`), and a **sequence**, which supplies its own grouping pair (`k16`). A kept shell whose content happens to **spell** a type is not a new question — a sequence spells the argument separator and `() =>` a function type, so the region-keyed reading commits on the content and the pair stands for the reason it stands everywhere else (`k16`, `k17`, which carry the pair with or without this rule).

## How far down the spine it reaches

The head is the region's **first printed byte**, so a `(` the leftmost printed spine inherits opens it too (`k20`–`k25`). How far the rule reaches is a grammar fact: a `(` opens a *type-argument* region only if the type grammar carries on past its matching `)`, and the only postfix a parenthesized type takes is `[`…`]` — so it descends the object of a plain **computed** member, and a run of them, and nothing else. Every other spine hop puts a token after the `)` that continues no type, and those cells stay bare: a `.` (`k26`), a `!` (`k27`), a call's `(` (`k28`), an operator (`k29`) and an optional member's `?` (`k30`). A hop that could put a type *separator* there instead (`,`, `|`, `&`, a nested `<`) is unreachable without the operand root taking a pair first, since each binds looser than `<` and a sequence supplies its own.

The width half of the class — where the break after the `>` is what commits the region — is [relational_chain_type_arg_parens_kept_shell_long](../relational_chain_type_arg_parens_kept_shell_long_prettier_divergence/), which measures where the loss begins.

`output_prettier.svelte` carries no `audit_signature.txt`: prettier throws on its own pass-1 output, so there is no chain to pin (rule F4b tolerates an erroring pass).

## Variants

`unformatted_ours_bare_chain` drops the outer pair — the authoring the divergence is about, and the one `deno task paren:audit`'s `< > chain` class synthesizes. tsv normalizes it back to `input.svelte`; prettier keeps it bare. It carries every cell but `k14` and `k15`, whose bare spelling is one tsv's own parser rejects (the `Parse`-side head over-rejection the catalog entry ends on, an input an author may write and acorn and tsc both read as the comparison chain), so no variant can claim a normalization from it.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
