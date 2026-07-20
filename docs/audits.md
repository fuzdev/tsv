# Audit Gates

> The standing correctness audits over the formatter and parsers — what each proves, what it is blind to, how to run it, and where it gates. The `deno task` entry points are indexed in [CLAUDE.md §Fixtures](../CLAUDE.md#fixtures-rust--deno-based); this doc is the full reference.

Most audits are pure Rust (no Deno sidecar). Those gated in `deno task check` scan `tests/fixtures` — a curated, format-stable tree — so several are cheap tripwires there whose real yield is external corpora (`../prettier/tests/format`, `../svelte/packages/svelte/src`, sibling dev repos): point them at real code after a printer change, or run `deno task audit:corpus`, the standing bundle for exactly that. Audits that need the feature-gated instrumentation (`swallow_check` / `comment_check`) build via the `audits` umbrella feature under `--profile corpus` — the single build world every `deno task check` audit shares (optimized + `panic = "unwind"`, so a formatter panic is caught and reported instead of killing the process; plain `--release` is `panic = "abort"`).

## Overview

| audit | task | catches | gating |
| --- | --- | --- | --- |
| [Swallow](#line-comment-swallow-audit-swallowaudit) | `swallow:audit` | `//` line comment followed by content on one output line (silent content loss) | `deno task check` |
| [Comment ledger](#comment-ledger-audit-commentsaudit) | `comments:audit` | a parsed comment DROPPED or DOUBLE-PRINTED (print-once) | `deno task check` |
| [Gap injection](#gap-injection-audit-gapsaudit) | `gaps:audit` | comment drops in gaps no fixture covers | `deno task check` (ratchet) |
| [Blank injection](#blank-line-injection-audit-blanksaudit) | `blanks:audit` | blank-line handling: panic / idempotency / reparse / ledger / blank-run | `deno task check` (ratchet) |
| [Build fanout](#build-fanout-audit-fanoutaudit) | `fanout:audit` | exponential doc-node rebuild in nested layout candidates | `deno task check` |
| [Raw-find scan](#raw-find-scan-audit-scanaudit) | `scan:audit` | new raw substring scans over source (comment-blind delimiter matching) | `deno task check` |
| [Authoring independence](#authoring-independence-audit-authoringaudit) | `authoring:audit` | two render-equivalent authorings settling on two fixed points; non-idempotency | `deno task check` |
| [Round-trip](#formatreparse-round-trip-audit-roundtripaudit) | `roundtrip:audit` | formatted output the parser rejects (delimiter/structure corruption) | `deno task check` |
| [Binding](#commenttoken-binding-audit-bindingaudit) | `binding:audit` | a glued comment re-bound to a different subtree by a migrating paren | `deno task check` |
| [Fuzz](#seeded-mutational-fuzzer-fuzzaudit) | `fuzz:audit` | panic / non-idempotency / structural divergence on arbitrary input | `deno task check` |
| [Render equivalence](#render-equivalence-audit-renderaudit) | `render:audit` | `tsv format` changing what a Svelte component renders | `deno task conformance` (release) |
| [Layout neutrality](#layout-neutrality-audit-neutrality_audit) | — | a layout gate reading comment *ownership* instead of page occupancy | dev tool (pre-ownership-change) |
| [F1 sweep](#f1-idempotency-sweep-idempotencysweep) | `idempotency:sweep` | pass-2 reflow on real code | conformance cadence |
| [Corpus bundle](#the-corpus-bundle-auditcorpus) | `audit:corpus` | the content-loss / robustness bundle over real code | publish Step 3c |
| [Lexer diff](#differential-lexer-harness-lex_diff) | — | token-stream drift after a lexer change | dev tool |
| [Conformance audit](#conformance-audit-conformanceaudit) | `conformance:audit` | doc/fixture catalog + link integrity | `deno task check` |

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
# for a targeted run. Gated in `deno task check` (via `swallow:audit`) over tests/fixtures.
#
# Coverage is every render that appends to the output buffer — the main loop AND
# its sub-renders (fill segments, the line-suffix flush), all driving one
# per-thread state machine. A `line_suffix` comment is NOT exempt: two of them
# flushed at the same line break land back-to-back on one line (`x; // c2 // c1`)
# and the first `//` swallows the second. Comments written straight to the output
# buffer (the Svelte template buffer path) bypass the doc renderer and stay out
# of scope.
```

⚠️ **A green `swallow:audit` does not mean "no swallows"** — it formats each file **as
authored**, so a swallow only reachable once a comment sits in some other gap is a swallow it
never provokes. The [gap-injection audit](gap_audit.md) arms this same check on its injected
formats and reports what that reaches, as its report-only
[SWALLOW class](gap_audit.md#the-swallow-class).

## Comment Ledger Audit (`comments:audit`)

The print-once comment ledger: every comment a document PARSES must be EMITTED exactly once. tsv's answer to prettier's `ensureAllCommentsPrinted`, and the structural guard on the [detached comment model](./comments.md): nothing else forces a comment that the parser produced to actually reach the output.

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

**Model.** A format entry point (`tsv_ts::format_in`, `tsv_css`'s `format_css*`, `tsv_svelte`'s `format_svelte*`) REGISTERS the comment list it is about to print — that is the expectation. A doc-based printer (tsv_ts, tsv_svelte) TAGS each comment's doc node (`DocArena::tag_comment_doc`) and the RENDERER records the emit when it reaches the node; tsv_css, which writes comments straight to its buffer, records at the write. The render-time seam is load-bearing: a builder may assemble the same subtree into two `conditional_group` candidates of which one renders, so counting at build time reads as a double-print (and a comment built only into a LOSING candidate would read as printed while being lost). A `format-ignore` region — and any other raw source slice that carries comments out verbatim (a raw at-rule prelude, a glued CSS compound selector) — records a VERBATIM RANGE that counts as one emit per comment it covers; keep those ranges tight, a too-wide carve-out silently re-opens the hole.

**Scope.** Both comment carriers are registered and guarded: the DETACHED comments (the flat `Vec<Comment>` on the language root) and the AST-NODE comments — a Svelte `<!-- … -->` (`FragmentNode::Comment`) and a CSS in-block `CssBlockChild::Comment`. The latter are carried by the tree rather than by the positional model, but a printer can still drop or double-print one, so each format entry walks its tree and registers their spans; with that, `unregistered emits` is a pure registration-gap signal (0 over clean fixtures) — a nonzero count means the walk missed a container. CSS declaration-VALUE comments remain outside the model by construction — never lexed as `Comment`s at all (re-derived from source), so there is nothing to register.

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
# snapshot of the ~717 shapes tests/fixtures produces, every line a KNOWN BUG, the file
# shrinking is the goal. A shape not on the list, one on it that no longer fires, or any
# PANIC, FAILS. `--limit`/`--payload`/`--all-bytes`/a path narrow a run, so they skip the
# ratchet and refuse `--update`. ~17 s.
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
#   - At a tag's CONTENT BOUNDARY — hug↔space↔newline, i.e. the run IS created and
#     destroyed. Svelte 5 removes start/end-of-content whitespace at compile, so all
#     three authorings render identically. This is the family that catches a formatter
#     letting a render-free character pick the layout (the delimiter-dangle class).
#     Fits-inline content is probed too — tsv trims a render-free boundary run even when
#     the content fits (`<span> text </span>` → `<span>text</span>`, the Svelte-mirror
#     trim; fixture `inline_boundary_whitespace_prettier_divergence`, conformance_prettier.md
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
cargo run -p tsv_debug roundtrip_audit --gate --canonical-all ../prettier/tests/format  # thorough
# Also: --no-render, --verbose (AST diff per finding), --limit N, --json. The full
# (non-gate) run is a diagnostic — the divergent bucket over tests/fixtures is
# Svelte-reflow-noisy vs render_normalize's simpler whitespace model.
cargo run -p tsv_debug roundtrip_audit --canonical-all --verbose ../prettier/tests/format/typescript
```

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
# paren deep inside is not a finding; a paren at the anchor is.
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
# Also: --gate (exit 1 on findings), --json, --limit N. Needs the Deno sidecar, so
# NOT in `deno task check` — and not in the pure-Rust `audit:corpus` either. It is
# release-gated as a leg of `deno task conformance` (the one leg that runs as a
# subprocess), scoped there to the version-pinned `framework` + `suite` checkouts so
# a live working tree can't move a release verdict; run it standalone on any corpus
# after a printer change. Files whose format is a no-op are skipped (trivially
# render-equal); files Svelte's semantic ANALYZER rejects are counted as
# compile-blind (that arm cannot speak there). The in-repo, any-corpus form of
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

The standing content-loss / robustness gate over REAL code — the extension-robustness bar that `deno task check`'s fixture-only scope is structurally blind to: `roundtrip_audit --gate` + `comment_audit` + `binding_audit --gate` (real gating; prettier suites report-only) + `authoring_audit` + `fuzz --iterations 0`, over the `perf` corpus view + the pinned prettier suites. Pure Rust; absent dev repos warn-skip (floor = `../svelte` src). NOT in `deno task check` (machine-dependent corpus, minutes); wired into publish Step 3c alongside conformance:all's SAFETY. Run at conformance/release cadence or after a printer change. See ../benches/js/CLAUDE.md §Gate map.

```bash
deno task audit:corpus
```

## Differential Lexer Harness (`lex_diff`)

```bash
# lex_diff - differential lexer harness: snapshot the raw token stream over a
# corpus and diff against a golden to prove token-stream identity (kind, start, end,
# decoded per token) after a lexer change — stronger than format byte-identity.
# Covers the context-free next_token dispatch for .ts/.mts/.cts/.svelte.ts/.css.
# Pure Rust, no Deno.
cargo run -p tsv_debug lex_diff ~/dev/zzz/src --golden /tmp/lex.golden --write  # capture golden
cargo run -p tsv_debug lex_diff ~/dev/zzz/src --golden /tmp/lex.golden          # check against it
# Options: --write (capture instead of check), --verbose (first divergent line per file)
```

## Conformance Audit (`conformance:audit`)

```bash
# conformance_audit - doc/fixture integrity in one fixture walk. Four checks:
#  (1) Orphans - every divergence-suffixed fixture must be linked in its conformance doc
#      (_prettier_divergence → docs/conformance_prettier.md, _svelte_divergence →
#      docs/conformance_svelte.md, _svelte_prettier_divergence in both).
#  (2) Dead links - every Markdown link (relative path + #anchor) in every docs/*.md
#      (enumerated at run time, so a new doc is gated by existing) and every fixture README
#      must resolve on disk (catches renamed/deleted fixtures, wrong ../ depth, stale
#      anchors). External URLs and targets that climb out of the repo (sibling checkouts,
#      machine-dependent) are out of scope.
#  (3) Missing back-links - every divergence fixture's README must contain a link resolving to
#      its sanctioning doc. (A missing README entirely is the validator's D1 rule.)
#  (4) Stray READMEs - a non-divergence fixture shouldn't carry a README; exceptions live in
#      the in-code ALLOWED_NONDIVERGENCE_READMES allowlist.
# Pure Rust (no Deno). Exits non-zero on any finding. Gated in `deno task check`.
cargo run -p tsv_debug conformance_audit
# Also: --json (machine-readable: {orphans, dead_links, missing_backlinks, stray_readmes})
```
