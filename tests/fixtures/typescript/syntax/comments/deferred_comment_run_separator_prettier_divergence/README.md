# Deferred comment run: the flush's own separator

Two comments deferred to the same line end. A `//` runs to end-of-line, so whatever
the flush emits behind one is **welded into it** — `x; // c1 // c2` reparses as a
single comment whose text happens to contain `// c2`, and the second comment stops
existing. The flush is a comment **run**, and like every other run in the printer it
owes a separator between its members: a comment landing behind a `//` takes a break
first, at the indent of the break the flush is happening at.

This is the renderer-level floor under the build-time rule
[`trailing_member_gap_comment_statement_trailer`](../../../expressions/calls/chained/trailing_member_gap_comment_statement_trailer_prettier_divergence/),
which breaks the chain where a *source* read can see the trailer coming. That read
stops at the first token past the closers, so it cannot see a trailer behind a further
token (`.bar as T; // c2`) or behind a following sibling in a list (`fn(fn1().bar, a);
// c4`) — whether those land on the deferred comment's output line is a layout fact
no build-time read has. The flush knows, because by then the line exists.

Each case here reaches the floor a different way, and the last one is the control:

- `// c1` / `// c2` — a trailer past an `as T` operand.
- `// c3` / `// c4` — a trailer past a following argument.
- `// c5` / `// c6` — a type shell's trailing comment behind the union member's own.
- `// c7` / `// c8` — a block tail, where the separator's indent is the body's and
  nothing about the run is chain- or type-specific.
- `// c9` / `// c10` — a **sequence**'s keep-inside gap. Its builder walks that gap
  unbounded and pushes one `line_suffix` per comment with a *space* separator, so the run
  welds at build time and only this floor separates it. Nothing else here is a sequence,
  and no other fixture reaches the construct, so its reliance on the floor was unpinned.
- `// c` / `// inj` — a bracketed **type list**'s closer. The construct that closes at the
  flush is the `>`, a level out from the item the comment belongs to, so this is the shape
  that fixes the separator's indent: it takes the suffix's own *queued* indent, not the
  closing break's.

**tsv**: each comment keeps its own line, in the authored order.

**prettier**: not one answer. For the chain cases it breaks the chain and keeps both
comments (a layout-only difference — it prints the member at the statement's indent
where tsv indents the continuation); at the type shell it **welds** (`(B & C) // c5 //
c6`), losing `// c6`; at the block tail it agrees with tsv exactly. It also needs two
passes to settle — pass 1 additionally breaks the intersection across its `&` — so the
chain is pinned by `audit_signature_welded.txt` rather than a single-form marker.

`input.svelte` is a fixed point for **tsv**, and prettier's own welded landing is a
fixed point too; for every case but the sequence prettier holds `input.svelte` as well,
so there the divergence is entirely in how the authored form normalizes — which is what
`unformatted_ours_welded.svelte` states. The sequence case is the one where prettier's
answer differs on `input.svelte` itself: it lifts `// c10` **out of the parens** to
follow the `;`, where tsv keeps it inside the construct it was written in, at the
indent it was queued at — [which indent the separator breaks at](#which-indent-the-separator-breaks-at),
below. That is `output_prettier.svelte`.

## Which indent the separator breaks at

The suffix's **own queued indent** — the indent the comment was captured at, inside
whatever construct captured it, which is where a reformat then reads it back. The
alternative is the indent of the break the flush is happening at, and that break belongs
to whatever is *closing*: a `)`, a `}`, a `>`, which can sit a level out from the item the
comment belongs to. The last two cases here are exactly those shapes — a sequence's own
parens and a type list's `>` — and under the closing-break indent both settled one level
out on pass 1 and moved in on pass 2.

Measured across 2,520 targeted two-suffix runs (a gap comment inside a construct plus a
statement trailer, over twelve statement hosts and eight type hosts): the closing-break
indent leaves 22 non-idempotent, the queued indent 2, and the queued indent regresses
none. A **switch's last case** was this rule's recorded counterexample; it is among the
shapes the queued indent settles, and the builder-side answer added for it — the case's
last-statement `;`-line comments defer own-line, dedented to the case's level, so
`h() // c⏎; // t` inside a final case is a one-pass fixed point, pinned in
[last_case_terminator_comment_run](../../../statements/switch/last_case_terminator_comment_run_prettier_divergence/)
— stands on its own and still keeps that shape away from this separator.

Reason: comment position preserved over prettier's merge, and print-once over the
weld. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
