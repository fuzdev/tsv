# Gap-Injection Audit

> Inject a comment into every gap and re-run the print-once ledger

`gap_audit` is the **discovery** arm of the dropped-comment class. The print-once ledger
([Comment Ledger Audit](../CLAUDE.md#debug-tooling)) is the detector, but it only ever sees
a document **as authored** — so a gap no fixture happens to put a comment in is a gap it
never checks. Eight such drops were found by hand, each green on `cargo test`,
`comments:audit`, `roundtrip:audit`, and the corpus diff, purely because no fixture covered
the position. This audit closes that hole mechanically: for each seed file it injects a
comment into **every** candidate gap, one at a time, formats, and runs the ledger over the
result.

Pure Rust, no sidecar. Gated in `deno task check` as a **ratchet**, not a green gate.

**Design rationale lives next to the code** — why sites are byte offsets rather than tokens,
why the ledger (and not an output diff) is the oracle, why the payload set is plural, and
what a green run does *not* prove: see the module docs at the top of
`crates/tsv_debug/src/cli/commands/gap_audit.rs`. This file is the operator's reference.

## Running it

```bash
deno task gaps:audit           # the gate: tests/fixtures, ~17 s
deno task gaps:audit:update    # regenerate the snapshot after fixing a shape

# Directly, against a real codebase — where the real yield is:
cargo run --profile corpus -p tsv_debug --features comment_check gap_audit ~/dev/zzz/src
```

Build with **`--profile corpus`** (release + `panic = "unwind"`). Plain `--release` is
`panic = "abort"`, so a formatter panic would kill the run instead of being caught and
reported as the finding it is.

| flag | effect |
| --- | --- |
| `--json` | machine-readable report on stdout (logs go to stderr) |
| `--jobs N` | worker threads (default: available parallelism) |
| `--limit N` | cap the seed files |
| `--payload <one>` | `block` \| `line` \| `jsdoc_cast` \| `annotation` \| `multiline` |
| `--all-bytes` | also inject strictly inside words — a diagnostic, not a stricter mode |
| `--update` | rewrite the committed snapshot |

### Full runs vs narrowed runs

The snapshot describes exactly one run: **every payload, at every non-word site, over all of
`tests/fixtures`**. Any flag that changes which shapes a run reaches — `--limit`,
`--payload`, `--all-bytes`, or an explicit path — makes its shape set something other than
what the snapshot means, so:

- **`--update` refuses** a narrowed run outright. It would otherwise pin a subset (or, for
  `--all-bytes`, a superset) and silently unpin real bugs.
- **the ratchet is skipped**, with an explicit `○ ratchet SKIPPED` note. A narrowed run
  reports; it does not grade, and a green one is *not* a passing gate.

`--json` and `--jobs` change how a run is reported and scheduled, never which sites it
reaches, so they don't narrow it.

Off the default corpus (an explicit path) the snapshot doesn't apply at all — every finding
is news, and any finding exits 1.

## The ratchet

`crates/tsv_debug/src/cli/commands/gap_audit_known.txt` is a **machine-generated** snapshot
of every finding shape `tests/fixtures` currently produces. Unlike `scan_audit`'s
hand-curated `ALLOW`, it carries **no per-entry rationale by design**: at ~700 shapes that is
not a thing a human can keep honest. Every line is a **known bug**, and the file shrinking is
the goal.

```
# Format: KIND<TAB>SHAPE<TAB>PAYLOADS
DROPPED	import⟨⟩.	block
DOUBLE-PRINTED	IDENT⟨⟩=	block,line
```

The gate fails on:

- a shape **not** on the list — a new *kind* of drop, which must not land silently;
- a listed shape that **no longer fires** — a stale entry, so the list can't rot;
- a **panic**, always. A crash is never pinnable (see below).

What it deliberately does **not** pin is **counts**. They churn with every ordinary fixture
PR, and a gate that fails per added fixture would just get turned off. The tradeoff is named:
a new drop at an **existing** shape is invisible.

The **payload set is** part of the key, though. A shape that drops only a `line` comment
today and starts dropping a `block` one tomorrow is a new bug on a new ownership path — keyed
on the shape alone it would land inside an existing entry and never be seen. It is also
stable in the way a count is not: it changes when the bug's character changes, not when a
fixture is added.

### A panic is never pinned

A `PANIC` shape is excluded from the snapshot and always fails the gate. The invariant it
breaks is absolute — a comment in a gap must never crash the formatter — so it is not a
"known bug" to ratchet alongside the drops, and `--update` must not be able to quietly absorb
a crash into the list whose shrinking is the goal. `--update` still writes the drops while a
panic fires, but exits 1 rather than reporting a clean `✓`.

## Reading a finding

Findings dedup by **site shape** — the adjacent tokens with identifiers abstracted
(`import⟨⟩.`, `IDENT⟨⟩=`, `.⟨⟩IDENT`). One bug fires at every site that reaches it, so raw
`(file, offset)` findings would be unreadable and, as a ratchet key, would go stale on the
next fixture edit.

```
     4123×  DROPPED        import⟨⟩.
            37 file(s) · payloads: annotation, block, jsdoc_cast
            e.g. inject block at tests/fixtures/…/input.svelte:412  …⏎import⟨⟩.source(x)…
            comment: "/* c */"
```

- **`e.g. inject <payload> at <path>:<offset>`** is a *triple* — the payload that produced
  **this** example, at this offset, in this file. Re-injecting some other payload of the
  union at that offset need not fire, or even parse.
- **`(N of M hits knock out a bystander)`** is the scarier half: the offending comment is one
  the author already had, knocked out by an injection *elsewhere*. An existing comment
  vanishing because someone added another one nearby.
- **`⚠ UNCONFIRMED`** — see below.

### `UNCONFIRMED`

Each shape's example is **self-verified in-run**, because an instrument that only ever agrees
with itself is not evidence. The ledger is made to predict something falsifiable: if it says
this format drops `d` comments and double-prints `p`, the output must reparse to exactly
`parsed - d + p` comments. Counting via reparse rather than matching the comment's *text* is
what makes it sound — a printer may legitimately re-indent a multi-line comment, so text
matching false-alarms.

A shape whose prediction fails is reported `UNCONFIRMED`, not silently dropped. The output
holds as many comments as its input, so something printed it without recording the emit — or
printed a **mangled** rebuild (`/* a⏎b */` → `/* ab */`, one comment either way). **Real
either way, but not the plain drop it is filed as**, so it wants different triage.

`UNCONFIRMED` is triage information, not a gate signal: it is a property of the shape's one
sampled example, not of the shape, so it is deliberately not part of the ratchet key.

## Triaging and fixing a shape

1. **Reproduce by hand.** Take the example triple verbatim — inject that payload at that
   offset in that file — and format. The report gives you everything needed; nothing else is
   required.
2. **Check it's this class**, not an over-acceptance. tsv's parser is deliberately more
   permissive than the canonical one, so confirm the injected form is something an author
   could actually write (Svelte rejects `<script lang="ts"/* c */>` outright, for instance —
   a comment dropped there is a different bug).
3. **Fix the printer**, fixtures-first per the repo's TDD rule. The fix is usually to route
   the gap through a comment-aware scan rather than concatenating fixed pieces.
4. **Re-pin**: `deno task gaps:audit:update`, and confirm the shape's line is **gone** from
   the snapshot rather than merely changed.

If a shape is genuinely pre-existing and merely newly *reached* by a fixture you added, the
same `gaps:audit:update` is the right move — the bug was always there; the corpus just went
quiet about it until now.

## Scope — what a green run does not prove

Two limits compose, and neither is visible in a `✓`. Both are detailed in the module docs;
the short version:

- **The ledger's scope.** Only **detached** comments count. A Svelte `<!-- … -->` and a CSS
  in-block comment are AST nodes carried by the tree; a CSS declaration's *value* comments are
  never lexed as `Comment`s at all. All outside the model by construction. CSS also has no
  line comments, so the `line` payload is inert in a `.css` file.
- **`code_regions`' reach.** A gap the region walk doesn't name is a gap never probed. Today
  a `.svelte` file's `<style>` content and the non-expression interior of a block tag
  (`{#if ⟨here⟩ a.b}`) are unprobed — so, for example, a Svelte fixture containing only a
  `<style>` block yields **zero sites**.

Related: [Comment Ledger Audit](../CLAUDE.md#debug-tooling) (the detector this drives),
[conformance_prettier.md §Comment Position Philosophy](conformance_prettier.md#comment-position-philosophy).
