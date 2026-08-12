# Audit Gates

> The standing correctness audits over the formatter, the parsers and their wire contract, the Svelte compiler, and the canonical oracles all of them are graded against — what each proves, what it is blind to, how to run it, and where it gates. The `deno task` entry points are indexed in [CLAUDE.md §Fixtures](../CLAUDE.md#fixtures-rust--deno-based); this doc is the full reference.

Most audits are pure Rust (no Deno sidecar). Those gated in `deno task check` scan `tests/fixtures` — a curated, format-stable tree — so several are cheap tripwires there whose real yield is external corpora (`../prettier/tests/format`, `../svelte/packages/svelte/src`, sibling dev repos): point them at real code after a printer change, or run `deno task audit:corpus`, the standing bundle for exactly that. Audits that need the feature-gated instrumentation (`swallow_check` / `comment_check`) build via the `audits` umbrella feature under `--profile corpus` — the single build world every `deno task check` audit shares (optimized + `panic = "unwind"`, so a formatter panic is caught and reported instead of killing the process; plain `--release` is `panic = "abort"`).

The Svelte compiler's *sidecar-dependent* harnesses — the corpus comparison, the validation-suite ratchet, the differential compile fuzzer — are not audits in this sense and are not gated here; they live in [compile_tooling.md](compile_tooling.md) and [compile_validation_ratchet.md](compile_validation_ratchet.md). The compile fixtures are the one split case: their parity legs are pure Rust and gate in `check`, their oracle-freshness leg needs the sidecar and gates in `conformance`, so both are documented [below](#compile-fixture-validation-compilefixturesvalidate).

**One seed resolution, one directory walk, and a vacuity floor that is not scope-dependent.** Every corpus-walking audit resolves its corpus through `resolve_seed_files` / `resolve_seed_files_named` (`tsv_debug`'s `cli/commands/profile.rs`): positional paths defaulting to `tests/fixtures`, one walk that prunes what `tsv format`'s own discovery prunes and keeps its extension set (`tsv_discover`'s safety nets, build-output heuristic, and `FORMATTABLE_EXTENSIONS` — so an audit's scope is the set the production formatter would process, not a hand-mirrored list beside it), then the audit's own subject filter, so an empty scan says "no `.svelte` files found" rather than a flattened message. **An empty scan is an error at every scope, and so is a run that resolves files but grades none of them** (`check_graded_nonzero` — the case where the corpus resolves fine and the parser rejects all of it). Each audit floors the count its own *verdict* rests on, not the count resolution returned: files formatted, files compared, boundary sites probed, render keys checked. The rule for picking it — count every outcome that carries a verdict, exclude only the ones that could not be evaluated. A trivially-clean outcome is a verdict (a no-op format renders identically by identity) and counts; "the parser rejected it" is not, and does not. The pinned minimums (`FIXTURES_FORMATTED_MIN`, `comments:audit`'s `REGISTERED_MIN`) are the stronger *default-corpus* guard layered above that floor: they catch a corpus that shrank rather than one that vanished, and only a default run can be held to a number, so they stay `default_paths`-gated. The two pins ask different questions — the file count catches a corpus that shrank or a skip policy that diverged, the comment count catches *registration* collapsing while every file still formats — so `comments:audit` passes both. Ignore files are deliberately **not** consulted by the walk — the root `.formatignore` prunes the fixture trees (they are data, not format fixed points), so a walk honoring them would resolve the audits' own default corpus to nothing.

## Overview

| audit | task | catches | gating |
| --- | --- | --- | --- |
| [Swallow](#line-comment-swallow-audit-swallowaudit) | `swallow:audit` | `//` line comment followed by content on one output line (silent content loss) | `deno task check`; `audit:corpus` (real code) |
| [Comment ledger](#comment-ledger-audit-commentsaudit) | `comments:audit` | a parsed comment DROPPED or DOUBLE-PRINTED (print-once) | `deno task check` |
| [Gap injection](#gap-injection-audit-gapsaudit) | `gaps:audit` | comment drops — and `//` swallows — in gaps no fixture covers | `deno task check` (ratchet) |
| [Blank injection](#blank-line-injection-audit-blanksaudit) | `blanks:audit` | blank-line handling: panic / idempotency / reparse / ledger / blank-run | `deno task check` (ratchet) |
| [Blank fabrication](#blank-fabrication-audit-fabricationaudit) | `fabrication:audit` | a blank line the formatter INVENTS on a pristine seed (the author never wrote it) | `deno task check` (ratchet) |
| [Comment census](#comment-census-audit-censusaudit) | `census:audit` | a comment interior lost, gained, or rewritten between raw input and raw output — parse-time drops included, which the ledger can't see | `deno task check` (ratchet) |
| [Print width](#print-width-audit-widthaudit) | `width:audit` | a new KIND of over-width output line — the shape a hard-limit bug takes | `deno task check` (ratchet) |
| [Ignore honoring](#ignore-directive-honoring-audit-ignoreaudit) | `ignore:audit` | `prettier-ignore` positions that silently reformat an ignored node, misbind a trailing directive, over-freeze, or lose the freeze on pass 2 | `deno task check` (ratchet) |
| [Build fanout](#build-fanout-audit-fanoutaudit) | `fanout:audit` | exponential doc-node rebuild in nested layout candidates | `deno task check` |
| [Raw-find scan](#raw-find-scan-audit-scanaudit) | `scan:audit` | new raw substring scans over source (comment-blind delimiter matching) | `deno task check` |
| [Self-format](#self-format-audit-formataudit) | `format:audit` | tsv failing to format its OWN TS/JS — a would-change file (non-idempotency) or a parse error (over-rejection) | `deno task check` |
| [Doc link](#doc-link-audit-docsaudit) | `docs:audit` | a doc-comment `[link]` that no longer resolves — a stale doc | `deno task check` |
| [Wire-type drift](#wire-type-drift-check-checkast-types) | `check:ast-types` | the shipped `tsv_ast.d.ts` no longer describing what the wire-JSON writers emit | `deno task check` |
| [Pin agreement](#canonical-pin-agreement-audit-pinsaudit) | `pins:audit` | the five canonical-oracle pin sites disagreeing — including the lockfile, which alone pins the oracle's own transitive deps | `deno task check` |
| [Checkout alignment](#checkout-alignment-audit-pinsauditcheckouts) | `pins:audit:checkouts` | a present `../svelte` / `../acorn-typescript` clone that is not the pinned version; commit drift (warn) | `deno task conformance` (preflight) |
| [Authoring independence](#authoring-independence-audit-authoringaudit) | `authoring:audit` | two render-equivalent authorings settling on two fixed points; non-idempotency | `deno task check` |
| [Round-trip](#formatreparse-round-trip-audit-roundtripaudit) | `roundtrip:audit` · `roundtrip:audit:prettier` | formatted output the parser rejects (delimiter/structure corruption) | `deno task check` (fixtures always; the prettier suites when `../prettier` is present) |
| [Binding](#commenttoken-binding-audit-bindingaudit) | `binding:audit` | a glued comment re-bound to a different subtree by a migrating paren | `deno task check` |
| [Render equivalence](#render-equivalence-audit-renderaudit) | `render:audit` | `tsv format` changing what a Svelte component renders | `deno task conformance` (release) |
| [Layout neutrality](#layout-neutrality-audit-neutrality_audit) | — | a layout gate reading comment *ownership* instead of page occupancy | dev tool (pre-ownership-change) |
| [Fuzz](#seeded-mutational-fuzzer-fuzzaudit) | `fuzz:audit` | panic / non-idempotency / structural divergence on arbitrary input | `deno task check` |
| [F1 sweep](#f1-idempotency-sweep-idempotencysweep) | `idempotency:sweep` | pass-2 reflow on real code | conformance cadence |
| [Corpus bundle](#the-corpus-bundle-auditcorpus) | `audit:corpus` | the content-loss / robustness bundle over real code | publish Step 3c |
| [Lexer diff](#differential-lexer-harness-lex_diff) | — | token-stream drift after a lexer change | dev tool |
| [Conformance audit](#conformance-audit-conformanceaudit) | `conformance:audit` | doc/fixture catalog + link integrity | `deno task check` |
| [Compiler conformance](#compiler-conformance-audit-conformanceauditcompiler) | `conformance:audit:compiler` | compile-fixture divergence catalog + checklist ↔ `Refusal` drift | `deno task check` |
| [Canonicalizer](#canonicalizer-audit-canonicalizeaudit) | `canonicalize:audit` | `canonicalize_js` non-idempotence / corrupt output / comment loss | `deno task check` |
| [Compile fixtures](#compile-fixture-validation-compilefixturesvalidate) | `compile:fixtures:validate` | a stale compile expectation (oracle freshness) · tsv-vs-expected compile parity · expected-file idempotence | parity legs in `deno task check` (`cargo test`); freshness in `deno task conformance` |

## Line-Comment Swallow Audit (`swallow:audit`)

```bash
# swallow_audit - format files with the render-time swallow check on and report
# any `//` line comment followed by content on the same output line (silent
# content loss). Pure Rust, no Deno. Defaults to tests/fixtures; pass dirs/files
# to audit real code. Exits 1 on any finding.
cargo run --profile corpus -p tsv_debug --features audits swallow_audit                # audit all fixtures
cargo run --profile corpus -p tsv_debug --features audits swallow_audit ~/dev/zzz/src  # audit a real codebase
# Also: --json. The check lives in tsv_lang::doc::swallow behind the `swallow_check`
# cargo feature — off by default, so it's compiled out of prod wasm/cli/ffi AND
# default tsv_debug builds (profile/perf sessions measure production-shaped render
# code). The `swallow:audit` deno task builds it via the `audits` umbrella feature
# (swallow_check + comment_check) under `--profile corpus`, the single build world
# EVERY `deno task check` audit shares; `--features swallow_check` alone still works
# for a targeted run. Gated in `deno task check` (via `swallow:audit`) over tests/fixtures,
# and as a leg of `deno task audit:corpus` over the real-code corpus + the prettier suites —
# where the class actually lives (the callee-position swallow sits in ONE file anywhere).
#
# Coverage is every render that appends to the output buffer — the main loop AND
# its sub-renders (fill segments, the line-suffix flush), all driving one
# per-thread state machine. A `line_suffix` comment is NOT exempt: two of them
# flushed at the same line break land back-to-back on one line (`x; // c2 // c1`)
# and the first `//` swallows the second.
#
# ONLY a `//` (and the hashbang) can swallow — `/* */` and `<!-- -->` close at their
# own delimiter, so they carry no tag and appear here only as a swallow's VICTIM.
# What arms the check is a text node carrying the WHOLE comment
# (`line_comment_source_span` / `line_comment_text_pooled`); an emitter that spells
# one as `text("//") + <content>` is invisible to it, so every line-comment emitter
# in tsv_ts and tsv_svelte uses the one-node form. Out of scope, victim-direction
# only: content written straight to the output buffer (the Svelte hoisted-section
# `<!-- -->` path), and a `//` left pending across a top-level render boundary
# (each `<script>`/`<style>`/template root node is its own render, which clears the
# pending state). Neither is reachable today — every Svelte `//` emitter ends its
# line — and the census covers both from the output side.
```

⚠️ **A green `swallow:audit` does not mean "no swallows"** — it formats each file **as
authored**, so a swallow only reachable once a comment sits in some other gap is a swallow it
never provokes. The [gap-injection audit](gap_audit.md) arms this same check on its injected
formats and ratchets what that reaches, as its
[SWALLOW class](gap_audit.md#the-swallow-class). The
[comment census](#comment-census-audit-censusaudit) sees an as-authored swallow from the other
side — the comment's interior GAINS the swallowed code, a multiset imbalance — with no
instrumentation seam at all, so it covers the two seams listed above that this check cannot
reach (its first external sweep caught a live `as const` swallow this way).

## Comment Ledger Audit (`comments:audit`)

The print-once comment ledger: every comment a document PARSES must be EMITTED exactly once. tsv's answer to prettier's `ensureAllCommentsPrinted`, and the structural guard on the [detached comment model](./comments.md): nothing else *inside the model* forces a comment that the parser produced to actually reach the output. (The [comment census](#comment-census-audit-censusaudit) forces conservation from *outside* the model — content-level, registration-independent — which is what covers the comments the parser never produced at all.)

```bash
# comment_audit - format files with the print-once comment ledger on and report every
# comment the format DROPPED (parsed, never emitted — silent content loss) or
# DOUBLE-PRINTED. Pure Rust, no Deno. Defaults to tests/fixtures; pass dirs/files to
# audit real code. Exits 1 on any finding.
cargo run --profile corpus -p tsv_debug --features audits comment_audit                # audit all fixtures
cargo run --profile corpus -p tsv_debug --features audits comment_audit ~/dev/zzz/src  # audit a real codebase
# Also: --json. The ledger lives in tsv_lang::comment_ledger behind the `comment_check`
# cargo feature — off by default, so it's compiled out of prod wasm/cli/ffi AND default
# tsv_debug builds (profile/perf sessions measure production-shaped code). The
# `comments:audit` deno task builds it via the `audits` umbrella feature (swallow_check +
# comment_check) under `--profile corpus`, the single build world EVERY `deno task check`
# audit shares; `--features comment_check` alone still works for a targeted run. Gated in
# `deno task check` (via `comments:audit`) over tests/fixtures.
```

The corpus walk is the shared pristine-format sweep, as in its twin [`swallow:audit`](#line-comment-swallow-audit-swallowaudit) — same skip buckets, and a formatter panic mid-walk is caught, counted, and named with its input rather than killing the run (panics are reported, not gated: the panic gates own that class).

**Model.** A format entry point (`tsv_ts::format_in`, `tsv_css`'s `format_css*`, `tsv_svelte`'s `format_svelte*`) REGISTERS the comment list it is about to print — that is the expectation. A doc-based printer (tsv_ts, tsv_svelte) TAGS each comment's doc node (`DocArena::tag_comment_doc`) and the RENDERER records the emit when it reaches the node; tsv_css, which writes comments straight to its buffer, records at the write. The render-time seam is load-bearing: a builder may assemble the same subtree into two `conditional_group` candidates of which one renders, so counting at build time reads as a double-print (and a comment built only into a LOSING candidate would read as printed while being lost). A `format-ignore` region — and any other raw source slice that carries comments out verbatim (a raw at-rule prelude, a glued CSS compound selector) — records a VERBATIM RANGE that counts as one emit per comment it covers; keep those ranges tight, a too-wide carve-out silently re-opens the hole.

**Scope.** Both comment carriers are registered and guarded: the DETACHED comments (the flat `Vec<Comment>` on the language root) and the AST-NODE comments — a Svelte `<!-- … -->` (`FragmentNode::Comment`) and a CSS in-block `CssBlockChild::Comment`. The latter are carried by the tree rather than by the positional model, but a printer can still drop or double-print one, so each format entry walks its tree and registers their spans; with that, `unregistered emits` is a pure registration-gap signal (0 over clean fixtures) — a nonzero count means the walk missed a container. CSS declaration-VALUE comments remain outside the model by construction — never lexed as `Comment`s at all (re-derived from source), so there is nothing to register. Everything outside the ledger's model — those value comments, and any comment a parse path consumes without registering — is the [comment census](#comment-census-audit-censusaudit)'s remit, which lexes both raw sides and never consults registration at all.

**Blind to: any position no input actually puts a comment in.** The ledger grades a document AS AUTHORED, so its verdict is only ever as strong as the corpus. A builder that emits its children's docs and never looks up a gap at all ([comments.md](./comments.md) hazard 4) drops *every* comment in that gap and still reports green everywhere the shape happens not to occur — `build_object_doc_expanded` / `build_array_doc_expanded` did exactly that on the member-chain hugged-argument route while `comments:audit` passed over tests/fixtures, zzz, gro, fuz_app, fuz_ui, prettier's TS+JS suites, and svelte's own source. `gaps:audit` is the discovery arm for that class — it had the same drops pinned as sixteen known shapes. A green ledger is evidence about the corpus, not about the printer; when adding or changing an alternate-layout builder, inject a comment into each of its gaps by hand.

## Gap-Injection Audit (`gaps:audit`)

Full reference — flags, the ratchet, reading a finding, triage + re-pin workflow, scope: **[gap_audit.md](./gap_audit.md)**. Design rationale (why byte offsets and not tokens, why the ledger is the oracle, why five payloads) lives in the `gap_audit` module docs.

```bash
# gap_audit - inject a comment into EVERY gap and re-run the print-once ledger. The
# DISCOVERY arm `comments:audit` can't be: the ledger only ever sees a document AS
# AUTHORED, so a gap no fixture puts a comment in is one it never checks (eight such
# drops were found BY HAND, all green on every gate). Pure Rust, no sidecar.
cargo run --profile corpus -p tsv_debug --features audits gap_audit   # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits gap_audit ~/dev/zzz/src
# Also: --json, --jobs N, --limit N, --payload <one>, --all-bytes, --update.
# Build with `--profile corpus` (optimized + panic=unwind): plain `--release` is
# panic=abort, so a formatter panic kills the process instead of being caught + reported.
#
# GATED as a RATCHET, not a green gate: `gap_audit_known.txt` is a machine-generated
# snapshot of every shape tests/fixtures produces, every line a KNOWN BUG, the file
# shrinking is the goal. A shape not on the list, one on it that no longer fires, or any
# PANIC, FAILS. `--limit`/`--payload`/`--all-bytes`/a path narrow a run, so they skip the
# ratchet and refuse `--update`. ~17 s.
#
# Two detectors ride the one format, and BOTH are ratcheted: the ledger's DROPPED /
# DOUBLE-PRINTED, and the render-time swallow check's SWALLOW — a `//` eating the content
# after it, i.e. lost CODE, which the print-once ledger is structurally blind to (the
# comment IS printed once). A holding run names the swallow share on its own line.
```

`deno task gaps:audit:update` regenerates the snapshot after fixing a shape (or when a new fixture merely REACHES a pre-existing one); it refuses a narrowed run.

## Blank-Line Injection Audit (`blanks:audit`)

Full reference — flags, the ratchet, reading a finding, the six invariants, scope: **[blank_audit.md](./blank_audit.md)**. Design rationale (the fast path, why a blank is graded against the injected input not the pristine, the string-interior exclusion) lives in the `blank_audit` module docs.

```bash
# blank_audit - inject a blank line into EVERY code gap and grade six policy-free
# invariants on the result: (1) no panic, (2) F1 idempotency (pass 2 is a fixed
# point), (3) structural reparse, (4) leaf conservation, (5) ledger-clean (no
# dropped/double-printed comment), (6) blank-run ≤ 1 (no 2+ blank run outside a
# template quasi / <pre> / <textarea> / format-ignore region). Mechanizes the
# blank-line handling class — the specifier-list / array-pattern bugs. Invariants
# 1-4 are the shared `f1_check` (also driving `fuzz`); 5 is the print-once ledger;
# 6 is a region-scoped output scan. Pure Rust, no sidecar.
cargo run --profile corpus -p tsv_debug --features audits blank_audit   # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits blank_audit ~/dev/zzz/src
# Also: --json, --report, --jobs N, --limit N, --update. Build with `--profile
# corpus` (optimized + panic=unwind) so a formatter panic is caught + reported.
#
# GATED as a RATCHET (like gap_audit): `blank_audit_known.txt` is a machine-generated
# snapshot of the known-bug shapes, every line a bug, the file shrinking is the goal.
# A graded shape not on the list, one that no longer fires, or any PANIC, FAILS. Unlike
# fuzz/roundtrip, NON-IDEMPOTENT and every policy kind ARE pinned (born red over a live
# bug family); PANIC stays absolute; and STRUCTURAL-DIVERGENCE is held REPORT-ONLY
# (fuzz-soft parity — reported but never gated, filtered out of the ratchet, `"gated":
# false` in --json). A FAST PATH — a blank the formatter ABSORBS reproduces the file's
# proven-clean pristine output byte-for-byte, so nothing is checked — keeps it near
# gap_audit's one-format-per-site cost; only a KEPT blank pays the full battery (~19%
# of injections over tests/fixtures). ~24 s.
# Scope: TS + Svelte body; CSS deferred; string/template interiors excluded (a raw
# newline there is lexed as content, not a gap); only format fixed points injected.
```

`deno task blanks:audit:update` regenerates the snapshot after fixing a shape; it refuses a narrowed run.

## Blank-Fabrication Audit (`fabrication:audit`)

The **pristine** counterpart to `blanks:audit`. That audit MUTATES a seed — injects a blank into a code gap and grades the response — so its subject is how the formatter reacts to a blank the author *did* write. This one never mutates: it formats the seed as authored and asks whether the output holds a blank run the input did not.

Why it needs its own gate. A fabricated blank is indistinguishable from an authored one once written, so it is silent content the author never approved — and **every other gate is structurally blind to it**:

- **F1 / idempotency** — the fabricated blank is authored as far as pass 2 is concerned, so pass 2 preserves it and the file is a fixed point. The property never trips.
- **`blanks:audit` / `gaps:audit` / `ignore:audit`** — all grade a MUTATED seed, and the first two exempt whole-file format-ignore regions outright.
- **Corpus format compare** — a fabrication prettier also makes is a match, not a divergence.

```bash
# fabrication_audit - format each pristine seed and report every blank RUN the output
# holds that the input did not, minus two structurally sanctioned layout rules.
# Pure Rust, no Deno. Defaults to tests/fixtures; pass dirs/files to audit real code.
cargo run --profile corpus -p tsv_debug --features audits fabrication_audit
cargo run --profile corpus -p tsv_debug --features audits fabrication_audit ~/dev/zzz/src
# Also: --json, --update. ~0.2 s over tests/fixtures.
#
# GATED as a RATCHET over `fabrication_audit_known.txt`, keyed by the SHAPE of the two
# lines bracketing the invented blank (`{:catch` ⇢ blank ⇢ `{/await`), not by path — so
# the snapshot is corpus-portable and states the bug rather than a location. Every line
# is a bug; the file shrinking is the goal. Currently EMPTY (born green).
```

**The metric.** Blank *runs*, not blank lines: collapsing `a⏎⏎⏎⏎b` to `a⏎⏎b` removes newlines but not the author's "there is a break here" signal. A finding is `unsanctioned_runs(output) > runs(input)`.

**The two sanctioned fabrications** are structural carve-outs in the audit, deliberately not snapshot lines — mixing sanctioned rules into a known-bug list would make "every line is a bug" false:

1. **Hoisted-section seam** — tsv moves `<script>` / `<style>` / `<svelte:options>` to canonical positions and separates each from its neighbours with a blank. The carve-out is **two-sided**: a glued `</script><div>` puts the closing tag before the run, a glued `</div><style>` puts the opening tag after it.
2. **Empty block body** — a kept-but-empty block section prints in block form, and the empty body between opener and terminator is a blank line (`{:catch error}{/await}` → `{:catch error}⏎⏎{/await}`). Sanctioned by [`empty_branch_collapse`](../tests/fixtures/svelte/blocks/empty_branch_collapse_prettier_divergence/) and [`empty_catch_multiline`](../tests/fixtures/svelte/blocks/await/empty_catch_multiline_prettier_divergence/), whose READMEs state it.

**Known over-report: a section's leading comment run.** Rule 1 recognizes the seam by the section *tag*, but comments travel with the section, so a glued `<div>block1</div>⏎<!-- comment -->⏎<style>` puts the section's **leading comment** where the tag would be — the run reads `</div` ⇢ `<!--` and no carve-out fires, though prettier emits the identical blank. Left un-widened deliberately: the shape the audit can see is "a blank before some comment", and excusing that would blind it to a whole class of real fabrications. The narrow statement needs the AST, not the line shapes. Currently latent — an already-formatted corpus carries the blank in its input, so counts match and nothing trips; it fires only on pristine glued input.

**Blind spots.**

- **Net-zero.** The metric compares counts, so a run fabricated in one place while another is dropped elsewhere in the same file nets out and is missed. Closing it needs a position-preserving alignment between input and output, which reflow (and section hoisting, which relocates blanks wholesale) makes unavailable.
- **Vacuous on fixed points.** Where `format(S) == S` the property holds by construction, so over a corpus of already-tsv-formatted files the audit adds nothing over F1. Its yield is on **pristine, not-yet-formatted** code — exactly where a first-format fabrication would otherwise go unnoticed, because every later format is a fixed point.
- **Shape attribution.** A file trips on a count, and every unsanctioned run in *that file* then contributes its shape. So a tripped file can pin an innocent shape alongside the guilty one. Harmless while the snapshot is empty; if it fills, read a line as "a shape present in a file that fabricated", not "this shape fabricated".

## Comment-Census Audit (`census:audit`)

The whole-comment conservation gate: does every comment the author wrote survive formatting, byte-for-byte (modulo re-indent)? Per file, lex the comment trivia off the raw INPUT and the raw formatted OUTPUT — with the audit's own trivia scanners, **never** `parse().comments` — and compare the interior **multisets**, per language bucket. A drop, a duplication, a merge, or an interior rewrite is a plain arithmetic imbalance, no matter which internal layer caused it.

Why it needs its own gate: every other comment instrument reads a channel the parser controls. The print-once ledger guards what a format entry *registered*; `parse().comments` is what the parser chose to carry. A comment a parse path consumes without registering (the CSS `skip_whitespace_and_comments` class that motivated this audit) never existed as far as those instruments know — the corpus stays green **by absence**, and every corrupted output in that family was a format fixed point, so F1, roundtrip, fuzz, and the authoring audit were all structurally blind too. The census's independence from the parser's comment carrying is its entire design.

```bash
# census_audit - format each pristine seed, lex comment trivia from BOTH raw sides with
# self-contained scanners (audit/census.rs), and compare per-line-trimmed interior
# multisets per language bucket: `ts` (TS-family files, <script> islands, template
# {expressions}), `css` (.css files, <style> islands), `template` (Svelte <!-- -->).
# MISSING = dropped comment; EXTRA = duplicated/fabricated one; a merge or interior
# rewrite shows as a MISSING + EXTRA pair. Pure Rust, no Deno.
cargo run --profile corpus -p tsv_debug --features audits census_audit                # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits census_audit ~/dev/zzz/src  # a real codebase
# Also: --json, --update. ~0.35 s over tests/fixtures.
#
# GATED as a RATCHET over `census_audit_known.txt`, keyed (path, bucket, direction) —
# file-level, like the compile validation ratchet (the file IS the reproducer). Born
# EMPTY: the CSS parse-time-drop class it was argued from was fixed by hand before the
# audit landed, so over tests/fixtures it stands as the tripwire that keeps the class
# closed. Whole-comment drops are sanctioned in exactly ONE place — the CSS CDO/CDC
# `<!-- ... -->` span, which tsv (matching parseCss) discards WHOLESALE, CSS between the
# markers included — and that carve-out lives in the scanner (those comments never enter
# the input multiset), so a snapshot line is always a bug. Rejected inputs make no
# format claim and are skipped; a format PANIC is counted, not gated (the panic gates
# own that class).
```

**The scanners** (`audit/census.rs`) are deliberately self-contained rather than driving the product lexers: TS comment *extents* depend on parser context (a regex body is opaque only because the parser said "regex here"), so a raw `next_token` loop mis-lexes real code — and an instrument sharing the product lexer's extent rules would inherit its bugs. TS handles strings, template literals (interpolation stack included), and regexes via the classic previous-token heuristic; CSS handles strings and unquoted `url()` opacity; Svelte is a lexical mode machine — `<script>`/`<style>` raw-text islands bounded by the first matching close tag (exactly Svelte's own rule, so a `</script>` inside a JS string bounds identically), `{...}` expressions in text, attribute, and quoted-attribute-value position, block sigils stepped over so `{/if}` is never a regex head. Interiors normalize by **per-line ASCII trim only** (`[ \t\r]` — multi-line blocks legitimately re-indent; NBSP/form feed at a line edge is content and stays significant).

**Where the yield is.** Over `tests/fixtures` the gate is a cheap standing tripwire; the discovery arm is external corpora. Its first sweep over the prettier suites found a live `as const` **code swallow** (`(1 // comment⏎) as const;` → `1 // comment as const;` — the code after the paren pulled into the comment) plus four line-comment **merge** sites (`// a⏎// b` → `// a // b`, the second comment demoted to text) — all invisible to every other standing gate. Point it at real code after any parser/printer comment change.

**Blind spots.**

- **Position-blind by construction.** The multiset compares interiors, not placements — a comment relocated anywhere in the document (even to a semantically wrong place) balances. Placement is `binding:audit`'s and the fixtures' remit.
- **Same-content cancellation.** A dropped `// x` plus a fabricated identical `// x` elsewhere in the same file nets zero, the same net-zero blindness `fabrication:audit` documents.
- **Instrument-symmetry residue.** The scanners misread rare shapes (a regex after `)`, post-`}` division) — but they misread input and output with the same eyes, so the phantoms cancel. A false positive needs the formatter to rewrite text the scanner misreads *differently* across the two sides; none observed over tests/fixtures, zzz, svelte src, or the prettier suites.
- **As-authored only.** Like every pristine audit, a drop in a gap no corpus file puts a comment in stays invisible — `gaps:audit` is the injection arm for that class (with the ledger, not the census, as its oracle).

`deno task census:audit:update` regenerates the snapshot after fixing a pinned loss site (or pinning a newly found one); it refuses a narrowed run.

## Print-Width Audit (`width:audit`)

The only gate that measures a **column**. [conformance_prettier.md §Print Width Philosophy](./conformance_prettier.md#print-width-philosophy) says tsv treats `printWidth` as a **hard limit** where prettier treats it as a soft target — *a line tsv can break is a line tsv does break* — and nothing measured that claim until this audit. It formats each seed and measures every output line.

Why it needs its own gate: **every other gate is blind to an over-width line, by construction.**

- **F1 / fuzz / round-trip** — the over-width output is a *fixed point*; formatting it again reproduces it byte-for-byte, and it reparses.
- **comment ledger / census** — nothing is dropped, merged, or rewritten.
- **gaps / blanks / fabrication / swallow / ignore injection** — these perturb comment gaps, blank lines and directives; none measures a column.
- **`corpus:compare:format`** — grades *against prettier*, and on the widest shape prettier emits the over-width line **itself**, so the oracle vouches for the bug.
- **`authoring:audit`** — asks for one fixed point per document, not a good one.

Two real bugs (the mid-run comment weld and its leading twin) shipped an over-width line — one of them also non-idempotent — with `deno task check` green throughout.

```bash
# width_audit - format each seed and report every output line over PRINT_WIDTH,
# rolled up by shape. Pure Rust, no Deno, no instrumentation feature.
cargo run --profile corpus -p tsv_debug --features audits width_audit
cargo run --profile corpus -p tsv_debug --features audits width_audit ~/dev/zzz/src
# Also: --json, --verbose (every line, not the per-shape rollup), --limit N, --update.
# A narrowed run (explicit paths / --limit) reports and exits 0 without grading — the
# snapshot pins the full default run — and says so, so it cannot read as a green gate.
#
# GATED as a RATCHET over `width_audit_known.txt` — a no-new-KINDS gate, NOT a debt
# list. Unlike gaps/blanks, "zero" is not the target: most over-width lines are the
# overruns §Print Width Philosophy sanctions. A shape found but not pinned FAILS; a
# pinned shape that stops firing FAILS.
```

`deno task width:audit:update` regenerates the snapshot; it refuses a narrowed run.

**Seeds are the whole tree, and that is load-bearing.** Measuring `input.*` alone would have caught neither motivating bug: tsv holds the *correct* form stable, so the overrun appears only when formatting an **alternate authoring**. The audit therefore sweeps every file under `tests/fixtures` — `unformatted_*` / `*_variant_*` / `output_prettier.*` siblings included — and **formats** each rather than measuring it as committed. Verified, not assumed: with the mid-run comment fix reverted, all seven extra over-width lines came from `unformatted_ours_compact.svelte` and `divergent_variant_packed.svelte`, and the `input.*` side did not move at all.

**The key is `head…tail`, and the tail half is why it works.** A shape is the language bucket plus how the over-width line opens and ends. The head alone does **not** discriminate — measured against the reverted fix, a head-only key produced *no* new shape, while `head…tail` produced `IDENT…-->` (a long word running into a comment), which is exactly what that bug emitted. Identifiers collapse to `IDENT` so an ordinary rename does not churn the snapshot.

**A third component, `inner`, keeps a weld out of the fattest shapes.** The two ends put a whole comment and two comments *welded onto one line* in the same bucket: `<!-- a -->` and `<!-- a --><!-- b -->` both open `<!--` and end `-->`. That matters because the fattest shape is exactly that one — measured over `tests/fixtures`, `<!--…-->` carries 218 of the 480 over-width lines (45%, across 134 files), and **every one of them is a single whole comment**. So its members are all *forced* overruns (tsv never rewraps a comment interior), and a weld is the only bug the silhouette could ever hide — the same class the trailing-run comment emitters have produced before, and one the ledger, census, F1 and round-trip are all blind to. `inner` records whether a `-->` or `*/` closes before the line does (`-` when none), rendered spliced (`head…-->…tail`). It costs **one** shape over `tests/fixtures` (32 → 33): the three `IDENT…WORD` lines that were mid-line comment glue split off from the 43 ordinary ones, and `<!--…WORD` becomes `<!--…-->…WORD` outright. Neither marker can occur inside the comment it closes, so a whole comment never reads as a weld.

⚠️ **What a non-`-` `inner` means over real code is NOT what it means over `tests/fixtures`, and the difference was mis-stated here before it was triaged.** Over `tests/fixtures` there are **zero** interior closers at all. Over real code (`../svelte/packages/svelte/src` + `~/dev/zzz/src`: 1,255 overruns, 91 shapes) they mint 13 shapes / 24 lines, and reading every one of those lines splits them two ways:

- **9 shapes / 13 lines are minted by a genuine interior comment that is not a weld** — overwhelmingly the JSDoc cast (`… /** @type {T} */ (expr) …`), which really does close a block comment mid-line. `inner` is reporting the truth; it just isn't reporting a bug.
- **4 shapes / 11 lines are the mirror false positive** — a `-->` or `*/` inside a *string*, *template*, *regex*, or the text of a `//` comment, read as interior with no comment involved. Only one of the four is the template-literal case (Svelte's migrator building `<!-- @migration-task … -->` text); string literals, regex literals and comment text produce it too.

Two of those nine are **mixed**, which is the silhouette doing what a silhouette does rather than a defect: one holds a real cast on one line and template-built comment text on another, and one holds both on a single line (a real `*/` cast inside a template whose `-->` is text). So the split above is per-shape, and a shape is not a homogeneous cause.

So the triage note in the snapshot header holds — a new `inner` shape is a **question**, not a verdict — but the likeliest answer on JS is "a JSDoc cast", not "a template built comment text". The gate pays nothing either way: it grades only the default corpus, a run pointed elsewhere reports without grading, and a false one surfaces as a new shape to triage rather than a wrong verdict on a pinned one.

⚠️ **A rejected design worth not re-deriving: the render-time hook.** The tempting version instruments the renderer — a break opportunity *is* a `Line` doc node, so "an over-width line that still held a flat `Line`" needs no lexing and no carve-out list, and forced overruns (a comment, a string, a `<pre>` body: all atoms with no `Line` inside) stay silent by construction. It was built, unit-tested, and **rejected on evidence**: it is blind to exactly the class it was built for. The mid-run comment bug *removed* the break point — it baked the boundary space into the preceding word — so there was no unspent seam to find, and reverting the fix left that check reporting **zero** while the output grew seven over-width lines. A missing seam is invisible to a check that looks for unspent seams. Re-test against a reverted fix before reviving it.

**Blind spots.**

- **Not a bug list.** A pinned shape is a *kind of line that exists*, not a defect. Triage a new one against §Print Width Philosophy before pinning it; the sanctioned overruns are real and numerous (~480 lines over `tests/fixtures`, dominated by fixture prose headers a formatter never rewraps).
- **Shape collision — the residual blind spot, and it is NOT closable by a fourth key component.** A width bug whose line happens to open, close inside, and end like an existing pinned shape passes. The key is a silhouette, not a proof; it catches new *kinds*, and a same-kind regression needs a fixture. The distribution says how concentrated that risk is: the fattest shape holds 45% of the lines and the top three hold 65%, so most of the corpus's absorbing power sits in a handful of buckets. `inner` (above) drains the specific bug class the fattest ones could hide; what remains is a *breakable* line — one with a real seam tsv failed to take — landing on a pinned silhouette. No third component separates that, because **nothing in the finished text distinguishes a seam tsv declined from one it never had** — that is a property of the artifact being measured, not a gap in the key, which is why no amount of further silhouette engineering reaches it. The rejected render-time hook (above) is the design that tried to read the seam instead of the text, and it is blind for its own, worse reason.

  Worked example, found by triaging this audit over real code rather than hypothesized: tsv granted the flat test-call layout to `test('<long name>', (a, b) => { … })` and broke the callback's *parameter list* to chase the width, where prettier keeps the parameters flat — and to two 3-argument shapes prettier's `isTestCall` excludes outright, where prettier breaks every argument out and holds 100 (see [conformance_prettier.md §Print Width Philosophy](./conformance_prettier.md#print-width-philosophy)). Those emitted an over-width line ending in `(`, whose shape is `svelte IDENT…(` — **already pinned**, so the ratchet stayed green on all of it. That is the blind spot behaving exactly as described, and the only thing that reached it was a fixture with a parameterized callback: `test_functions`, the fixture that pins this layout, uses only parameterless ones, so the two cases are now held by [test_functions_params](../tests/fixtures/typescript/expressions/calls/test_functions_params/) and [test_functions_timeout](../tests/fixtures/typescript/expressions/calls/test_functions_timeout/) instead.

  So the honest answer is the one already stated: **a same-kind regression needs a fixture**, and the ratchet's job is new kinds. Read this audit's real-code output as a *triage list* for finding those fixtures — which is what produced the example above — never as a verdict.
- **Vacuous on a corpus with no wide lines.** `FIXTURES_FORMATTED_MIN` guards the file count, not the line count.

## Ignore-Directive Honoring Audit (`ignore:audit`)

The mechanized discovery of unhonored `// prettier-ignore` / `format-ignore` positions (Arm A of the systematic ignore-honoring gap). Recognition is centralized (`tsv_lang::is_format_ignore_directive`), but *consumption* is a per-node opt-in the printer makes at ~15 scattered sites — any position without one silently reformats an ignored construct, which prettier-authored code does not expect. This audit turns the guess-list of suspected positions into a computed ledger, the way `comments:audit` structurally guards the per-site `owned_by_node` model rather than trusting each site by inspection. Design rationale lives in the `ignore_audit` module docs.

```bash
# ignore_audit - inject `// prettier-ignore` before every JS node and grade FOUR checks.
# Per candidate node that leads its line (inside a code_regions JS span), prepend the
# directive on its own line and DOUBLE the node's interior structural spaces
# (reformat-removable; string/template/comment/regex interiors excluded), format, and grade:
#   1 honoring        — the perturbed slice survives verbatim (absent ⇒ UNHONORED at the
#                       node's AST position `{parent}.{field}`);
#   2 stability       — the accepted output formats to ITSELF on a second pass (else
#                       UNSTABLE — the freeze-on-pass-1/inert-on-pass-2 relocation class);
#   3 scope           — the same injection with every structural space OUTSIDE the node
#                       also doubled formats byte-identically to the primary output (else
#                       OVERFROZEN — over-freezing otherwise reads as "honored");
#   4 trailing inert  — the directive appended to the END of the preceding line instead
#                       freezes nothing (else TRAILING_FROZEN — the decided placement
#                       floor: a directive freezes only when alone on its line, no
#                       exceptions; trailing an opening `{`/`[`/`(`/`<` is inert too).
# Checks 2-4 run only on the span-maximal node beginning on each line — the directive
# binds to the OUTERMOST construct beginning there, so a narrower same-line candidate
# would grade that decided wider freeze as a finding. Honoring stays per-candidate.
# A cheaper per-injection battery (<= 4 formats) than blank_audit's F1. Pure Rust, no sidecar.
cargo run --profile corpus -p tsv_debug --features audits ignore_audit   # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits ignore_audit ~/dev/zzz/src
# Also: --json, --report, --jobs N, --limit N, --update. Build with `--profile corpus`
# (optimized + panic=unwind) so a formatter panic is caught + reported.
#
# GATED as a RATCHET (like gap_audit / blank_audit): `ignore_audit_known.txt` pins the known
# findings per (KIND, position), born red — the file shrinking (adding printer opt-ins,
# fixing misbindings/transients) is the goal. A NEW (kind, position) pair, a STALE one (no
# longer fires), or any PANIC (a crash on the injected directive — unpinnable) FAILS. Keyed
# by AST position, so every site of a position rolls up to one line; a "covered" position
# can still appear when an uncovered SUB-PATH of it (e.g. an object property nested in a
# member chain) doesn't honor.
# A Union/Intersection candidate is excluded outright (paren-transparently): Rule A freezes only its
# first member at a single-child head, so the whole-node substring check is the wrong expectation
# there — the member-level positions (`TSUnionType.types` / `TSIntersectionType.types`) carry that
# placement. A closed line at a single-child head (e.g. `TSTypeAliasDeclaration.typeAnnotation`)
# means only its non-composite sites honor; union-typed LIST members (whole-member freezes) also go
# untested by the skip and ride on the member-freeze fixtures.
# Scope: JS positions only (the TS `//` directive over code_regions — standalone .ts/.svelte.ts +
# Svelte <script>/{expr}); CSS `/* prettier-ignore */` and Svelte template `<!-- prettier-ignore -->`
# are a follow-up; whitespace-perturbation only (a quote/paren-only reformat is invisible — Arm B's
# remit); only format fixed points injected.
```

`deno task ignore:audit:update` regenerates the snapshot after adding a printer opt-in (a now-honored position goes stale → drop its line) or fixing a misbinding, over-freeze, or relocation transient; it refuses a narrowed run.

⚠️ **Blind spot: this audit injects a DIRECTIVE, `gaps:audit` injects a COMMENT, and
neither injects the pair — so a bug that needs both is invisible to both.** Live case
(found + fixed 2026-08-10): an object property under `prettier-ignore` whose value carried
a comment inside a *stripped* paren shell (`a: (b /* x */)`) had that comment printed by
the frozen slice AND re-claimed by the trailing-comma seam, so the run **grew by one copy
on every pass** — unbounded duplication, and non-idempotent. Both ratchets stayed green: an
injected directive alone finds no interior comment to duplicate, and an injected comment
alone never freezes. Closing it needs a seed that already carries the comment, which is what
the fixture `expressions/objects/prettier_ignore_property` now does. Until the two
injections compose, **treat a freeze site paired with a comment seam as ungated** and check
it by hand — the anchor rule is `Printer::element_claim_anchor`.

## Build-Fanout Audit (`fanout:audit`)

```bash
# build_fanout_audit - guard the O(1)-doc-builds-per-source-node invariant. A
# builder that assembles `conditional_group` candidates by RE-INVOKING the recursive
# builder on the same nodes — instead of building the subtree once and reusing the
# DocId — grows the doc-node count exponentially in nesting depth (hang/OOM on a
# deeply-nested but ordinary file). Builds synthetic nested inputs across six axes
# (svelte elements / {#if} / {#each} / {#await} / sibling-`>` dangle, ts member
# chains) at increasing depth and fails if the doc-node count grows faster than
# ~depth^3. Deterministic, pure Rust, no Deno. Exits 1 on any super-linear case.
cargo run -p tsv_debug build_fanout_audit
# Also: --json. Gated in `deno task check` via the `fanout:audit` task.
```

**What it is blind to.** The audit measures how the doc-node count *grows with depth*, so it
sees only violations that **compound**. Two shapes escape it:

- **A flat constant factor.** An eager build a later branch discards — `let x = build_…(…)`
  bound before a `match`/`if` whose arms don't all use `x` — costs a fixed 2× at that one site
  and never compounds, so no depth curve can separate it from the baseline. The tell is
  syntactic, not empirical; grep the shape rather than waiting for a profile, since a single
  site sits below the noise floor of a corpus measurement while still being free to remove.
- **Breadth.** A quadratic over a flat sibling sequence (`[..i].iter().any(…)` inside a child
  loop) is likewise structurally invisible — it grows with sibling count, not nesting.

Both are guarded by review and grep, not by this audit. The known open instance of the first is
a parenthesized binary chain base; see the `chain/printing.rs` note in the perf queue.

## Raw-Find Scan Audit (`scan:audit`)

```bash
# scan_audit - guard against new raw position-anchoring substring scans over
# source. A raw `self.source[..].find(delim)` can match the glyph inside an
# enclosed comment/string and drop content (the "Comment-Aware Delimiter Scans"
# bug class); the fix is the trivia-aware cursor (`tsv_lang::source_scan`).
# Flags every `find`/`rfind`/`match_indices`/`rmatch_indices` (non-closure pattern)
# in the four language crates and fails on any not in the reviewed, categorized
# in-code allow-list (ALLOW). A new scan must move onto the cursor or be consciously
# allow-listed; a migrated/reformatted scan must drop its now-stale entry (the list
# mirrors the live sites exactly). Pure Rust, no Deno.
cargo run -p tsv_debug scan_audit            # audit (exit 1 on any violation/stale)
cargo run -p tsv_debug scan_audit --list     # enumerate every scan site
# Also: --json. Gated in `deno task check` via the `scan:audit` task. Out of scope:
# closure `.find(|…|)` (iterator/predicate), counting/existence checks, and hand
# byte-loops (the cursor is their sanctioned home).
```

## Self-Format Audit (`format:audit`)

```bash
# tsv formats its own TS/JS. Runs `tsv format --check .` over the repo, so the
# formatter's own output on real, non-fixture source is a standing gate.
deno task format:audit
```

**What it proves.** Two things at once, and it fails on either:

- **exit 1 — a would-change file**: the committed tree is not a `tsv format` fixed
  point. Because the tree is committed formatted, this also makes the audit an
  **idempotency** check on real code: a non-idempotent file can never be committed
  clean, so it shows up here rather than as silent churn.
- **exit 2 — a parse error**: tsv rejects a file it must be able to read. This is
  the signal with no other home — every over-rejection the repo's own TS has hit
  was found here first.

**Why it is not redundant with the corpus gates.** `corpus:compare:format` and the
conformance gates read **other** repositories (framework checkouts, prettier
suites, the live dev repos). None of them read tsv's own `benches/js`, `scripts`,
or `crates/**/npm` sources, so nothing else covers this tree.

**Scope.** The root `.formatignore` prunes `tests/fixtures/` and
`tests/fixtures_compile/` — those files are DATA whose whole claim is the state
they are committed in, and many are deliberately not format fixed points. A
directory argument can never step past that; only an explicitly named FILE bypasses
the ignore files, which is why nothing under `tests/` may ever be named directly.
Markdown and JSON are out of scope entirely — they stay hand-maintained.

**Blind spots.** It only sees shapes this repo's own source happens to contain, so
it is a dogfooding tripwire, not a survey — a formatter bug in a construct tsv's
own TS never writes stays invisible here. The injection audits (`gaps:audit`,
`blanks:audit`) and the corpus gates are the discovery arms.

**Build world.** Runs `tsv_cli` under `--profile corpus`, the single build world
every `deno task check` audit shares, so it adds no separate compile.

## Doc-Link Audit (`docs:audit`)

```bash
# rustdoc over the whole workspace with the doc lints denied. No artifact is
# consumed — the run itself is the assertion.
deno task docs:audit
```

**What it proves.** That every `[`path`]` in a doc comment still names something that
exists. A doc link is the only claim a doc comment makes that a machine can check, so an
unresolvable one is not a formatting nit — it is a **stale doc**. Every occurrence found
when this gate was introduced was a rename that left its back-references behind:

- a struct field doc'd as mirroring `ElementContext::block_flow_multiline`, a field that
  had never existed on that struct (the value is a local, cached for two readers);
- `stripped_redundant_paren_leading_line_comments`, renamed to
  `stripped_redundant_paren_member_leading_run`, still named by two callers — one of
  which also mis-described how the two forms differ;
- `ControlFlowGap::BlockToKeyword`, an enum that exists nowhere; the real thing is
  `push_block_to_keyword_gap`, called twenty lines below the comment naming it.

None of those were reachable by any other gate. They were found only because clearing the
27 accumulated broken links made them visible — which is the argument for denying rather
than warning: a pile of known-broken links is precisely what hides the next one.

**Why `--document-private-items`.** All three staleness findings above were on **private**
items, which a default `cargo doc` never checks. This codebase puts its design rationale
on private functions, so the private-items build is both the one maintainers read and the
only one where the gate has teeth.

**Why the whole crate has to reach it — two silent coverage holes.** `cargo doc` documents a
crate's *targets*, and a bin target whose name collides with its lib's is skipped outright.
`tsv_debug` had exactly that shape — a `[[bin]] name = "tsv_debug"` beside an implicit lib of the
same name — with `audit/` and `cli/` declared only in `main.rs`. Those two trees are ~36k of the
crate's ~56k lines, so the gate graded roughly a third of it and reported green; a deliberately
broken link in either passed. The crate is now one lib, with `main.rs` a shim over `cli`, which
also stops the nine shared modules compiling twice (they were declared in both roots). The second
hole is `#[cfg(feature)]`: five `audit` submodules sit behind `comment_check`, so a default-feature
build cannot resolve a link that names them — hence `--all-features`, which reaches gated code in
every crate, not only this one. Both holes fail the same way, which is the thing to remember about
this gate: **absent code is indistinguishable from clean code**, so its coverage is a claim about
what got compiled, not about what is in the tree.

⚠️ **A link in a module's `//!` docs resolves in the parent module's scope, not the module's own.**
A `super::` path in an inner-doc block therefore means one level higher than the same path written
in a `///` beside it, and a name a `use` brought into the module is not in scope. Write those as
absolute `crate::` paths. A private item of a *sibling* module is nameable by no path at all — drop
the link and leave plain backticks.

**Lints, and one deliberate exemption.** `broken_intra_doc_links` (the staleness detector),
plus three mechanical ones that keep it legible — `invalid_html_tags` (prose like
`<script>` or `Vec<Doc>` that rustdoc reads as markup), `bare_urls`, and
`redundant_explicit_links`. `private_intra_doc_links` is **allowed on purpose**: public
docs here routinely link private items, and under `--document-private-items` those links
resolve and navigate, so satisfying that lint would replace working navigation with inert
backticks in exactly the build that matters.

**Where the lints live.** `[workspace.lints.rustdoc]` in the root `Cargo.toml` carries them
for day-to-day work (IDE, a bare `cargo doc`) along with the rationale. The task re-states
them as `RUSTDOCFLAGS` because `tsv_ffi` and `tsv_napi` define their own `[lints.clippy]`
tables, which Cargo cannot merge with `[lints] workspace = true` — so they do not inherit,
and only the task's flags cover all fourteen crates.

**Blind spots.** It checks that a link *resolves*, never that the surrounding prose is
true. A doc that describes behavior the code no longer has passes cleanly; so does a
correct link attached to a wrong claim. Two of the three findings above came with prose
errors *beside* the dead link, and those needed reading, not the gate.

**Build world.** `cargo doc` builds its own rmeta; it does not share the `--profile corpus`
world the other `check` audits use, so it adds a short compile of its own.

## Wire-Type Drift Check (`check:ast-types`)

```bash
# scripts/check_ast_types.ts — `tsv parse` a curated sample set, embed each JSON
# output as a typed literal in a generated TS file, `deno check` it.
deno task check:ast-types
```

**What it proves.** That `crates/tsv_wasm/types/tsv_ast.d.ts` still describes what the
converter emits. That `.d.ts` is hand-maintained and it **ships** — `@fuzdev/tsv_parse_wasm`
bundles it, so it is the wire contract consumers type against, and nothing in the Rust build
knows it exists. TypeScript's excess-property checking on the generated object literals catches
both directions of drift: a field the converter emits that the `.d.ts` lacks ("may only specify
known properties"), and a field the `.d.ts` requires that the converter does not emit
("Property 'X' is missing"). Renames and value-type changes fall out the same way.

**Blind spots.** Coverage is the sample list, and it is small **by design** — each sample costs
a parse invocation, so the goal is structural coverage, not fixture-style exhaustiveness. A
node no sample reaches can drift freely; the per-field checklist in
[crates/tsv_wasm/CLAUDE.md §TS Type Maintenance](../crates/tsv_wasm/CLAUDE.md#ts-type-maintenance)
is what carries a writer change, and this gate is the backstop for the fields a sample happens
to touch. Add one when an uncovered node regresses. It also asks only whether the two sides
AGREE — never whether the shape they agree on is the one acorn / `parseCss` / Svelte actually
produce, which is the parse conformance gates' remit.

## Canonical-Pin Agreement Audit (`pins:audit`)

```bash
# scripts/check_canonical_pins.ts --pins. Read-only Deno, no build, no sidecar.
deno task pins:audit
```

**What it proves.** That the canonical oracle is *one* version rather than five. The five
pin sites that must be identical:

1. `sidecar.ts` `VERSIONS` — what `tsv_debug check` reports;
2. `sidecar.ts` static `npm:` import specifiers — what the sidecar actually runs;
3. `benches/js/package.json` `dependencies` — what the bench and conformance gates run;
4. `actor.rs`'s `deno_config` acorn import-map pin — the shared-acorn-instance pin;
5. the sidecar lockfile `crates/tsv_debug/src/deno/deno.lock` — what deno ACTUALLY
   resolves, so a literal the lock disagrees with is a lie.

Drift here silently grades fixtures and corpora against a different oracle than the bench
measures.

**Why it gates early in `deno task check`.** It is a **repo fact**: nothing outside the repo
can change the verdict, so it holds on a clean checkout — and its failure invalidates the
fixture grading `cargo test` does later in the same chain, which is the cheapest thing to
learn first.

**`LOCKED_TRANSITIVE` — the pin with no sibling.** The lockfile also pins what no literal
names: the oracle's own transitive dependencies, which float on THEIR declared ranges. Today
that is `esrap`, the printer that emits the JS `svelte.compile()` returns and therefore the
effective oracle for every compile fixture — which svelte depends on as `^2.2.12`, a caret.
Before the lockfile the compile oracle's output could change with no version in this repo
changing and no pin site able to see it, which is exactly what happened: esrap 2.3.1 stopped
dropping a string-literal specifier's `as` alias and silently staled five committed fixtures
while `deno task check` stayed green. Bumping one is a deliberate oracle move — regenerate the
lock, re-run `deno task compile:fixtures:validate`, update the constant, all in one change.

**Blind spots.**

- **Agreement is not correctness.** It proves the sites say the same thing, never that the
  thing they say is right or current. A wrong version pinned five times passes.
- **`LOCKED_TRANSITIVE` names only `esrap`.** Every other transitive dependency is pinned by
  the lock but unreviewed — a regeneration moves them silently unless someone diffs the lock.
- **It cannot see oracle freshness.** A pin can be perfectly self-consistent and still grade
  against stale committed expectations. That is `compile:fixtures:validate`'s job — the only
  check that grades the committed expectations against a LIVE oracle — which is why `deno task
  conformance` preflights it.

**Maintenance counterpart: `deno task pins:lock`** (`scripts/regen_sidecar_lock.ts`), not an
audit. The lock is frozen at runtime, so this is the only way it moves. `--check` reports
drift without writing; `--allow-fresh` opts past deno's 24 h `minimumDependencyAge`
supply-chain window, needed ONLY to take a version published in the last day (a lock made with
it reproduces flag-free once that version ages out). `--check` is deliberately **not** gated:
its verdict depends on what the registry offers at that moment, so it would go red the day
upstream publishes — drift to decide about, not a broken build.

## Checkout-Alignment Audit (`pins:audit:checkouts`)

```bash
# the same script's other mode (--checkouts). `--allow-run=git` is load-bearing:
# without it every checkout reads as absent and the drift half is silently inert.
deno task pins:audit:checkouts
```

**What it proves.** That this machine's `../svelte` and `../acorn-typescript` clones are the
version the pins say they are. The fixtures gates grade INPUTS from those suites with the
PINNED npm parser, and their SANCTIONED / KNOWN_GAPS ledgers are path-keyed against them, so a
skewed checkout silently grades different inputs than the oracle version defines (and rots the
ledgers). An ABSENT checkout is skipped with a note; a PRESENT-but-mismatched one FAILS. This
is the hard half of the guard — the gates themselves only WARN on skew and catch it indirectly
via their exact `scanned` count pins, which a skew that happens not to move the count slips
past. `../prettier` is deliberately not gated: its suites' expected output is computed live per
file (no path-keyed ledger to rot) and the checkout legitimately rides `-dev` versions —
`doctor` reports it instead.

**Why it is NOT in `deno task check`.** It is an **environment fact**, and nothing in `check`
reads those clones — so a skew there cannot change a committed-tree verdict, and failing on it
would be pure collateral damage: the chain halts and the real regressions go unrun. It
preflights `deno task conformance` instead, whose legs do read them; `doctor` reports both
modes ahead of time. Passing neither flag runs both, which is what `doctor` does.

**Commit drift is warn-only, by design.** A version string only bumps at release, so upstream
commits landing in between change a graded suite with no version signal at all — precisely how
the count pins went stale unnoticed. Each checkout's HEAD is compared against the commit
`GATE_CHECKOUT_COMMITS` records it was measured at ([gate_counts.md](gate_counts.md)) and a
move is reported, never failed: the count pins are the gate, this is the diagnosis, so when one
trips "the corpus moved" is distinguishable from "tsv regressed" at a glance rather than by
reverse-engineering.

**Blind spots.** Absent or non-git checkouts are skipped, so a machine without the clones
passes vacuously. Alignment is a version-string comparison — it says the checkout is the pinned
*release*, never that the pin is the right one — and, like
[`pins:audit`](#canonical-pin-agreement-audit-pinsaudit), it cannot see whether the committed
expectations that oracle produces are still fresh.

## Authoring-Independence Audit (`authoring:audit`)

```bash
# authoring_audit - probe whether the SAME logical document, authored with
# different boundary whitespace, formats to ONE tsv fixed point. Stronger than the
# corpus idempotency sweep: a formatter can be idempotent yet authoring-DEPENDENT
# (two authorings settling on two different stable outputs). Two mutation families,
# never a blank line (Tier-1 significant) and never inside <pre>/<textarea>:
#   - BETWEEN siblings — space↔single-newline only. Inter-node whitespace is
#     render-SIGNIFICANT (it collapses to one space, it doesn't vanish), so the run is
#     reshaped, never created or destroyed. Both forms collapse identically ⇒ safe.
#   ⚠️ The audit's own whitespace class is `is_collapsible_ws_char` (`[ \t\n\r]`), never Rust's
#   `is_ascii_whitespace`. Every site it finds is spliced over WHOLESALE, so a character it
#   wrongly counts as whitespace is one the mutation DELETES — the mutant is then a different
#   document, and its second fixed point is a fact about the instrument rather than about the
#   formatter. With the wide class a form-feed-bearing document reported 11 spurious
#   dual-stable sites where its NBSP twin (the same document, same classification) reported 2.
#   - At a tag's CONTENT BOUNDARY — hug↔space↔newline, i.e. the run IS created and
#     destroyed. Svelte 5 removes start/end-of-content whitespace at compile, so all
#     three authorings render identically. This is the family that catches a formatter
#     letting a render-free character pick the layout (the delimiter-dangle class).
#     Fits-inline content is probed too — tsv trims a render-free boundary run even when
#     the content fits (`<span> text </span>` → `<span>text</span>`, the Svelte-mirror
#     trim; fixture `inline_boundary_whitespace_prettier_divergence`,
#     conformance_prettier_svelte.md
#     §Svelte: Inline content block-style), so hug↔space↔newline reach ONE fixed point at
#     every content boundary outside pre/textarea. Sanctioned residual: a BOTH-side
#     newline-authored boundary around an ELEMENT child keeps its multiline layout
#     (newlines are intent; text-only content glues regardless — width alone decides), so
#     its single-boundary mutants settle glued — reported dual-stable, deliberate.
# The element expansion a mutation may trigger is the property under test. Svelte only.
# Gated in `deno task check` via the `authoring:audit` task — which scans tests/fixtures
# ONLY, so point it at a real codebase too: findings live there (a non-idempotent fill
# 2-cycle was green on fixtures while failing on ~/dev/zzz).
cargo run -p tsv_debug authoring_audit                  # audit tests/fixtures (pure Rust)
cargo run -p tsv_debug authoring_audit ~/dev/zzz/src    # audit a real codebase
# Pure-Rust verdict per site: converge / diverge (dual-stable) / diverge
# (NON-IDEMPOTENT); exits 1 on any non-idempotency — site-level, and also a
# base-non-idempotent FILE (one whose own format isn't a fixed point). Such a file
# is excluded from the authoring analysis (its fixed point is undefined, so the
# converge/diverge verdict would be meaningless), but the exclusion is not a reason
# to pass the run — that is how a whole-file reflow could sit here reported-but-green.
#
# --prettier adds sidecar triage:
# (a) tsv diverges where prettier converges (bug); (b) tsv converges where prettier
# diverges (a _prettier_divergence to pin, the space_after_block class); (c) both
# diverge (sanctioned, e.g. Tier-2 element expansion). --dump-dir writes byte-exact
# repro artifacts per hard finding — the basis for a fixtures-first fix.
# Also: --json, --verbose, --limit N (sites/file), --examples N.
cargo run -p tsv_debug authoring_audit ~/dev/zzz/src --prettier --dump-dir /tmp/audit
```

## Format→Reparse Round-Trip Audit (`roundtrip:audit`)

```bash
# roundtrip_audit - corpus-scale "does format(src) reparse to the SAME document?".
# Catches the class the other gates can't see: output that mis-delimits but loses no
# characters (attr='a"b' → attr="a"b", `+(+x)` → `++x`) — corpus:compare:format's
# SAFETY is char-frequency, BLIND to delimiter/structure corruption. Two phases
# (tsv-self pre-filter → canonical confirm via sidecar): parse input and formatted
# output, reduce each to a STRUCTURAL SKELETON (node-tree shape + `type`, erasing
# reformattable leaf scalars + acorn `extra`), compare — so legit reformatting
# doesn't read as corruption. Buckets: {tsv,canonical}_unreparseable (the prize —
# output the parser rejects) and {tsv,canonical}_divergent (structural change).
# Zero false positives on real formatted code; point it at the delimiter-dense
# prettier suites for the work-list.
cargo run -p tsv_debug roundtrip_audit                              # audit tests/fixtures
cargo run -p tsv_debug roundtrip_audit ../prettier/tests/format/js ../zzz/src
# --gate fails ONLY on the *_unreparseable buckets (the reliable half — divergent is
# render-model noise over tests/fixtures). Bare --gate runs phase 1 only via a
# reparse-only fast path (pure Rust, no sidecar) — the `deno task roundtrip:audit`
# check gate; a cheap tripwire over tests/fixtures, real yield on external corpora.
# --canonical-all confirms every file (also guards canonical_unreparseable: tsv's
# parser accepting output the real parser rejects).
cargo run -p tsv_debug roundtrip_audit --gate                       # the check gate (pure Rust, tests/fixtures)
deno task roundtrip:audit:prettier                                 # the check gate's second scope (the prettier suites)
cargo run -p tsv_debug roundtrip_audit --gate --canonical-all ../prettier/tests/format  # thorough
# Also: --no-render, --verbose (AST diff per finding), --limit N, --json. The full
# (non-gate) run is a diagnostic — the divergent bucket over tests/fixtures is
# Svelte-reflow-noisy vs render_normalize's simpler whitespace model.
cargo run -p tsv_debug roundtrip_audit --canonical-all --verbose ../prettier/tests/format/typescript
```

### Two scopes in `check`, and why the second one is opportunistic

`deno task check` runs the audit twice: over `tests/fixtures` (`roundtrip:audit`) and over the
pinned Prettier format suites (`roundtrip:audit:prettier`, ~2,350 files in ~0.1 s on the binary
the first leg already built). The second scope is not redundancy — it is the only corpus in
`check` that is **not format-stable**. The fixture tree cannot contain the input shape that
triggers a valid→unreparseable regression, which is how a statement-head paren strip
(`for ((let) of foo);` → `for (let of foo);`, output tsv's own parser rejects) sat behind a green
`check` for a whole PR while three prettier-suite files caught it on the first run.

Its corpus is a **sibling checkout**, so it is read opportunistically: present ⇒ it gates,
absent ⇒ a loud `NOT RUN` line and exit 0. That keeps `check` runnable on a bare clone (the CI
`check` job has no sibling checkouts at all), and it is sound here in a way it would not be for
a count-pinned gate — this audit asserts an invariant, so a finding fails wherever it occurs and
a smaller corpus can only cost coverage, never soften a verdict. A *partial* `../prettier` (the
checkout exists but a listed suite does not) warns per suite and audits the rest. The suite list
is shared with [the corpus bundle](#the-corpus-bundle-auditcorpus) from
`scripts/roundtrip_audit_prettier.ts`, so the cheap leg and the release-cadence one cannot drift.

## Comment↔Token Binding Audit (`binding:audit`)

```bash
# binding_audit - does format re-bind a FORWARD-binding comment to a different
# subtree? Two comment kinds bind to the token AFTER them: a JSDoc type cast
# (`/** @type {T} */ (x)` — the parens + comment ARE the cast) and a bundler
# annotation (`/* @__PURE__ */ f()` — marks the call side-effect-free). A paren
# migrating across such a comment under formatting silently re-binds it (a cast
# annotating a wider node, an annotation gone inert). This class is INVISIBLE to
# every other gate — neither a cast, a grouping paren, nor an annotation is a
# public-AST node, so both forms serialize to byte-identical wire JSON: ast_diff
# says "equivalent", roundtrip_audit's skeleton can't see it, corpus SAFETY is
# char-frequency (the characters only MOVE). Pure Rust, no sidecar.
#
# Signal: reparse input + tsv-formatted output with `preserve_parens` (grouping
# parens become ParenthesizedExpression nodes), and per glued comment compare the
# bound subtree. A cast stays invisible even so (its JsdocCast node emits its bare
# inner), so the audit anchors INSIDE the cast's `(`. And since the only structural
# delta formatting can add under preserve_parens is a clarity-paren (roundtrip_audit
# gates the rest), the skeleton is compared with ParenthesizedExpression STRIPPED —
# the binding-paren signal rides a separate `anchor_is_paren` flag. So a clarity
# paren deep inside is not a finding; a GROUPING paren at the anchor is.
#
# ⚠️ Both `(` tests ask for a NODE, never for the byte. An arrow's parameter list
# also opens with `(` and is the arrow's own syntax, so `/* c */ x => x` ->
# `/* c */ (x) => x` (`arrowParens: always`) binds the same ArrowFunctionExpression
# either way, and a cast-shaped comment on a bare parameter is not a cast. The byte
# test called both re-bindings and reported HARD on ordinary JS
# (`arr.map(/* index unused */ x => x.id)`). "No node begins at the `(`" is NOT the
# cast test either: a real cast's parens are swallowed by whatever encloses it, so
# `(root).head` has a MemberExpression starting right there. Only an arrow begins
# at its own parameter `(`.
#
# HARD (a parser-owned glued comment re-binds) fails --gate — every glued block
# comment is owned, so a cast, an annotation, and a plain glued comment alike; SOFT
# (an unowned glued block comment, now rare) is informational. TS-family files
# only (.ts/.js/.mts/.cts/…); casts/annotations concentrate in JSDoc-typed JS.
cargo run -p tsv_debug binding_audit                                  # audit tests/fixtures
cargo run -p tsv_debug binding_audit ../svelte/packages/svelte/src ../prettier/tests/format/js
cargo run -p tsv_debug binding_audit --gate                          # the check gate (HARD only)
# Also: --verbose (in→out bound-subtree per finding), --limit N, --json. A bare
# --gate over tests/fixtures is a cheap tripwire (fixtures are format-stable); the
# real yield is external corpora, where JSDoc casts + annotations are dense.
cargo run -p tsv_debug binding_audit --verbose ../svelte/packages/svelte/src
```

## Render-Equivalence Audit (`render:audit`)

```bash
# render_audit - corpus-scale "does `tsv format` change what a Svelte component
# RENDERS?". Per .svelte file: compare the browser-visible RENDER KEY of the source
# against the render key of format(source). The key is `svelte compile --generate
# server` reduced to its visible render (baked template text, `${…}` holed out,
# <script>/<style>/comments stripped, whitespace collapsed with block-boundary
# whitespace dropped) — equal keys prove equal renders, and a <script>/<style>
# reformatting that leaves the template alone is correctly ignored.
#
# This is the CORPUS-SCALE arm of the fixture render-equivalence check (the R rules
# in `fixtures:validate`). Those gate a CURATED corpus whose whitespace variants are
# hand-authored to be render-equivalent — a regression guard, close to the least
# likely place for a render change to hide. Real code is the exposure, the same gap
# `audit:corpus` exists to close for the content-loss class.
#
# Invisible to every other gate: corpus:compare:format's SAFETY is char-frequency
# (blind — the characters only MOVE), roundtrip_audit's structural skeleton erases
# the very whitespace that carries the meaning, and authoring_audit asks the
# CONVERGENCE question (do two authorings reach one fixed point), never whether that
# fixed point renders like the input.
cargo run --profile corpus -p tsv_debug --quiet render_audit ~/dev/zzz/src
deno task render:audit ../svelte/packages/svelte/tests   # (--gate baked in)
# Also: --gate (exit 1 on findings), --json, --limit N (0 = unlimited, as in every
# other audit). Needs the Deno sidecar, so
# NOT in `deno task check` — and not in the pure-Rust `audit:corpus` either. It is
# release-gated as a leg of `deno task conformance` (the one leg that runs as a
# subprocess), scoped there to the version-pinned `framework` + `suite` checkouts so
# a live working tree can't move a release verdict; run it standalone on any corpus
# after a printer change. Files whose format is a no-op skip the ORACLE (trivially
# render-equal by identity) but still carry a verdict, so they count toward the
# vacuity floor; files Svelte's semantic ANALYZER rejects are counted as
# compile-blind (that arm cannot speak there) and do not. The in-repo, any-corpus form of
# ../test-svelte-prettier-whitespace/whitespace-safety-check.mjs.
```

## Layout-Neutrality Audit (`neutrality_audit`)

```bash
# neutrality_audit - does a comment's OWNERSHIP ever change tsv's layout? An owned
# comment must occupy exactly the page space a same-width ordinary comment does — a
# layout gate that instead SKIPS owned comments (asks the to-emit question where it
# should ask on-page) goes blind, and the comment silently changes the layout it
# should have forced. At each glued block-comment position, format the file with the
# comment made OWNED (annotation-shaped) and made ORDINARY (plain filler) at the SAME
# width — only ownership varies, so any layout difference is a gate reading ownership.
# Pure Rust, no sidecar. A development / characterization tool, NOT a `deno task
# check` gate: it needs an owned/ordinary CONTRAST to detect anything, and under the
# "every glued block comment is owned" rule a run passes vacuously — its moment is
# BEFORE any future ownership-rule change (run it then, over external corpora).
# TS-family files only; defaults to tests/fixtures.
cargo run -p tsv_debug neutrality_audit ../svelte/packages/svelte/src
# Also: --gate (exit 1 on findings; dev-loop convenience), --verbose (the
# owned-vs-ordinary output diff per finding), --limit N, --json.
```

## Seeded Mutational Fuzzer (`fuzz:audit`)

```bash
# fuzz - dep-free seeded mutational fuzzer (the coverage-trifecta fuzzing leg). A
# SplitMix64 PRNG + byte-level mutation operators (plus multi-byte inserts: a
# unicode span/width stress set — NBSP/zero-width/BOM/combining/CJK/emoji/CRLF —
# and a structure-bearing token dictionary aimed at the parser's ACCEPT paths)
# over a seed corpus (default tests/fixtures); every valid-UTF-8 mutant is driven
# through parse+format+reparse under catch_unwind. Asserts three properties
# nothing else guards on ARBITRARY input: (1) no panic — the parser must never
# crash (prod WASM is panic=abort → a panic is a DoS; the corpus profile only
# catches panics on real code); (2) format idempotency (the F1 fixed point);
# (3) structural reparse (reusing roundtrip_audit's skeleton compare).
# Deterministic per --seed + corpus — and CORPUS-ADD-STABLE: each seed file draws
# mutants from its own path-keyed PRNG stream, scheduled round-robin, so a
# fixture add/remove/rename changes only that file's mutants (every other stream
# is byte-identical; a shrunken per-file budget trims a stream's tail, never
# rewrites it). Pure Rust, no sidecar. Not the differential (tsv-vs-canonical) leg.
# The `fuzz:audit` deno task (fixed --seed 0 --iterations 5000 over tests/fixtures) is
# gated in `deno task check` — a cheap standing tripwire for the three invariants.
#
# Hangs can't be caught in-process (the exponential-rebuild class), so two
# tripwires: every attempt's input is written to a last-input repro file BEFORE
# the attempt (path printed at startup; removed on a clean exit — a killed hung
# run leaves its exact input on disk), and attempts over --slow-budget-ms
# (default 2000) are reported, never fatally.
#
# TWO passes. Pass 1 drives every seed file AS AUTHORED (unmutated), pass 2 the
# mutants. The pristine pass matters because the corpus is the richest source of
# real, formatter-reachable inputs — and over tests/fixtures it is the ONLY gate
# that drives the non-`input.*` fixture files: the validator claims F1 on `input.*`
# alone, so `output_prettier.*` / `variant_*` / `unformatted_*` (all real code)
# were never themselves formatted twice. A pristine seed's *soft* verdict does not
# FAIL the run (the corpus deliberately holds mis-formatted `unformatted_*` files whose
# reflow is the point) but IS reported, with paths — over a real-code corpus there are
# no such files, so each wants triage, and the seed path is itself the repro (an
# unmutated file on disk), so it is listed rather than dumped. HARD verdicts fail.
cargo run -p tsv_debug fuzz                                    # 2000 iters over tests/fixtures
cargo run -p tsv_debug fuzz --seed 7 --iterations 20000 --evolve --minimize --dump-dir /tmp/fz  # discovery
cargo run -p tsv_debug fuzz --iterations 0 ~/dev/zzz/src       # pristine pass only = an F1 sweep
# HARD findings (exit 1): panic / unreparseable / non_idempotent / format_error —
# always real bugs. SOFT findings (reported, non-fatal): structural_divergence — the
# render-model-noisy bucket that needs canonical confirmation (roundtrip_audit
# --canonical-all), like roundtrip_audit --gate. --strict fails on soft too.
#
# Discovery aids (both opt-in, off in the gate): --evolve feeds every mutant that
# passes all invariants back into the seed pool (bounded at 2× the initial corpus)
# so later mutants walk deeper into the ACCEPTED-input space — the formatter's
# coverage, since a mutant must parse before F1/reparse grade anything; --minimize
# ddmin-shrinks each stored HARD finding (greedy chunk removal while the same
# outcome reproduces, bounded probes) into a consumable repro before report/dump.
# Also: --parser not applicable (per-file extension), --max-mutations N, --limit N,
# --max-findings N (HARD only), --slow-budget-ms N, --json.
```

## F1 Idempotency Sweep (`idempotency:sweep`)

The fuzzer's pristine pass, pointed at the `perf` corpus view (the sibling dev repos + upstream framework source) — `format(format(x)) == format(x)` on every real file. NOT in `deno task check`: the corpus is machine-dependent checkouts and the sweep is minutes, not seconds. It is a different risk surface from the fixtures — a formatter can be idempotent on every curated fixture and still reflow a real component on pass 2. Run at conformance cadence, or after any printer change.

```bash
deno task idempotency:sweep
# Absent corpus checkouts are skipped with a warning (not a failure); builds with
# `--profile corpus` (optimized + panic=unwind) because the fuzzer needs catch_unwind.
```

## The Corpus Bundle (`audit:corpus`)

The standing content-loss / robustness gate over REAL code — the extension-robustness bar that `deno task check`'s fixture-only scope is structurally blind to: `roundtrip_audit --gate` + `comment_audit` + `swallow_audit` + `binding_audit --gate` (real gating; prettier suites report-only) + `authoring_audit` + `census_audit` + `fabrication_audit` + `fuzz --iterations 0`, over the `perf` corpus view + the pinned prettier suites. Pure Rust; absent dev repos warn-skip (floor = `../svelte` src). NOT in `deno task check` (machine-dependent corpus, minutes); wired into publish Step 3c alongside conformance:all's SAFETY. Run at conformance/release cadence or after a printer change. See ../benches/js/CLAUDE.md §Gate map.

```bash
deno task audit:corpus
```

**Two of the three as-authored ratchets are legs here; the third cannot be.** `census_audit` and `fabrication_audit` both assert a **zero** — off their default corpus the snapshot is not consulted and `grade_narrowed_strictly` fails every finding, pinned or not — and the corpus currently holds that zero over all ~5,800 files, at about the cost of a leg already in the bundle. The census in particular is the leg whose own module doc names external corpora as its discovery arm, so its absence was a hole rather than a policy.

`width_audit` stays out **structurally**, not by omission: it has no zero to grade against (the sanctioned overruns are everywhere, which is why a narrowed run reports and exits 0 by design). Gating it here would need a second snapshot pinned over this corpus — and that corpus is the LIVE dev repos, so the snapshot would churn with ordinary work: over `../svelte/packages/svelte/src` + `../zzz/src` alone, 83 of 91 shapes are absent from the committed fixtures snapshot and 25 pinned shapes never fire. That is the re-pin treadmill the format count pins escaped by moving to the reproducible subset ([gate_counts.md](gate_counts.md)). A real-code width run stays a **triage** command.

## Differential Lexer Harness (`lex_diff`)

```bash
# lex_diff - differential lexer harness: snapshot the raw token stream over a
# corpus and diff against a golden to prove token-stream identity (kind, start, end,
# decoded per token) after a lexer change — stronger than format byte-identity.
# Covers the context-free next_token dispatch for the TypeScript family
# (.ts/.mts/.cts/.js/.mjs/.cjs, .svelte.ts and .d.ts included — the whole family
# dispatches to one lexer) plus .css.
# Pure Rust, no Deno.
cargo run -p tsv_debug lex_diff ~/dev/zzz/src --golden /tmp/lex.golden --write  # capture golden
cargo run -p tsv_debug lex_diff ~/dev/zzz/src --golden /tmp/lex.golden          # check against it
# Options: --write (capture instead of check), --verbose (first divergent line per file)
```

## Conformance Audit (`conformance:audit`)

```bash
# conformance_audit - doc/fixture integrity in one fixture walk. Five checks:
#  (1) Orphans - every divergence-suffixed fixture must be linked in its conformance doc
#      (_prettier_divergence → any docs/conformance_prettier*.md, _svelte_divergence →
#      docs/conformance_svelte.md, _svelte_prettier_divergence in both). The glob is a
#      hand-listed constant held to it by check 5.
#  (2) Dead links - every Markdown link (relative path + #anchor) in every Markdown file
#      in the repo (walked at run time, so a new doc is gated by existing) must resolve on
#      disk (catches renamed/deleted fixtures, wrong ../ depth, stale anchors). That is
#      docs/*.md, every fixture README, and the set that previously had no link gate at
#      all: root CLAUDE.md / README.md, each crate's CLAUDE.md, the shipped
#      crates/tsv_wasm/README_*.md, a container-directory README under tests/fixtures/.
#      The walk shares `tsv format`'s prune policy — tsv_discover's safety nets
#      (node_modules, .git, .sl, .hg, .svn, .jj) over tsv_ignore's IgnoreStack — so build
#      output and the per-machine *.local.* / *.tmp conventions are pruned by the repo's
#      own rules rather than a second copy of them, and a gate can't fail over content the
#      repo doesn't have. It stops short of classify_dir: the build-output heuristic and
#      the tsv layer are format-file policy, and .formatignore prunes tests/fixtures/ —
#      the fixture READMEs are exactly what this must check. Symlinks are skipped
#      (AGENTS.md points at CLAUDE.md; following both would report every finding twice).
#      External URLs and targets that climb out of the repo (sibling checkouts,
#      machine-dependent) are out of scope.
#  (3) Missing back-links - every divergence fixture's README must contain a link resolving to
#      a doc that CATALOGS that fixture (check 1's per-doc attribution), not merely to some
#      member of the family. With one conformance doc "cataloged in D" and "links D" were the
#      same fact; across the six-doc prettier family they are independent, so a README could
#      point at the shared frame while its entry lived in a language catalog. (A missing README
#      entirely is the validator's D1 rule.)
#  (4) Stray READMEs - a non-divergence fixture shouldn't carry a README; exceptions live in
#      the in-code ALLOWED_NONDIVERGENCE_READMES allowlist.
#  (5) Catalog-family drift - the docs/conformance_prettier*.md on disk must be exactly
#      CONFORMANCE_PRETTIER, and the frame's §Catalogs table must index every member.
#      Checks 1+3 read a hand-listed family while REPORTING the glob, so a catalog the list
#      omits fails only as their findings — its entries as orphans, a README aiming at it as
#      a missing back-link — with nothing naming the constant. (The reverse, a listed member
#      absent from disk, was already a hard error: it would make both checks vacuous.) The
#      index half covers the reader's route — CLAUDE.md sends divergence authors to the
#      §Catalogs table, so an unindexed member is unreachable that way with every other
#      check green. benches/js/lib/divergence/validation.ts reads the same glob off disk
#      rather than keeping its own copy, so only the constant needs the manual add.
# Pure Rust (no Deno). Exits non-zero on any finding. Gated in `deno task check`.
cargo run -p tsv_debug conformance_audit
# Also: --json (machine-readable: {orphans, family, dead_links, missing_backlinks, stray_readmes})
```

## Compiler Conformance Audit (`conformance:audit:compiler`)

```bash
# compile_conformance_audit - the compiler analog of conformance_audit, deliberately minimal:
# any _compiled_divergence-suffixed compile fixture must be cataloged in
# docs/conformance_svelte_compiler.md AND carry a README back-linking it. The catalog is expected
# to stay EMPTY (a safety valve for declining to reproduce a genuine oracle output bug — never a
# tolerance budget), so those two checks inspect nothing today — a tripwire armed for the first
# entry, not a standing gate. The third check needs no fixture and is the one that holds now:
# CHECKLIST ↔ `Refusal` DRIFT. docs/checklist_svelte_compiler.md quotes refusal bucket keys
# verbatim in its `**Refused**:` bullets and claims it maps onto corpus runs; nothing verified
# that. The audit extracts each quoted key and compares it against the keys the `Refusal` catalog
# can actually produce (`Refusal::all_bucket_keys`, one representative per variant; backticks are
# stripped on both sides, since a key may itself contain one). Only the DOC-QUOTES-A-NONEXISTENT-KEY
# direction GATES — that is the claim being false where a reader is misled. The reverse (a
# producible key with no bullet) is REPORT-ONLY: a variant covered by a prose paragraph rather than
# its own bullet is fine, so gating it would be born red and would push the doc toward a mechanical
# key dump. ⚠️ `Refusal::every_variant` (the oracle behind that check) is hand-maintained and NOT
# compiler-enforced — a new variant compiles fine while missing from it, silently narrowing the
# oracle; a pinned-count unit test is the only backstop. Pure Rust (no Deno). Exits non-zero on any
# finding. Gated in `deno task check`.
cargo run -p tsv_debug compile_conformance_audit
# Also: --json
```

## Canonicalizer Audit (`canonicalize:audit`)

```bash
# canonicalize_audit - canonicalize_js (the compile-parity reprint) at corpus scale: run the
# canonicalizer twice per TS/JS file (.ts/.js/.mts/.cts/.mjs/.cjs, .svelte.ts included) and bucket —
# input-rejected (informational: invalid fixtures, script-goal files), NON-IDEMPOTENT (failure),
# CORRUPT-OUTPUT / unreparseable reprint (failure; the canonicalizer self-validates by reparse),
# COMMENT-LOSS (failure; whitespace-normalized comment text/order before-vs-after — the bucket the
# other two are structurally blind to: a swallowed comment leaves valid, idempotent JS).
# Pure Rust (no Deno). Exits 1 on any failure. Gated in `deno task check` over tests/fixtures +
# tests/fixtures_compile (fast); point it at real corpora for the full sweep.
cargo run -p tsv_debug canonicalize_audit                              # default scope (tests/fixtures only)
cargo run -p tsv_debug canonicalize_audit tests/fixtures tests/fixtures_compile  # the check-gate scope
cargo run -p tsv_debug canonicalize_audit ~/dev/zzz/src ~/dev/gro/src  # real-corpus sweep
# Also: --json
```

## Compile-Fixture Validation (`compile:fixtures:validate`)

```bash
# per fixture in tests/fixtures_compile — three checks, all gating. The oracle leg
# needs the Deno sidecar. Also: --list, --json, positional filter patterns.
deno task compile:fixtures:validate
```

**What it proves.** Per fixture:

- **(a) oracle freshness** — `canonicalize_js(oracle(input.svelte))` equals the committed
  `expected_server.js` byte-exact, and the oracle CSS matches `expected.css` (both absent
  counts as a match);
- **(b) ours** — `tsv_svelte_compile::compile` succeeds and its canonicalized JS + CSS equal
  those same expectations (`parity`; a `mismatch` or `error` fails);
- **(c) expected idempotence** — the committed `expected_server.js` is a `canonicalize_js`
  fixed point.

Expectations are always oracle-generated (`compile_fixture_init`), never hand-written, so a
fixture records what Svelte does — declining to reproduce some behavior of it is a
`_compiled_divergence` plus a catalog entry
([conformance:audit:compiler](#compiler-conformance-audit-conformanceauditcompiler)), not an
edit to an expected file.

**Split gating, and the split is the point.** Checks (b) and (c) — plus "`input.svelte`
parses" — need no sidecar, so they also run as `tests/compile_fixtures_tests.rs` in every
`cargo test --workspace`: the offline parity gate inside `deno task check`. Check (a) cannot,
because it calls the canonical compiler. `deno task conformance` therefore preflights the full
command, and **that is the only place oracle freshness is graded anywhere.**

**Why that split is load-bearing.** The sidecar-free slice compares tsv's output against the
COMMITTED file — both inside the repo. When the oracle itself moves, neither side moves, so the
slice stays green while the expectations quietly stop describing what Svelte compiles today.
That is the same hole [`pins:audit`](#canonical-pin-agreement-audit-pinsaudit) has from the
other end (a self-consistent pin set says nothing about freshness), and the lockfile's `esrap`
pin exists because it was live: five committed fixtures staled with `deno task check` green
throughout.

**Blind spots.**

- **Scope is `tests/fixtures_compile`** — a curated tree, so a compile bug in a shape no
  fixture holds is invisible. The corpus-scale arms are `compile:corpus:compare` and the
  validation-suite ratchet ([compile_tooling.md](compile_tooling.md),
  [compile_validation_ratchet.md](compile_validation_ratchet.md)), neither gated in `check`.
- **Parity is canonical-reprint parity, not byte parity.** Both sides are compared after
  `canonicalize_js`, whose bar tolerates a comment-POSITION difference (`compare_canonical`) —
  so a difference the canonicalizer erases is one this cannot see, and the canonicalizer itself
  is guarded separately by [`canonicalize:audit`](#canonicalizer-audit-canonicalizeaudit).
