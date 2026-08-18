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

**tsv**: each comment keeps its own line, in the authored order.

**prettier**: not one answer. For the chain cases it breaks the chain and keeps both
comments (a layout-only difference — it prints the member at the statement's indent
where tsv indents the continuation); at the type shell it **welds** (`(B & C) // c5 //
c6`), losing `// c6`; at the block tail it agrees with tsv exactly. It also needs two
passes to settle — pass 1 additionally breaks the intersection across its `&` — so the
chain is pinned by `audit_signature_welded.txt` rather than a single-form marker.

`input.svelte` is a fixed point for **both** formatters, and so is prettier's own
welded landing; the divergence is entirely in how the authored form normalizes, which
is what `unformatted_ours_welded.svelte` states.

## Known bound: a switch's last case

The separator breaks at the indent of the flush's own line break, which is the
document's answer for "what indent starts the next line here" — the value a reformat
then agrees with at every container tail (block, method, object, and a case followed
by a sibling case). The **last** case of a switch is the one shape where it is not:
there the next break is the switch's `}`, one level out from where a dangling comment
in a case settles, so `h() // c⏎; // t` inside a final case reaches its fixed point on
the second pass rather than the first. A bracketed type list's closer after a deferred
item run is the same bound from the other side — the run separated behind the last
item's own `//` (`Foo<⏎A,⏎a | b // c⏎// inj⏎>`) breaks at the `>`'s indent, one level
out from where the own-line comment settles on pass 2. Neither shape is fixturable
while that is true, and both are left out of `input.svelte` deliberately — the
alternative at those sites is the weld, which is content loss rather than a position
that settles.

Reason: comment position preserved over prettier's merge, and print-once over the
weld. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
