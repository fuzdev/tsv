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
detector affordable: no extra format, no extra parse. Both detectors' findings are ratcheted
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
| `--report` | per-shape detail: every pinned shape with kind, count, payloads, and a reproducer |
| `--jobs N` | worker threads (default: available parallelism) |
| `--limit N` | cap the seed files |
| `--payload <one>` | `block` \| `line` \| `jsdoc_cast` \| `annotation` \| `multiline` |
| `--all-bytes` | also inject strictly inside words — a diagnostic, not a stricter mode (comment interiors stay excluded) |
| `--by-node` | also print the coarse by-`(node, edge)` rollup after the run (report-only; see [Reading a finding](#reading-a-finding)) |
| `--rank` | print the top-N `(node, edge)` clusters as a paste-ready **markdown table** for `TODO_GAPS` §Status (report-only; `deno task gaps:audit:rank`) |
| `--since <baseline.json>` | print the **delta** vs a prior `--json` output: the per-cluster ranking diff, the per-shape `(kind, shape) → (count, payloads)` diff, and the seed-eligibility change (report-only) |
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
hand-curated `ALLOW`, it carries **no per-entry rationale by design**: at the scale the file
has run at (several hundred shapes at its peak) that is not a thing a human can keep honest.
Every line is a **known bug**, and the file shrinking is the goal.

```
# Format: KIND<TAB>SHAPE<TAB>PAYLOADS
DROPPED	import⟨⟩.	block
DOUBLE-PRINTED	IDENT⟨⟩=	block,line
SWALLOW	()⟨⟩;	line
```

The gate fails on:

- a shape **not** on the list — a new *kind* of loss, which must not land silently;
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

A `SWALLOW` shape is **pinned and graded exactly like a drop** — same file, same key
(`KIND<TAB>SHAPE<TAB>PAYLOADS`), same two failure modes (a shape not on the list, a listed
shape that no longer fires). It is the only kind here that loses **code** rather than a
comment, so a run that holds still names its share (`○ of those, N SWALLOW shape(s) …`) rather
than letting a bare `✓` over the whole file read as "no swallows".

It was **staged report-only** through its first phase, and the reason is worth keeping: the
check only arms on a text node carrying a whole comment, and at the time only `tsv_ts`'s
emitters spelled one that way — `tsv_svelte`'s built `text("//") + content` and were invisible
to it. Freezing a shape set that half the printers could not produce would have pinned a
property of the *instrument*. Every Svelte comment emitter now routes through the one-node
form, so the arm is whole and the class ratchets.

Two properties still differ from the ledger kinds. It is **not self-verified** — a swallow is
observed directly on the rendered output (like `blank_audit`'s F1/reparse kinds), so the
`UNCONFIRMED`/`PARTIAL` axis does not apply (nor the `⚑ ANCHOR?` probe, which rides the
verify pass); the verify pass's oracle is the multiset of
comment *contents*, which answers the ledger's question and not this one. And it has **no
bystander axis**: the tracker reports a property of an output *line*, not of a registered
comment, so every finding keys at its injection site.

Most shapes fire on the `line` payload alone — the injected `//` is the swallower. A handful
carry the block payloads too: there the injection merely reflowed the file and a comment the
*author* wrote does the swallowing, which is the same bug reached from further away.

Cost: arming the check adds roughly **+10% CPU** to a run (measured over `tests/fixtures`:
~146 s → ~160 s user, ~17 s → ~19 s wall) — a couple of seconds on a whole-`deno
task check` wall clock measured in minutes. The rejected alternative — running
the full `f1_check` battery per injection — was
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
- **`skipped by: <file:line:column>`** (when present) — the ledger's **skip-∧-dropped join**:
  a `CommentFilter::BlockOnly` builder passed over this line comment during the format, and the
  comment ended DROPPED. The `BlockOnly` licence is a promise that a gate routed line comments
  to an expansion builder first; this line names the caller whose gate broke that promise, so
  triage starts at the responsible call site instead of from the shape. A skip whose comment
  another emitter prints (the routed expansion path, a winning `conditional_group` sibling)
  never surfaces — a bare skip is an annotation, not a finding.
- **`⚠ UNCONFIRMED (0/5: …)` / `⚠ PARTIAL (2/5 confirmed; unconfirmed: …)` /
  `⚑ ANCHOR? (…)`** — the self-verify verdict with its per-cause breakdown, and the
  anchor-sensitivity hint over the confirmed side — see below.

### The by-node rollup (`--by-node`)

`--by-node` prints a second, **coarser** view after the run: the finding shapes rolled up onto
their structural key `(node_type, edge)` — the enclosing AST node and the child-role edge each
site's gap sits in (`(CallExpression, arguments→$)`, `(VariableDeclarator, id→init)`), read off
the wire tree. Where the site shape keys a finding by its raw adjacent tokens (the fine ratchet
key), this keys it by the **emitter**: the fine shapes fold onto far fewer
`(node, edge)` clusters — each roughly one printer function — ranked worst-first, the
burn-down work-list. The comment-attachment fields the wire mirrors from acorn
(`leadingComments` / `trailingComments`) are **not** treated as structural children, so a gap
keys to its emitter edge regardless of whether a comment happens to sit beside it. A **bystander** finding keys on its victim's site
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

**Clusters rank by DISTINCT GAPS, not raw hits.** A hit is one `(offset, payload)` finding, and
an N-wide whitespace run offers ~N injectable offsets while a glued (zero-width) gap offers
exactly one — measured ~3.7× (98.3 vs 26.3 hits/shape) — so a raw-hit ranking systematically
deprioritizes glued gaps, where the owned-claim and fused-`text()` families live. Each cluster
therefore also counts its **distinct gaps** — `(file, whitespace-run-start)` deduped, every
offset in one ASCII-whitespace run normalizing to the run's first byte — and the ranking sorts
on that (hits as tie-break, both still reported). A comment inside a gap splits the run into two
keys — a small accepted residual.

**Each cluster also carries its edge CLASS and kind COMPOSITION.** The class —
`leading` (`^→…`), `trailing` (`…→$`), or `interior` — is the boundary/interior split that
reframed the remainder (boundary regions are the fused-`text()`-with-no-query territory;
interior gaps are the element-comma-seam family). The kind cell (`drop 12 · swal 3`, compact
labels, nonzero only) says what a slice against the cluster would actually yield **before**
the slice starts: a gap-ranked #1 that is all `SWALLOW` has zero pinned-ratchet presence, so
its yield is silent-corruption fixes rather than line retirement — a lesson once rediscovered
mid-slice, now a column.

`--json` carries the ranked work-list as one additive top-level section, `by_node` — one
`{node, edge, edge_class, hits, gaps, shapes, share, gaps_share, example_shape, kinds}` per
cluster, distinct-gaps-descending (`kinds` keyed by the full snapshot labels, nonzero only) —
plus a top-level `by_node_unresolved` (the `UNRESOLVED` tail count)
and **`by_node_metric`**, the ranking-metric version stamp (`1` = the retired hits-sorted
ranking; `2` = distinct-gap sorted). `--since` deliberately keeps diffing **hits** — exact and
comparable across both metrics, so baselines from before the change stay usable; a consumer
wanting gap-based diffs keys on the stamp.

### The ranking, productized (`--rank` / `--since`)

The `--by-node` rollup is the raw material; three thin views make it something a session
consumes directly instead of parsing `--json` and hand-transcribing (all report-only —
byte-identical to the gate):

- **`deno task gaps:audit:rank`** (`--rank`, `--top N`) prints the top-N clusters as a
  **paste-ready markdown table** for `TODO_GAPS` §Status — rank, `` `(node, edge)` ``, edge
  class, distinct gaps, hits, shapes, kind composition, gap share (sorted by distinct gaps;
  see the by-node section above) — so
  the fattest-first work-list stays current by paste, not by re-transcription (which rots as
  slices land).
- **`--since <baseline.json>`** diffs this run against a prior `--json` output, in three
  report-only sections. The **ranking diff** lists the clusters whose hit count **changed** —
  `(CallExpression, arguments→$) 2861 → 2790 (−71)`, biggest reduction first — the direct
  answer to "did my slice move its target cluster?". The **shape diff** lists every
  `(kind, shape)` whose `(count, payloads)` changed — including a count move at an
  **already-pinned** shape, which the ratchet (a shape-set diff) and a payload-only diff are
  both structurally blind to, and which has carried a real injected double-print past a green
  gate. A clean run prints the strong claim explicitly: *no shape-level delta*. The
  **seed-eligibility** line prints only when `files` / `dirty` / `parse-skipped` moved — a fix
  that makes a dirty seed clean hands the audit new seeds, so a count rise on an existing
  shape can be new coverage rather than a regression, and the diff says so at the moment it
  matters. A missing/malformed baseline **warns and skips**; it never fails the gate.
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
`--jobs`-independent), and each is re-checked. The ratio separates **`UNCONFIRMED (0/N)`** —
*no* example reproduced — from **`PARTIAL (k/N)`** — some reproduced and some didn't, a
*mixed* real drop. An unlabelled shape confirmed on every example.

The pass is cheap and **lazily run**: each example costs two ledger formats (the re-splice
and its output) plus one more for the anchor probe on a confirmed one — thousands of formats
against the injection loop's millions — and the quiet green gate (`deno task check`'s path)
skips it entirely. The labels appear on `--report` / `--json`, on `--update`, and on any
non-holding, narrowed, or off-corpus run.

**A declined confirmation names its CAUSE**, because "unconfirmed" is not one thing — the
label carries the per-cause breakdown (`⚠ UNCONFIRMED (0/5: 4 content-conserved, 1
OUTPUT-UNPARSEABLE)`), the `--json` shape carries it as `verify_unconfirmed_causes`, and the
causes divide into three families:

- **`content-conserved`** — the output holds the same comment **contents** as its input, so
  something printed the comment without recording the emit: a genuine instrument gap, not the
  content loss it is filed as. (A mangled rebuild — which the old count read as UNCONFIRMED,
  since a mangle keeps the comment *count* — normalizes different and reproduces as
  **CONFIRMED**, the real corruption it is.) The residual, far narrower than the count's: a
  multiset can still balance if the *same* content is dropped in one place and duplicated in
  another; no corpus example does this.
- **`OUTPUT-UNPARSEABLE` / `OUTPUT-PANICKED`** — the formatter's **own output fails a
  re-format** after the injection: a **real corruption class**, not an instrument artifact,
  and one no other gate can see — `roundtrip:audit` formats files *as authored*, so an output
  made unreparseable by an injected comment is graded nowhere else. Every report path totals
  these on their own line (`verify_output_bug_examples` in `--json`); triage them first.
- **staleness / re-run artifacts** — `no-longer-fires` (the re-run produces no findings),
  `injection-rejected` / `injection-panicked` (the re-splice no longer formats), and the
  bookkeeping trio `seed-unreadable` / `payload-unknown` / `offset-invalid`.

**Confirmed examples get one more probe: `⚑ ANCHOR?`.** The probe re-splices the same payload
with a single leading space; if the finding *vanishes*, the shape is tagged (`⚑ ANCHOR? (2/3
rescued by a leading space)`, `verify_anchor_rescued` / `verify_anchor_probed` in `--json`).
That is the signature of a **fixed-offset anchor** — a scan starting at `span.start + K` that
the one extra byte pushes the comment past — mechanizing the triage move "vary the whitespace
and a fixed-offset anchor announces itself". It is a *hint* for cause triage, never a verdict:
a space can also legitimately change the site, so confirm the emitter by reading it.

The verify verdict is triage information, not a gate signal: it is a property of the shape's
sampled examples, not of the shape, so it is deliberately not part of the ratchet key (and
`--update` regenerates a byte-identical snapshot regardless of it). `--update` still reports
the tallies — how many shapes are fully `UNCONFIRMED` and how many `PARTIAL`, the cause
breakdown, and the output-bug shape count — since pinning a whole snapshot of claims is the
moment worth naming the ones the audit couldn't reproduce.

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

- **The ledger's scope.** Both comment carriers count: the **detached** comments a format
  entry registers, and the **AST-node** ones — a Svelte `<!-- … -->` and a CSS in-block
  `CssBlockChild::Comment`, which the ledger registers by span. What stays outside the model
  by construction is a CSS declaration's *value* comments: never lexed as `Comment`s at all,
  so there is nothing to register (that surface belongs to the
  [comment census](audits.md#comment-census-audit-censusaudit)). CSS also has no line
  comments, so the `line` payload is inert in a `.css` file.
- **`code_regions`' reach.** A gap the region walk doesn't name is a gap never probed. Today
  a `.svelte` file's `<style>` content is unprobed — so a Svelte fixture containing only a
  `<style>` block yields **zero sites**. That one is held back by **yield, not difficulty**:
  `Style::content_span` names it in a line, but measured over `tests/fixtures` — before the
  CSS in-block ledger extension, so re-measure before pricing it — it was +154k
  sites (+20% runtime) for 3 shapes, all `@import`-prelude double-prints. The thinness is
  structural — CSS's remaining unguarded comment surface is the declaration-value one the
  ledger cannot see at all — so the census, not the ledger, is what covers it.

Related: [Comment Ledger Audit](audits.md#comment-ledger-audit-commentsaudit) (the detector this drives),
[conformance_prettier.md §Comment Position Philosophy](conformance_prettier.md#comment-position-philosophy).
