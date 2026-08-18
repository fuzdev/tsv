# Blank-Line Injection Audit

> Inject a blank line into every gap and grade a fixed set of format invariants

`blank_audit` mechanizes the **blank-line handling** bug family — a printer that reflows a list, a
pattern, or a block and mishandles a blank line an author left in a gap: it fails to collapse a 2+
blank run, settles on a *different* output on the second pass (a non-idempotent fixed point), drops
a nearby comment, corrupts the reparse — or silently **eats** the blank at a kind of gap where it
should survive (the drop class, graded by [the absorb pin](#the-absorb-pin--the-blank-drop-class)
rather than by the invariants). The specifier-list and array-pattern blank-line bugs are
the named instances. Nothing else probes it: `fuzz`'s byte mutation essentially never forms a blank
line in a gap, `gap_audit` injects comments, and the fixture suite only ever formats each file **as
authored** — so a gap no fixture puts a blank in is a gap never checked.

For each seed file it injects a **blank line** into every candidate gap, one at a time, formats, and
grades six policy-free invariants on the result — plus the absorb pin over the drops.

Pure Rust, no sidecar. Gated in `deno task check` as a **ratchet**, not a green gate — it was born
RED over a live bug family, and the baseline (`blank_audit_known.txt`) is a snapshot of known bugs
whose shrinking is the goal.

**Design rationale lives next to the code** — why the sites are byte offsets, why a blank is graded
against the injected input (not the pristine output), and what a green run does *not* prove: see the
module docs at the top of `crates/tsv_debug/src/cli/commands/blank_audit.rs`. This file is the
operator's reference.

## The six invariants

Each injected blank is graded, keyed by the [site shape](#reading-a-finding) of the injection offset:

| # | invariant | finding kind | graded? |
| --- | --- | --- | --- |
| 1 | **no panic** — the formatter must never crash on a blank in a gap | `PANIC` | gates (never pinned — always fails) |
| 2 | **F1 idempotency** — pass 1 may keep or drop the blank, pass 2 must be a fixed point | `NON-IDEMPOTENT` | pinned |
| 3 | **structural reparse** — `format(injected)` reparses to the same document | `UNREPARSEABLE` (pinned) / `STRUCTURAL-DIVERGENCE` (**report-only**) | see below |
| 4 | **leaf conservation** — no decode-invariant leaf value changes | `LEAF-CORRUPTION` | pinned |
| 5 | **ledger-clean** — the blank must not drop / double-print a comment | `DROPPED` / `DOUBLE-PRINTED` | pinned |
| 6 | **blank-run ≤ 1** — the output never holds a 2+ blank run outside a verbatim region | `BLANK-RUN` | pinned |

Invariants 1–4 are the shared `f1_check` (also driving `fuzz`); 5 is the print-once comment ledger;
6 is a region-scoped output scan.

## The absorb pin — the blank-DROP class

The six invariants are deliberately policy-free, and invariant 2 lets pass 1 keep **or drop** the
injected blank — so a formatter that silently EATS a blank at some gap passes all of them forever.
The dropped-blank output is its own fixed point, which blinds every other standing gate too (F1,
fuzz, round-trip, the ledger, the census); only a prettier compare on the authored shape can see
one. That is the blank-DROP class (the after-element-fold family: an authored blank before a
last-child declaration, deleted), and the **absorb pin** closes its *silent* half.

A second machine-generated snapshot, `blank_absorb_known.txt`, pins every **node-edge class** —
`(node_type, left_role→right_role)`, the innermost AST node holding the injection offset and the
child-role edge its gap sits in — where an injected blank is ABSORBED (the output is byte-identical
to the pristine output, i.e. the blank was silently deleted). Two deliberate design points:

- **It is a BEHAVIOR PIN, not a bug list** (`width_audit`'s stance). Most absorption is sanctioned —
  prettier collapses a blank inside an expression, an argument list, a parameter list too — so a
  pinned class is a *kind of gap that eats blanks*, not a defect. A **NEW class is a question, not a
  verdict**: triage it against prettier (if prettier collapses the blank there too, re-pin; if
  prettier keeps it, it is a blank-DROP bug — fix it instead). New and stale classes both fail the
  gate, so a new kind of silently-eaten blank cannot land silently and the list cannot rot.
- **The key is coarse on purpose.** ~80% of injections absorb, so the bug ratchet's fine token
  shape would pin the fixture tree's whole token-adjacency vocabulary (~5.7k shapes, measured) and
  mint new ones on ordinary fixture PRs — the churn that gets gates turned off. A node-edge class
  (~525 over `tests/fixtures`; the snapshot header carries the exact count) ≈ one emitter decision, the grain a triage verdict actually covers.

One absorption is **exempt**: an injection beside an *already-authored* blank reproduces the
pristine output via the sanctioned 2+→1 run collapse (invariant 6's own requirement), not a drop —
recording it would pin classes where blanks otherwise survive (a `Program.body` statement gap)
just because one seed authors a blank beside one eligible site.

The pin is graded only on the **full default corpus** — absorption is normal behavior, so an
off-corpus absorbed class is not news the way an off-corpus finding is. `--report` prints every
class with its count and a reproducer (`e.g. inject blank at <path>:<offset>`), which is the
triage view of the pin file.

**Grading a line means running the prettier compare yourself**, since nothing here does it: splice
`\n\n` at the reproducer's offset and ask whether *prettier* still reproduces its own pristine
output. Three things make that harness easy to get wrong, each of which inflates the candidate
list rather than shrinking it:

- The offsets are **byte** offsets. Splicing at a UTF-16 index (a JS `String.slice`) splits tokens
  in every non-ASCII seed.
- **`prettier-plugin-svelte` swallows an embedded-TS parse failure** — it logs and emits the
  `<script>` body verbatim, so `format` *resolves* with a different output that has nothing to do
  with blank-line policy. Capture `console.error` around the call and treat a non-empty capture as
  its own verdict: the injected source is not valid TS, which is a parser over-acceptance question,
  not a blank one.
- **"The outputs differ" is not "prettier kept the blank."** Split it on whether the two are equal
  *modulo blank lines*; anything else changed the parse or the layout and is a different finding.

⚠️ **A class that reads ABSORBS on its canonical example is not cleared.** The key is one node-edge
class but the reproducer is one *textual shape*, and a class routinely spans many — an inline
comment, a glued run, an own-line run all key the same line. So a sweep over the per-class
reproducers under-reports (a fix can clear several classes while flagging one), and, in the other
direction, **a line does not go stale until NO injection at that edge absorbs** — a real fix will
often leave its line in place, which is correct and must not be "corrected" with
`blanks:audit:update`.

**`--json` therefore carries the real work-list**: `absorb_variants`, one row per
`(class, `[site shape](#reading-a-finding)`)` pair rather than per class, each with its own
reproducer (`class`, `shape`, `path`, `offset`, `snippet`). Over `tests/fixtures` that is ~14.8k
rows against the ~525 pinned classes, and the difference is not academic — the first per-class sweep of the
pin graded 10 divergences where the per-shape sweep of the same corpus graded 404. `--report`'s
per-class rows carry the pair count in a `[N shapes]` column, so a class whose reproducer reads
ABSORBS still shows how much of it that one reading covered. The pin file itself stays keyed by
class: the variants are a triage view, never pinned.

**What it still cannot see**: a drop at a gap the site enumeration never injects — the sites come
from `code_regions` (JS spans), so a Svelte **template-text** gap (where the
after-element-fold drop itself lived) is never probed; that needs the template-gap substrate extension, and as-authored drops over real
corpora need the blank census. Both are tracked as follow-ups, not covered here.

Every **policy** kind is **pinned** into the ratchet (NON-IDEMPOTENT, DROPPED, DOUBLE-PRINTED,
UNREPARSEABLE, LEAF-CORRUPTION, BLANK-RUN) — deliberately unlike `fuzz` / `roundtrip_audit`, where
non-idempotency is an absolute never-pinnable gate: this audit is a ratchet over a live bug family,
so its day-one findings must be pinnable or the gate would hard-block `deno task check` on landing.
Two carve-outs:

- **`PANIC`** always fails and is never listed (a crash is absolute).
- **`STRUCTURAL-DIVERGENCE` is held REPORT-ONLY** (fuzz-soft parity — fuzz's `structural_divergence`
  is its soft, non-fatal, canonical-confirmation-wanting bucket). A blank-induced structural change
  over Svelte is render-model noisy, so it is reported but **never gated** — neither pinned into the
  snapshot nor able to fail the gate. Mechanically it is *filtered out of the graded key set*
  (`is_graded`), a third category — not made "un-pinnable" (which would make it fail like a panic).

## Running it

```bash
deno task blanks:audit           # the gate: tests/fixtures, ~30 s
deno task blanks:audit:update    # regenerate BOTH snapshots (bug shapes + absorb pin)

# Directly, against a real codebase — where the real yield is:
cargo run --profile corpus -p tsv_debug --features audits blank_audit ../zzz/src
```

Build with **`--profile corpus`** (optimized + `panic = "unwind"`). Plain `--release` is
`panic = "abort"`, so a formatter panic would kill the run instead of being caught and reported.

| flag | effect |
| --- | --- |
| `--json` | machine-readable report on stdout (logs go to stderr; `absorb_shapes` count in the extras) |
| `--report` | print the full per-shape report, the skipped-file list, and the per-class absorb rows (each with its reproducer) even when the gates hold |
| `--jobs N` | worker threads (default: available parallelism) |
| `--limit N` | cap the seed files |
| `--update` | rewrite BOTH committed snapshots (bug shapes + absorb pin) |

`--json`, `--jobs`, and `--report` change how a run is reported and scheduled, never which sites it
reaches, so they don't narrow it. `--limit` and an explicit path DO: `--update` refuses a narrowed
run (it would pin a subset and silently unpin real bugs), and the ratchet is skipped with an
explicit `○ ratchet SKIPPED` note. Off the default corpus every finding is news, and any **graded**
finding exits 1 — STRUCTURAL-DIVERGENCE stays report-only there too (it is never in the graded set),
matching how it is held soft on the default corpus.

### Cost — the fast path

The audit stays near `gap_audit`'s one-format-per-site cost via a **fast path**: when the formatter
ABSORBS the blank (the output is byte-identical to the file's pristine, already-proven-idempotent
output), every invariant holds by transitivity and none of the property battery runs — the only
per-hit work is [the absorb pin](#the-absorb-pin--the-blank-drop-class)'s node-edge keying (one
span-containment descent over the file's already-parsed wire). Over `tests/fixtures` ~80% of
accepted injections absorb; only the rest — a blank the formatter KEEPS — pay the full property
battery, and that reuses the ledger's already-computed output rather than re-formatting. A run
reports the split (`N of M accepted injections were absorbed …`).

## The ratchet

`crates/tsv_debug/src/cli/commands/blank_audit_known.txt` is a **machine-generated** snapshot of
every finding shape `tests/fixtures` currently produces. Every line is a **known bug**, and the file
shrinking is the goal. (The sibling `blank_absorb_known.txt` is a different kind of file — a
behavior pin whose lines are mostly sanctioned; see
[the absorb pin](#the-absorb-pin--the-blank-drop-class).)

```
# Format: KIND<TAB>SHAPE
NON-IDEMPOTENT	IDENT⟨⟩,
DROPPED	␣⟨⟩/*
```

The gate fails on:

- a **graded** shape **not** on the list — a new *kind* of break, which must not land silently;
- a listed shape that **no longer fires** — a stale entry, so the list can't rot;
- a **panic**, always. A crash is never pinnable — a blank in a gap must never crash the formatter,
  so it always fails the gate rather than being ratcheted alongside the drops.

**`STRUCTURAL-DIVERGENCE` is not in the file at all** — it is held report-only (see the invariant
table), filtered out of the graded key set, so it is neither pinned nor able to fail the gate. It
still prints, in its own `○ N STRUCTURAL-DIVERGENCE shape(s) … reported, NOT gated` section (and
carries `"gated": false` under `--json`).

What it deliberately does **not** pin is **counts** — they churn with every ordinary fixture PR, and
a gate that fails per added fixture would just get turned off. There is no payload dimension in the
key (there is one payload). The tradeoff is named: a new break at an **existing** shape is invisible.

## Reading a finding

Findings dedup by **site shape** — the adjacent tokens with identifiers abstracted (`IDENT⟨⟩,`,
`␣⟨⟩/*`, `...⟨⟩IDENT`). One bug fires at every site that reaches it, so raw `(file, offset)` findings
would be unreadable and, as a ratchet key, would go stale on the next fixture edit.

```
    413×  NON-IDEMPOTENT  IDENT⟨⟩,
          17 file(s)
          e.g. inject blank at tests/fixtures/…/input.svelte:63  …{#snippet fn2(a⟨⟩, b)}…
```

- **`e.g. inject blank at <path>:<offset>`** — splice a blank line (`\n\n`) at that byte offset in
  that file and format to reproduce.
- The `⟨⟩` in the shape / snippet marks the injection point.

There is no bystander axis (unlike `gap_audit`): a blank line drops nothing of the author's by
relocation.

**On confidence — the ledger kinds are not self-verified.** The F1, reparse, leaf, and blank-run
invariants (2, 3, 4, 6) are observed **directly on the output** — a shape reproduces or it does not.
The two **ledger kinds** (`DROPPED` / `DOUBLE-PRINTED`, invariant 5) are different: they are reported
as the print-once ledger *sees* them, **without** the per-example self-verification `gap_audit` runs
(its confidence axis, on the principle that "an instrument that only agrees with itself is not
evidence"). So a pinned ledger-kind shape is a known-bug **candidate**, not a self-confirmed one — it
could include an instrument-gap false positive. That is self-correcting: such an entry goes stale
when the ledger improves, and the ratchet's stale-entry check catches it. A per-example verify pass
for the ledger kinds (mirroring `gap_audit`'s confidence axis) is a possible future hardening.

## Triaging and fixing a shape

1. **Reproduce by hand** — inject a blank line at the example offset and format.
2. **Fix the printer**, fixtures-first per the repo's TDD rule. The fix is usually to route the gap
   through a blank-aware reflow rather than assuming the gap is empty.
3. **Re-pin**: `deno task blanks:audit:update`, and confirm the shape's line is **gone** from the
   snapshot rather than merely changed.

If a shape is genuinely pre-existing and merely newly *reached* by a fixture you added, the same
`blanks:audit:update` is the right move.

## Scope — what a green run does not prove

- **CSS is deferred.** A `.css` seed is skipped outright, and a `.svelte` file's `<style>` is
  unprobed (`code_regions` doesn't name it) — CSS's whole-file region is the most exposed to the
  string-interior class below, and its blank-line behavior is a separate follow-up.
- **String / template interiors are excluded.** tsv's lexer accepts a raw newline inside a quoted
  string as content, so a blank injected there would not be *rejected* — it would silently become
  string content and read as a false finding. `string_and_template_spans` excludes string-literal
  and template-quasi interiors up front (the third exclusion class after word interiors and comment
  interiors); the `${ … }` expression holes stay probed.
- **Only format fixed points are injected into.** A seed that isn't idempotent, doesn't reparse, or
  already violates a blank-run AS AUTHORED is reported once and skipped (injecting would re-report
  the base problem at every site). Over `tests/fixtures` that skips the `unformatted_*` / variant /
  prettier-output fixture files, which are not tsv fixed points by design — expected, and reported
  as a count (`--report` to list the paths). A ledger-dirty file is reported as `comments:audit`
  would report it.
- **A format-ignore-bearing file is exempt from invariant 6 whole** — locating the verbatim ignore
  range from the output alone is fragile — while the other five still run.
- **The structural fast accept has one narrow blind spot** — a format that DROPS an ASI split the
  injection introduced (output back to the pristine shape) — covered by `fuzz` / `roundtrip_audit`.

Related: [Gap-Injection Audit](gap_audit.md) (the same substrate, for the dropped-comment class),
[Comment Ledger Audit](audits.md#comment-ledger-audit-commentsaudit) (invariant 5's detector).
