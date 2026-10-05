# Region the lookahead claims and the type parse rejects — Svelte Divergence

Every line of `input.svelte` parses under acorn-typescript — `expected_svelte.json` is its
tree — and tsv rejects it. Each is a `<` region tsv's type-argument lookahead keeps
**claimed** although tsv has no type to read there, so the type parse fails loudly. The
claim is deliberate: in every group the comparison chain acorn-typescript reads has no
printed form that every parser reads back as the input, or sits under a claim that
guards one.

- **`a` — a paren shell heading the region.** `f<(A)<C>>(x)` is the chain
  `f < (A) < C >> x` to tsc and acorn-typescript alike: a parenthesized type takes no
  argument list. Around a name the printer strips that shell, and the bare
  `f < A < C >> x` is a region whose inner list closes with the outer one on a single
  shift token — a generic call the moment a `(`, a template or a line break follows. The
  pairs the printer has stand at a `>` operator and at a bar, and neither is there; the
  same holds where a `,` follows the list (`a4`) or stands inside it (`a8`), where a later
  `>` closes the region. The claim asks neither what the shell holds nor what follows the
  list, since which contents print as a list-taking name — and which the printer keeps
  shelled for another parser to read its own way — is the printer's paren rules to say: a
  name the printer makes by stripping an inner shell (`a5`, `a6`), a shell it would keep
  (`a7`) and a list a bar follows (`a10`) are rejected with the rest. A name whose `<`
  stands past a line break (`a9`) heads the region the same way — the break ends the type
  for both parsers and the printer folds it; that head being a bare name, a bar behind
  its list ends the claim (`f<B⏎<C> | D>(x)` prints as `(f < B<C>) | (D > x)`). This
  group is a **known over-rejection**, not a sanctioned reading. Behind a bar AHEAD of
  the shell the chain's own pair ends the region first, so `f<A | (B)<C>>(x)` parses and
  prints as `(f < A) | (B < C >> x)`
  ([less_than_operand_type_args](../../../syntax/disambiguation/less_than_operand_type_args/)).
- **`b` — a negative literal's list, or its glued member tail.** acorn-typescript reads a
  negative literal type with its expression parser, which takes an instantiation's list
  behind the digits wherever an instantiation may stand, so `f<-1<C>(e)>(x)` is a generic
  call over the literal type of `-(1<C>(e))`; tsc reads the comparison chain. tsv has
  neither tree — its type parser stops at the `<` — and the chain would print as text
  acorn-typescript reads as the call. Where the list would close into a shift
  (`f<-1<C>>(x)`) no parser reads one, and tsv reads the chain both oracles do. The claim
  holds wherever the literal is graded, an arrow function's body in an index included
  (`b4`). A member tail glued to the digits (`b5`, `b6`) is the same expression parse:
  the literal type of `-(1..x)` to acorn-typescript, a chain to tsc, and a chain that
  would print as `f < -(1).x > x`. So is the spaced postfix — a call's arguments, a
  member, a template — in an index whose body opens as a type (`b7`–`b9`): on a name a
  bar or a `[` follows, or on the literal itself ahead of a `.`. In an index that opens
  any other way, and at the list's own level, the same postfix is the comparison chain
  tsc reads (`f<A[-1(e)]>(x)`, `f<-1(e)>(x)`).
- **`c` — an import head that is no import type.** tsc's import type takes any specifier
  and leaves the string rule to its checker, so `f<import(c)>(x)` is a generic call to the
  compiler and a chain over a dynamic import to acorn-typescript; `import.meta` is claimed
  the same way and rejected by the compiler itself.
- **`d` — a second list behind a nested one.** acorn-typescript reads `A<B>` as an
  instantiation instantiated again; tsc rejects `f<A<B><C>>(x)` and reads `d2` as a chain
  through a type assertion. The pair the printed chain takes around `f < A<B>` would move
  the compiler's reading of both. Behind a keyword type's own list heading the region
  (`d3`) the instantiation prints as `(string<C>)<D>`, a paren shell heading the region
  — group `a`'s claim. Inside an index (`d4`) the second list is claimed where an operand
  follows it for the compiler's assertion to take, behind a shell there too (`d5`).
- **`e` — an import type's list past a line break.** acorn-typescript takes the list
  across the break and reads a generic call; tsc ends the type there and reads a chain.
  tsv's type parser holds tsc's line rule
  ([import_type_line_break](../../../types/type_args/import_type_line_break_svelte_divergence/)),
  so the claimed region is rejected.
- **`f` — an arrow function's parameter default, and a meta-property, in an index that
  opens as a type.** tsc reads `(a = 1) => 0` as a function type and leaves the default to
  its checker, where acorn-typescript reads the chain over an arrow function (`f1`);
  acorn-typescript reads `new.target` as a type reference by that name, and tsc rejects the
  line (`f2`). Like group `b`'s postfix, each is the comparison chain in an index that
  opens any other way (`f<A[(a = 1) => 0]>(x)`, `f<A[() => new.target]>(x)`).

`tsv_rejects.txt` pins tsv's own error on the first line; every line is pinned on its own
by `claimed_regions_stay_rejected` in
[tests/index_type_args_follow.rs](../../../../../index_type_args_follow.rs).

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject), and with no accepted
parse there is nothing for a formatter to claim — hence no `expected.json` and no
format-claim siblings.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (a region the lookahead claims and the type parse rejects).
