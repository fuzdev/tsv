# Gap-Injection Audit

> Inject a comment into every gap and re-run the print-once ledger

`gap_audit` is the **discovery** arm of the dropped-comment class. The print-once ledger
([Comment Ledger Audit](audits.md#comment-ledger-audit-commentsaudit)) is the detector, but it only ever sees
a document **as authored** — so a gap no fixture happens to put a comment in is a gap it
never checks. Eight such drops were found by hand, each green on `cargo test`,
`comments:audit`, `roundtrip:audit`, and the corpus diff, purely because no fixture covered
the position. This audit closes that hole mechanically: for each seed file it injects a
comment into **every** candidate gap, one at a time, formats, and runs the ledger over the
result.

Pure Rust, no sidecar. Gated in `deno task check` as a **ratchet**, not a green gate.

**Two detectors ride the one format.** The ledger answers "was a comment dropped or printed
twice?"; the render-time [swallow check](audits.md#line-comment-swallow-audit-swallowaudit)
answers "did a `//` comment eat following content on its output line?" — a class the ledger is
**structurally blind** to, since a swallowing comment is printed exactly once and the
print-once account balances. Arming both on the *same* format call is what makes the second
detector affordable: no extra format, no extra parse. Its findings are held **report-only**
(see [The SWALLOW class](#the-swallow-class)).

**Design rationale lives next to the code** — why sites are byte offsets rather than tokens,
why the ledger (and not an output diff) is the oracle, why the payload set is plural, and
what a green run does *not* prove: see the module docs at the top of
`crates/tsv_debug/src/cli/commands/gap_audit.rs`. This file is the operator's reference.

## Running it

```bash
deno task gaps:audit           # the gate: tests/fixtures, ~17 s
deno task gaps:audit:update    # regenerate the snapshot after fixing a shape

# Directly, against a real codebase — where the real yield is:
cargo run --profile corpus -p tsv_debug --features audits gap_audit ~/dev/zzz/src
```

Build with **`--profile corpus`** (optimized + `panic = "unwind"`). Plain `--release` is
`panic = "abort"`, so a formatter panic would kill the run instead of being caught and
reported as the finding it is.

| flag | effect |
| --- | --- |
| `--json` | machine-readable report on stdout (logs go to stderr) |
| `--jobs N` | worker threads (default: available parallelism) |
| `--limit N` | cap the seed files |
| `--payload <one>` | `block` \| `line` \| `jsdoc_cast` \| `annotation` \| `multiline` |
| `--all-bytes` | also inject strictly inside words — a diagnostic, not a stricter mode (comment interiors stay excluded) |
| `--by-node` | also print the coarse by-`(node, edge)` rollup after the run (report-only; see [Reading a finding](#reading-a-finding)) |
| `--rank` | print the top-N `(node, edge)` clusters as a paste-ready **markdown table** for `TODO_GAPS` §Status (report-only; `deno task gaps:audit:rank`) |
| `--since <baseline.json>` | print the per-cluster ranking **delta** vs a prior `--json` output — "did my slice move the cluster?" (report-only) |
| `--top N` | with `--rank`, how many clusters the table shows (default 12); a `--since` diff always lists every changed cluster |
| `--update` | rewrite the committed snapshot (prints a `# shapes: N` stamp + a RETIRED/RE-PINNED yield line) |

### Full runs vs narrowed runs

The snapshot describes exactly one run: **every payload, at every non-word site, over all of
`tests/fixtures`**. Any flag that changes which shapes a run reaches — `--limit`,
`--payload`, `--all-bytes`, or an explicit path — makes its shape set something other than
what the snapshot means, so:

- **`--update` refuses** a narrowed run outright. It would otherwise pin a subset (or, for
  `--all-bytes`, a superset) and silently unpin real bugs.
- **the ratchet is skipped**, with an explicit `○ ratchet SKIPPED` note. A narrowed run
  reports; it does not grade, and a green one is *not* a passing gate.

`--json`, `--jobs`, `--by-node`, `--rank`, and `--since` change how a run is reported and
scheduled, never which sites it reaches, so they don't narrow it.

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

### The SWALLOW class

A `SWALLOW` shape is **report-only**: neither pinned into the snapshot nor able to fail the
gate. Mechanically it is *filtered out of the graded key set* (`is_graded`) — a third category
beside pinnable and never-pinnable, mirroring `blank_audit`'s `STRUCTURAL-DIVERGENCE`. Making
it *un*-pinnable instead would make it fail like a panic, the opposite of what is wanted.

It is real content loss, so this is a **staging decision, not a verdict**: the class only
became visible when the check was armed here, and pinning several hundred untriaged claims
into a ratchet whose shrinking is the goal would be pinning noise. It reports until its
shapes are triaged; the run prints its own `○ N SWALLOW shape(s) … reported, NOT gated`
section so a quiet `✓` can never be misread as "no swallows".

Two properties differ from the ledger kinds. It is **not self-verified** — a swallow is
observed directly on the rendered output (like `blank_audit`'s F1/reparse kinds), so the
`UNCONFIRMED`/`PARTIAL` axis does not apply; the verify pass's oracle is the multiset of
comment *contents*, which answers the ledger's question and not this one. And it has **no
bystander axis**: the tracker reports a property of an output *line*, not of a registered
comment, so every finding keys at its injection site.

Cost: arming the check adds roughly **+10% CPU** to a run (measured over `tests/fixtures`:
~146 s → ~160 s user, ~17 s → ~19 s wall), against a whole-`deno task check` budget of
~137 s. The rejected alternative — running the full `f1_check` battery per injection — was
measured at **>40x** baseline CPU, because it pays `tsv_parse_to_value` twice per accepted
injection and, unlike `blank_audit`, gap injection has no absorbed-input fast path (an
injected comment must appear in the output, so it is never absorbed).

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
  vanishing because someone added another one nearby. A bystander finding is **keyed and
  reported at the victim's own site** — the emitter that dropped the comment — not at the
  perturbation site the payload went in at (the finding's span, in the formatted input's
  coordinates, is mapped back across the splice to the seed). Its example reads
  `e.g. inject <payload> at <path>:<injection> → drops the comment at :<attribution>`: the
  injection offset reproduces the drop, and `<attribution>` (with the snippet) is where the
  victim comment lived, which is what the shape keys on.
- **`⚠ UNCONFIRMED (0/5 confirmed)` / `⚠ PARTIAL (2/5 confirmed)`** — see below.

### The by-node rollup (`--by-node`)

`--by-node` prints a second, **coarser** view after the run: the finding shapes rolled up onto
their structural key `(node_type, edge)` — the enclosing AST node and the child-role edge each
site's gap sits in (`(CallExpression, arguments→$)`, `(VariableDeclarator, id→init)`), read off
the wire tree. Where the site shape keys a finding by its raw adjacent tokens (the fine ratchet
key), this keys it by the **emitter**: the ~700 shapes fold into a few dozen `(node, edge)`
clusters — each roughly one printer function — ranked worst-first, the burn-down work-list. The
comment-attachment fields the wire mirrors from acorn (`leadingComments` / `trailingComments`)
are **not** treated as structural children, so a gap keys to its emitter edge regardless of
whether a comment happens to sit beside it. A **bystander** finding keys on its victim's site
(the attribution offset), so it rolls up onto the emitter that dropped the comment — not the one
whose gap the payload perturbed.

Each finding is keyed to its own site's `(node, edge)` **at record time** — one wire parse per
seed file, one `node_edge_key` walk per hit — so the per-cluster totals are **exact per-site
tallies**, not an approximation: a generic shape occurring in several structural contexts is
split across its clusters per hit, never attributed wholesale to one. Keying runs only when a
rollup consumer is present (`--by-node` / `--json`), so a plain graded gate run pays nothing for
it. The one residual caveat is the `UNRESOLVED` tail — a finding whose offset keys to no node
(out of range, or a node with no `type`), reported alongside the clusters; over `tests/fixtures`
that tail is empty.

It is **report-only** — it never changes the ratchet grade or the exit code.

`--json` carries the ranked work-list as one additive top-level section, `by_node` — one
`{node, edge, hits, shapes, share, example_shape}` per cluster, hits-descending, what per-slice
tooling reads to ask "did my fix move the cluster?" — plus a top-level `by_node_unresolved`, the
count in the `UNRESOLVED` tail.

### The ranking, productized (`--rank` / `--since`)

The `--by-node` rollup is the raw material; three thin views make it something a session
consumes directly instead of parsing `--json` and hand-transcribing (all report-only —
byte-identical to the gate):

- **`deno task gaps:audit:rank`** (`--rank`, `--top N`) prints the top-N clusters as a
  **paste-ready markdown table** for `TODO_GAPS` §Status — rank, `` `(node, edge)` ``, hits,
  shapes, share — so the fattest-first work-list stays current by paste, not by
  re-transcription (which rots as slices land).
- **`--since <baseline.json>`** diffs this run's ranking against a prior `--json` output and
  prints only the clusters whose hit count **changed** — `(CallExpression, arguments→$) 2861 →
  2790 (−71)`, biggest reduction first — the direct answer to "did my slice move its target
  cluster?". A missing/malformed baseline **warns and skips**; it never fails the gate.
- **`gaps:audit:update`** prints, after the write, a `# shapes: N` count stamp (into the
  snapshot header — the file also carries `#`/blank lines, so a casual `head`/`wc -l`
  over-counts) and a **yield line** — `yield: gaps −R +A (net ±K)` — where `R` is RETIRED (a
  bug this slice fixed, its line gone) and `A` is newly-pinned; the RE-PINNED bulk (the
  unchanged intersection) is silent. It reads the pre-write snapshot to make the RETIRED /
  RE-PINNED split a `git diff --stat` cannot.

### `UNCONFIRMED` / `PARTIAL`

Each shape's kept examples are **self-verified in-run**, because an instrument that only ever
agrees with itself is not evidence. The ledger's finding is checked against something
falsifiable: the multiset of comment **contents** in the injected input vs the format's
output. Each content is whitespace-normalized first (split on newlines, trim each line, rejoin)
so a legitimate re-indent of a multi-line comment (`/* a⏎   b */` → `/* a⏎b */`) normalizes
equal and is *not* a false alarm — while a **mangle** that collapses the newline
(`/* a⏎b */` → `/* ab */`) yields fewer lines, normalizes different, and *is* caught. This
supersedes the earlier `parsed - dropped + double` count comparison, closing both of the
count's blind spots: a balancing drop+duplicate (equal count, unequal contents) and a mangle
(equal count, unequal content).

A shape keeps up to five examples (the smallest by `(path, attribution_offset)` — the
victim's own site for a bystander, the injection site otherwise — so the set is
`--jobs`-independent), and each is re-checked. The ratio is what separates two very different
findings: **`UNCONFIRMED (0/N)`** — *no* example reproduced, so the shape is uniformly an
instrument artifact — versus **`PARTIAL (k/N)`** — some reproduced and some didn't, a *mixed*
real drop. An unlabelled shape confirmed on every example.

Where a finding does *not* reproduce, the output holds the same comment **contents** as its
input, so something printed the comment without recording the emit — a genuine instrument gap,
not the content loss it is filed as. (A mangled rebuild — which the old count read as
UNCONFIRMED, since a mangle keeps the comment *count* — now normalizes different and reproduces
as **CONFIRMED**, the real corruption it is.) The residual, far narrower than the count's: a
multiset can still balance if the *same* content is dropped in one place and duplicated in
another; no corpus example does this.

The ratio is triage information, not a gate signal: it is a property of the shape's sampled
examples, not of the shape, so it is deliberately not part of the ratchet key (and `--update`
regenerates a byte-identical snapshot regardless of it). `--update` still reports the tallies —
how many shapes are fully `UNCONFIRMED` and how many `PARTIAL` — since pinning ~700 claims is
the moment worth naming the ones the audit couldn't reproduce.

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
  a `.svelte` file's `<style>` content is unprobed — so a Svelte fixture containing only a
  `<style>` block yields **zero sites**. That one is held back by **yield, not difficulty**:
  `Style::content_span` names it in a line, but measured over `tests/fixtures` it is +154k
  sites (+20% runtime) for 3 shapes, all `@import`-prelude double-prints. The thinness is
  structural — the ledger registers only *detached* comments, and CSS keeps its in-block
  comments as AST nodes and never lexes a declaration-value comment as a `Comment` at all —
  so extending the ledger (see `comment_ledger`'s TODO) is the honest prerequisite.

Related: [Comment Ledger Audit](audits.md#comment-ledger-audit-commentsaudit) (the detector this drives),
[conformance_prettier.md §Comment Position Philosophy](conformance_prettier.md#comment-position-philosophy).
