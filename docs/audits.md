# Audit Gates

> The standing correctness audits over the formatter, the parsers and their wire contract, the Svelte compiler, and the canonical oracles all of them are graded against — what each proves, what it is blind to, how to run it, and where it gates. The `deno task` entry points are indexed in [CLAUDE.md §Fixtures](../CLAUDE.md#fixtures-rust--deno-based); this doc is the full reference.

Most audits are pure Rust (no Deno sidecar). Those gated in `deno task check` scan `tests/fixtures` — a curated, format-stable tree — so several are cheap tripwires there whose real yield is external corpora (`../prettier/tests/format`, `../svelte/packages/svelte/src`, sibling dev repos): point them at real code after a printer change, or run `deno task audit:corpus`, the standing bundle for exactly that. Audits that need the feature-gated instrumentation (`swallow_check` / `comment_check`) build via the `audits` umbrella feature under `--profile corpus` — the single build world every `deno task check` audit shares (optimized + `panic = "unwind"`, so a formatter panic is caught and reported instead of killing the process; plain `--release` is `panic = "abort"`). ⚠️ A **stack overflow** is the one failure this containment cannot reach — it is not a panic, so no `catch_unwind` sees it and the sweep dies outright rather than reporting one file. That is why `tsv_debug`'s dispatch runs on the same stated reservation the `tsv` binary does (`tsv_cli::cli::stack`), which puts the depth an audit survives out of reach of ordinary input and, more to the point, makes it the same on every machine; see [cli.md §Recursion Depth](./cli.md#recursion-depth).

The Svelte compiler's *sidecar-dependent* harnesses — the corpus comparison, the validation-suite ratchet, the differential compile fuzzer — are not audits in this sense and are not gated here; they live in [compile_tooling.md](compile_tooling.md) and [compile_validation_ratchet.md](compile_validation_ratchet.md). The compile fixtures are the one split case: their parity legs are pure Rust and gate in `check`, their oracle-freshness leg needs the sidecar and gates in `conformance`, so both are documented [below](#compile-fixture-validation-compilefixturesvalidate).

**One seed resolution, one directory walk, and a vacuity floor that is not scope-dependent.** Every corpus-walking audit resolves its corpus through `resolve_seed_files` / `resolve_seed_files_named` (`tsv_debug`'s `cli/commands/profile.rs`): positional paths defaulting to `tests/fixtures`, one walk that prunes what `tsv format`'s own discovery prunes and keeps its extension set (`tsv_discover`'s safety nets, build-output heuristic, and `FORMATTABLE_EXTENSIONS` — so an audit's scope is the set the production formatter would process, not a hand-mirrored list beside it), then the audit's own subject filter, so an empty scan says "no `.svelte` files found" rather than a flattened message. **An empty scan is an error at every scope, and so is a run that resolves files but grades none of them** (`check_graded_nonzero` — the case where the corpus resolves fine and the parser rejects all of it). Each audit floors the count its own *verdict* rests on, not the count resolution returned: files formatted, files compared, boundary sites probed, render keys checked. The rule for picking it — count every outcome that carries a verdict, exclude only the ones that could not be evaluated. A trivially-clean outcome is a verdict (a no-op format renders identically by identity) and counts; "the parser rejected it" is not, and does not. The pinned minimums (`FIXTURES_FORMATTED_MIN`, `comments:audit`'s `REGISTERED_MIN`) are the stronger *default-corpus* guard layered above that floor: they catch a corpus that shrank rather than one that vanished, and only a default run can be held to a number, so they stay `default_paths`-gated. The two pins ask different questions — the file count catches a corpus that shrank or a skip policy that diverged, the comment count catches *registration* collapsing while every file still formats — so `comments:audit` passes both. Ignore files are deliberately **not** consulted by the walk — the root `.formatignore` prunes the fixture trees (they are data, not format fixed points), so a walk honoring them would resolve the audits' own default corpus to nothing.

## Overview

| audit | task | catches | gating |
| --- | --- | --- | --- |
| [Swallow](#line-comment-swallow-audit-swallowaudit) | `swallow:audit` | `//` line comment followed by content on one output line (silent content loss) | `deno task check`; `audit:corpus` (real code) |
| [Comment ledger](#comment-ledger-audit-commentsaudit) | `comments:audit` | a parsed comment DROPPED or DOUBLE-PRINTED (print-once) | `deno task check`; `audit:corpus` (real code) |
| [Gap injection](#gap-injection-audit-gapsaudit) | `gaps:audit` | comment drops — and `//` swallows — in gaps no fixture covers | `deno task check` (ratchet) |
| [Wire injection](#wire-injection-audit-wireaudit) | `wire:audit` | a WIRE divergence from the canonical parser that only a spelling no corpus contains reveals — the parse-side sibling of gap injection | on demand (⚠️ red by design) |
| [Blank injection](#blank-line-injection-audit-blanksaudit) | `blanks:audit` | blank-line handling: panic / idempotency / reparse / ledger / blank-run — plus the blank-DROP absorb pin (a new kind of silently-eaten blank) | `deno task check` (ratchet) |
| [Blank fabrication](#blank-fabrication-audit-fabricationaudit) | `fabrication:audit` | a blank line the formatter INVENTS on a pristine seed (the author never wrote it) | `deno task check` (ratchet); `audit:corpus` (real code) |
| [Comment census](#comment-census-audit-censusaudit) | `census:audit` | a comment interior lost, gained, or rewritten between raw input and raw output — parse-time drops included, which the ledger can't see | `deno task check` (ratchet); `audit:corpus` (real code) |
| [Print width](#print-width-audit-widthaudit) | `width:audit` | a new KIND of over-width output line — the shape a hard-limit bug takes | `deno task check` (ratchet) |
| [Ignore honoring](#ignore-directive-honoring-audit-ignoreaudit) | `ignore:audit` | `prettier-ignore` positions that silently reformat an ignored node, misbind a trailing directive, over-freeze, or lose the freeze on pass 2 | `deno task check` (ratchet) |
| [Build fanout](#build-fanout-audit-fanoutaudit) | `fanout:audit` | exponential doc-node rebuild in nested layout candidates | `deno task check` |
| [Raw-find scan](#raw-find-scan-audit-scanaudit) | `scan:audit` | new raw substring scans over source (comment-blind delimiter matching) | `deno task check` |
| [Self-format](#self-format-audit-formataudit) | `format:audit` | tsv failing to format its OWN TS/JS — a would-change file (non-idempotency) or a parse error (over-rejection) | `deno task check` |
| [Doc link](#doc-link-audit-docsaudit) | `docs:audit` | a doc-comment `[link]` that no longer resolves — a stale doc | `deno task check` |
| [Wire-type drift](#wire-type-drift-check-checkast-types) | `check:ast-types` | the shipped `tsv_ast.d.ts` no longer describing what the wire-JSON writers emit — plus a wire type it never declared at all | `deno task check` |
| [Pin agreement](#canonical-pin-agreement-audit-pinsaudit) | `pins:audit` | the five canonical-oracle pin sites disagreeing — including the lockfile, which alone pins the oracle's own transitive deps | `deno task check` |
| [Checkout alignment](#checkout-alignment-audit-pinsauditcheckouts) | `pins:audit:checkouts` | a present `../svelte` / `../acorn-typescript` clone that is not the pinned version; commit drift (warn) | `deno task conformance` (preflight) |
| [Authoring independence](#authoring-independence-audit-authoringaudit) | `authoring:audit` | two render-equivalent authorings settling on two fixed points; non-idempotency | `deno task check`; `audit:corpus` (real code) |
| [Razor sweep](#print-width-razor-sweep-razoraudit) | `razor:audit` | width-keyed layout bugs — an F1 break at some column, and the stray line-head boundary space that is its OWN fixed point | `deno task check` |
| [Round-trip](#formatreparse-round-trip-audit-roundtripaudit) | `roundtrip:audit` · `roundtrip:audit:prettier` | formatted output the parser rejects (delimiter/structure corruption) | `deno task check` (fixtures always; the prettier suites when `../prettier` is present); `audit:corpus` (real code) |
| [Binding](#commenttoken-binding-audit-bindingaudit) | `binding:audit` | a glued comment re-bound to a different subtree by a migrating paren | `deno task check`; `audit:corpus` (real code) |
| [Render equivalence](#render-equivalence-audit-renderaudit) | `render:audit` | `tsv format` changing what a Svelte component renders | `deno task conformance` (release) |
| [Layout neutrality](#layout-neutrality-audit-neutrality_audit) | — | a layout gate reading comment *ownership* instead of page occupancy | dev tool (pre-ownership-change) |
| [Fuzz](#seeded-mutational-fuzzer-fuzzaudit) | `fuzz:audit` | panic / non-idempotency / structural divergence on arbitrary input | `deno task check` |
| [F1 sweep](#f1-idempotency-sweep-idempotencysweep) | `idempotency:sweep` | pass-2 reflow on real code | conformance cadence |
| [Corpus bundle](#the-corpus-bundle-auditcorpus) | `audit:corpus` | the content-loss / robustness bundle over real code | publish Step 3c |
| [Lexer diff](#differential-lexer-harness-lex_diff) | — | token-stream drift after a lexer change | dev tool |
| [Variant direction](#variant-whitespace-direction-audit-variantsaudit) | `variants:audit` | a `_compact` variant that ADDS whitespace, or a `_spaces` variant that REMOVES it — the pair's two directions collapsing into one | `deno task check` |
| [Conformance audit](#conformance-audit-conformanceaudit) | `conformance:audit` | doc/fixture catalog + link integrity | `deno task check` |
| [Compiler conformance](#compiler-conformance-audit-conformanceauditcompiler) | `conformance:audit:compiler` | compile-fixture divergence catalog + checklist ↔ `Refusal` drift | `deno task check` |
| [Canonicalizer](#canonicalizer-audit-canonicalizeaudit) | `canonicalize:audit` | `canonicalize_js` non-idempotence / corrupt output / comment loss | `deno task check` |
| [Compile fixtures](#compile-fixture-validation-compilefixturesvalidate) | `compile:fixtures:validate` | a stale compile expectation (oracle freshness) · tsv-vs-expected compile parity · expected-file idempotence | parity legs in `deno task check` (`cargo test`); freshness in `deno task conformance` |
| [Fixture validation](./fixture_overview.md) | `fixtures:validate` | a fixture claim no longer holding — parser/formatter parity vs the committed files · the ORACLE itself having moved (freshness, sidecar) | parity in `deno task check` (`cargo test --test fixtures_tests`); freshness in `deno task conformance` (with `bench:pins:suites`, its pin-freshness preflight) |

⚠️ **Editing whitespace in a fixture is never local to that fixture.** The three
injection ratchets — [gaps](#gap-injection-audit-gapsaudit),
[blanks](#blank-line-injection-audit-blanksaudit),
[ignore](#ignore-directive-honoring-audit-ignoreaudit) — enumerate their sites **from
the seed text**, and the seeds are every fixture file, variants included (the
[fabrication](#blank-fabrication-audit-fabricationaudit) sweep injects nothing — it
grades the pristine seeds — but its pinned shapes are seed-dependent the same way).
Whitespace
you delete is a site they can no longer probe, so a pinned bug whose only reproducer
lived in that spelling stops firing and its ratchet fails **STALE**. The fix is to
restore the shape, never to re-pin: a `known.txt` line is a real bug on a work list,
and dropping it retires a bug nobody fixed. Find what was lost by running the audit
with `--report` (gaps/blanks/ignore; fabrication reports via `--json`) against an
unmodified checkout — that also answers "did I cause this
or inherit it?" — and re-home the shape in a variant whose name describes it. A shape
that is *bidirectional* (a space before a bracket, the identifier glued after) fits
neither bare `_compact` nor `_spaces` and needs its own qualified variant. After any
whitespace edit across many fixtures, run all four.

## Line-Comment Swallow Audit (`swallow:audit`)

```bash
# swallow_audit - format files with the render-time swallow check on and report
# any `//` line comment followed by content on the same output line (silent
# content loss). Pure Rust, no Deno. Defaults to tests/fixtures; pass dirs/files
# to audit real code. Exits 1 on any finding.
cargo run --profile corpus -p tsv_debug --features audits swallow_audit                # audit all fixtures
cargo run --profile corpus -p tsv_debug --features audits swallow_audit ../zzz/src  # audit a real codebase
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
cargo run --profile corpus -p tsv_debug --features audits comment_audit ../zzz/src  # audit a real codebase
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
cargo run --profile corpus -p tsv_debug --features audits gap_audit ../zzz/src
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

## Wire-Injection Audit (`wire:audit`)

```bash
deno task wire:audit                # whitespace in heads — tests/fixtures, Svelte only
deno task wire:audit:terminators    # lone CR / U+2028 / U+2029 anywhere — same corpus
deno task corpus:compare:parse <path> --filter svelte --inject [--inject-terminators] [--inject-limit N]
```

**What it proves.** That tsv's parse **wire** still matches the canonical parser after
whitespace is injected into a Svelte tag or block head — `{#…}`, `{:…}`, `{@…}`. Each
manufactured input is graded against the real external oracle (`svelte.parse`), through
the same deep-diff and documented-divergence classifier `corpus:compare:parse` uses.

**Why it exists — the shape of the hole it fills.** Every other injection or fuzz audit
in this repo grades a *formatter-side, self-referential* property: gap and blank
injection check formatting, the fuzzer checks no-panic + idempotency + structural
reparse, round-trip checks that output reparses. The wire-vs-canonical comparison is the
opposite — a genuine external oracle — but it only ever ran over inputs someone had
already **written**: committed fixtures and real repos. Nothing manufactured an input
and graded the resulting *wire*.

A hand-rolled scan can therefore be confidently wrong about a spelling no document
contains. Both block-annotation bugs lived exactly there: the line seed a `: T`'s own
acorn parse needs, and the offset that annotation is anchored at, were each wrong for
any spelling that put whitespace between a binding and its colon — invisible to 9441
fixtures and every real repo, because everyone writes `x: T`. The comparison catches
both the instant such an input exists; this audit makes them exist.

**What it perturbs — two families, because there are two kinds of claim to break.**

- **`ws`** — whitespace inside a head, at three kinds of position: the head's own `#` /
  `:` / `@` **marker** (does the reader assume the marker is glued to the `{`?), the start
  of each existing whitespace run (does this construct measure from the token or from the
  gap?), and each `:` / `,` / `=` with nothing before it (does it assume the two are
  adjacent?). Heads are where tsv hand-rolls its scanning — head splitting,
  binding/annotation separation, delimiter finding — rather than delegating to acorn.

  ⚠️ **The marker position is the sharpest of the three, and was reachable only for `:`
  until it was made explicit** (`:` is in the delimiter set; `#` and `@` are not, so the
  axis was blind for exactly the two markers a placement rule reads). It is the one
  position where the two sides of the language disagree by design — Svelte's `tag()` and
  `read_attribute` run `allow_whitespace()` after the `{`, its `read_sequence` does not —
  so a reader mirroring either reads a byte at a **fixed offset** from the brace, i.e.
  assumes a gap of width **zero**. That is the same emit-set ⊆ parse-set hazard a baked-in
  keyword width is, one notch narrower: the formatter's brace normalization *closes* that
  gap, so the assumption is reachable from tsv's own output. Three bugs came out of it at
  once — `<div { @attach fn}>` rejected though prettier formats it, a `{ #x in y}` the
  printer glued into a form tsv then refused to reparse, and a static `<script { #a}>` head
  whose attribute-name run folded the author's whitespace into the name (that third one
  reaches the gap through a lexer TOKEN's end rather than through byte arithmetic, so it is
  the spelling a `+ 2` sweep does not find).
- **`terminators`** — a lone `\r`, `<LS>` or `<PS>` at every whitespace-run start in the
  document, head or not. These are exactly the spellings on which the two `loc` line
  classes DISAGREE, and which class a node was counted under is decided *per acorn parse*
  by what Svelte did to the prefix it handed acorn (three preparations across five
  readers — [architecture.md §`loc` lines](./architecture.md#loc-lines-two-classes-one-per-acorn-parse)).
  That model is mirror-knowledge held by hand at seven call sites in `tsv_svelte`'s
  parser, and **nothing else grades it**: no fixture can carry a raw `<CR>` (every
  parse-then-format entry point folds it, so such a document is not the fixed point F1
  requires), and no real repo contains one. `\n` and `\r\n` are deliberately not injected
  — both classes count them identically, so a variant carrying one tests nothing.
  Document-wide rather than head-scoped because the axis is: a terminator matters in the
  prefix acorn measured, in the run acorn *skipped*, and inside the island itself, and
  only the first of those is ever in a head.

  ⚠️ **It grades a second axis nothing else does, and its best yield so far came from that
  one**: U+2028 / U+2029 are also members of **JS `\s`**, so this family is the standing
  probe for whether a Svelte reader asks the right WHITESPACE CLASS — the `ws` family cannot
  be (its inserts are ASCII). **Four of the five** `tsv_svelte` readers that were found asking
  a Rust whitespace class were found *by this family* — one of them turning valid Svelte into
  output `svelte compile` rejects — for 19 of the 27 signature groups it has ever retired. (The
  fifth, the printer's `lang` read, is out of its reach: only a U+FEFF exhibits it, and this
  family injects a CR and the two line separators.) Point it at any new reader, and pair a
  finding with U+0085 NEL — Rust-whitespace but not JS `\s` — as the null control.

**Coverage differs per family, and that is the whole design.** `ws` is head-scoped —
~11,049 sites over `tests/fixtures` — so it runs as a **CENSUS** (`--inject-limit 0`,
every site, ~17 s for 22,098 variants). `terminators` is document-wide — ~628,852 sites
— so a census would cost ~9 minutes and it stays a strided **SAMPLE** (the default
`--inject-limit 12`, which reaches 5.9% of its sites in 111,669 variants, ~33 s). Where a
run is sampled, sites are taken by even stride rather than as a prefix, so the per-file
cap spreads across the document instead of piling into its first few lines.

⚠️ **A sampled family cannot be RATCHETED, and the reason is the stride, not the key.**
The obvious pin — the diff signature, which is already computed and already the grouping
key — is stable in itself; what is not stable is *which sites a run probes*. The stride
divisor is the file's own site count, so an edit anywhere in a file redraws its whole
sample, including in text the edit never touched. Measured over `tests/fixtures`: adding
one member to one type-literal fixture — a routine coverage extension, unrelated to
anything the family grades — moved that file's probed offsets from `[7, 37, 77, 106]` to
`[7, 44, 82, 118]` and retired **12 of the terminator family's then-194 finding signatures**
as stale, with every underlying divergence intact. (194 is the reading that measurement was
taken against, kept because it is the evidence; the family's standing count is the 175
above.) A ratchet would have hard-failed and been
"fixed" by re-pinning, which is the rot [gap_audit.md](./gap_audit.md) designs against
("a gate that fails per added fixture would just get turned off"). The exposure is
structural, not incidental: **96 of those 194 signatures are produced by a single base
fixture each, and 17 files carry all 96** (one alone carries 18).

A census has no divisor and no such motion — the same edit moved the `ws` census by
**zero** signatures — and it is monotone under a fixture addition, since a new file can
only add sites. So the rule is: **census ⇒ gradeable; sample ⇒ discovery only.** Making
`ws` a census was not merely a stability win, either — the sampled form found 3
undocumented files where the census found **25** (7 signature groups vs 19), which is 8x
its own findings hidden. That reading is kept because it is the evidence; it has since
been worked down to zero, the comment-extent bug having accounted for 15 of the 25 and all
but 4 of the groups, and the dedent bug below for the rest.

**Each variant is graded against its own base file.** A divergence the base already had
is not the injection's doing, and `tests/fixtures` deliberately contains ~91
`_svelte_divergence` fixtures whose whole purpose is to differ from canonical. The base
files are controls and are dropped; only the delta is reported. Subtraction is by diff
*signature*, since an injection shifts every offset after it.

**Blind spots.**

- **Svelte inputs only.** Standalone TS/CSS files are never perturbed (their `loc` has one
  line class and no per-parse seed), and the `ws` family additionally reaches only heads.
- **A base that already diverges at a signature masks a new divergence at that same
  signature** in its variants. A clean base is the better seed.
- **A rejection is a finding on either side, and neither carries diffs to be found by.** A
  variant **tsv** rejects where its base parsed is an injection-introduced over-rejection;
  one the **ORACLE** rejects where its base parsed is an injection-introduced
  over-acceptance. Both are kept with their `#inj:` label — the only thing that reproduces
  them — both are baseline-subtracted on their own side (`TSV_REFUSED` / `CANONICAL_REFUSED`
  in `wire_inject.ts`), and both are listed per file in `--json`'s `errors`. Reported, never
  gated, in both directions: a deferred early error is a documented tsv posture. On
  `tests/fixtures` today: 196 over-rejections under `terminators` (against 577 before the
  inherited ones — every `input_invalid_*` fixture's — were subtracted out), and under `ws`
  0 over-rejections against **33** over-acceptances (against 64 raw, the rest inherited).
  ⚠️ The subtraction is what makes the number readable: raw, the oracle-side count
  re-reports each `_svelte_divergence` fixture's own sanctioned over-acceptance once per
  variant derived from it, so it sizes the fixture tree rather than the injection.
  Only `both_error` — an injection that made the document invalid outright — stays a raw
  skip, being a claim about neither side.
- **`both_error` hides the over-acceptance's twin.** An injection that turns a document
  tsv accepted into one it rejects *while the oracle also rejects it* is dropped, since a
  variant no side accepts says nothing about either. A real over-rejection whose injected
  form happens to be invalid Svelte therefore never surfaces.
- **The head scan is approximate** — it counts braces without tracking strings or
  comments, so a head containing `'}'` ends early. That costs sites; it cannot
  manufacture a wrong finding.

⚠️ **Currently RED by design**, like `compile:fuzz` — a discovery tool with an open work
list, not a regression gate, which is why it is not in `deno task check` (it also needs
the canonical parser, so it is conformance-tier at best). Standing findings:

- **`ws`** (census: **0 files**) — the family is CLEAN, and both of the bugs it found asked
  a sub-parse the same question: **which SOURCE did that parse actually see?**

  The one it retired second was **the acorn comment DEDENT computed against the document**
  (10 files, from 2 `tags/const/` bases) where canonical computes it against the synthetic
  source it built. Svelte's `onComment` strips the comment line's own indentation from every
  line of a multiline block comment's `value` (`1-parse/acorn.js`), and `Comment::wire_value`
  mirrors it — but four of Svelte's parses hand acorn a **manufactured** string whose line
  prefix is not the author's. `tsv_lang::AcornPrefix` is the model of them, resolved per
  COMMENT from `Root::acorn_regions` (a block binding's island is up to two parses, each
  blanking a different span). Same shape as the per-parse `loc` line class (`AcornSeed`), one
  field over: what acorn saw, not what the document says — and it is subtler than a width in
  four ways, each of which was a live bug in the first cut: the two blankings differ
  (`/[^\n]/g` erases the author's tab, `{#snippet}`'s `/\S/g` keeps it and blanks past it);
  `read_pattern` deletes one blank from its prefix; a run that reaches `read_script`'s body
  carries on into the body's own whitespace; and `read_type_annotation`'s `_ as ` is spliced
  OVER five document bytes, so a `\n` among them is one acorn never sees. The blanks are
  built in **JavaScript's** units at that — `\S` complements JS `\s`, not Rust's
  `White_Space`, and `String.replace` walks UTF-16 code units, so an astral character blanks
  to two columns. Pinned by
  [comment_dedent_manufactured_source.rs](../tests/comment_dedent_manufactured_source.rs)
  (each reader with its null controls, and the spellings no formatter leaves standing), by
  the frozen
  [head_multiline_comment_dedent](../tests/fixtures/svelte/syntax/comments/head_multiline_comment_dedent/)
  fixture (the template readers, kept alive behind `<!-- prettier-ignore -->`), and by
  [const_annotation_comment_svelte_divergence](../tests/fixtures/svelte/tags/const/const_annotation_comment_svelte_divergence/)
  (the one spelling that is a fixed point unfrozen).

  ⚠️ **The census's zero certifies only the sources some fixture carries in a head.** It
  injects whitespace, so it can only reveal the bug where a fixture already holds a
  multi-line block comment in the affected head — the trigger is a comment that OPENS on the
  synthetic region's own line, which formatting normally moves off it. The three TEMPLATE
  readers (`read_pattern`, `read_type_annotation`, `{#snippet}`'s `\S` prelude) are kept
  reachable by the `<!-- prettier-ignore -->`-frozen
  [head_multiline_comment_dedent](../tests/fixtures/svelte/syntax/comments/head_multiline_comment_dedent/)
  fixture, which carries each with its null controls; `read_script` cannot be a fixture at
  all (prettier reformats a script's body through an ignore directive, and both formatters
  move its content off the tag's line), so there the Rust test is the sole guard.

  The sibling it retired was a **comment extent clipped at a trimmed slice boundary**: the
  bounded head readers handed their interior to the sub-parse whitespace-trimmed at BOTH
  ends, so a `//` that ended the interior ended where the trim did — a line comment runs to
  the terminator, so the space in `{@html expr // c ⏎}` is the comment's own text. **Eight**
  readers spelled it that way against the neighbours' leading-only trim, and the census could
  name only five: it injects WHITESPACE, so it reveals the bug just where a fixture already
  carried a trailing `//` in that head. Defining the class by the trim rather than by the
  bases it surfaced is what reached the `{#each}` key and the `{#key}` head; enumerating the
  sub-parse ENTRY POINT's callers is what reached the eighth, `reparse_each_iterable`, whose
  slice ends at a `{#each}` head's second `as` and which no head-shaped sweep passes over.
  That last enumeration also bounds the class: the three remaining both-ends trims that feed
  a sub-parse (the `{#each}` binding's, the `{:then}`/`{:catch}` region's, the `{@const}`
  binding's) are unreachable, each guarded by an explicit rejection of the trailing comment
  that canonical rejects too. `Parser::parse_ts_expression` now states the rule for its
  callers, and the extents of every head — the trimmed ones and the agreeing neighbours, in
  one wire — are pinned by
  [head_final_line_comment_extent](../tests/fixtures/svelte/syntax/comments/head_final_line_comment_extent/),
  whose `prettier-ignore` is what makes the trigger format-stable enough to be a fixture at
  all (both formatters trim a line comment's trailing whitespace).

  Because the family is a census it is now at zero, which makes `ws` a candidate to become a
  **green gate at zero**, not a ratchet: there is nothing left to pin. Its oracle-side
  rejections are separately sized above and are currently **all documented** — 32 split
  `!==` into `! ==`, whose non-null assertion only a TS parse
  accepts (the tracked [TypeScript-mode
  gating](./conformance_svelte.md#typescript-mode-gating-tracked-over-acceptance)
  over-acceptance), and 1 splits `?:` into `? :`, feeding Svelte's own `/\?\s*:/g` template
  rewrite (pinned in
  [`block_pattern_annotation_span.rs`](../tests/block_pattern_annotation_span.rs)).
- **`terminators`** (sampled: 277 files / 175 signature groups), by file count over
  `tests/fixtures` — two groups, neither of them a line-*class* question despite the family
  that surfaced them:
  - a `{@debug}` identifier's `loc` line and column under a **lone `<CR>`** (54 / 51 files);
  - the An+B residue the CSS finding left behind — `REGEX_NTH_OF` is a JS regex and tsv's
    An+B scanner is ASCII, so a `<LS>`/`<PS>` in an `:nth-*()` argument diverges; enumerated
    and pinned in [css_boundary_whitespace.rs](../tests/css_boundary_whitespace.rs)
    (4 files, down from 52 / 38 / 20).

  The unquoted-attribute-value terminator is a **whitespace-class** question, not a terminator
  one: it must spell Svelte's JS `\s` (all nineteen non-ASCII members plus VT), not a raw BYTE
  match or a Rust whitespace class — a narrower class absorbs those characters into the value
  and turns an expression attribute into a quoted string `svelte compile` rejects. The crate's
  discipline (one class, Svelte's) is stated at the top of
  [whitespace.rs](../crates/tsv_svelte/src/whitespace.rs).

  The CSS side answers the same class the same way: the CSS parser steps the whole
  `allow_whitespace()` run (`CssParser::skip_boundary_whitespace`, and its
  comment-looping twin `skip_boundary_whitespace_registering_comments`) at every juncture
  `parseCss` has one — the stylesheet body, a style rule's block and an at-rule's, the
  selector-internal ones, the compound break, a pseudo-argument list's start and its `)`, and
  the attribute selector's interior, and a declaration's property→colon gap — and the printer
  puts the non-ASCII members back at every selector juncture, at every rebuilt block-child
  head, in the property gap, and at a block's tail (`preserved_boundary_ws` /
  `boundary_ws_in_gap`, which partition each gap between them; an attribute selector's tail
  and the property gap keep the author's bytes outright). One printer position still drops
  the run — the stylesheet's trailing whitespace — ratcheted in
  [css_boundary_whitespace.rs](../tests/css_boundary_whitespace.rs). See
  [conformance_svelte.md §Boundary whitespace](./conformance_svelte.md).

## Blank-Line Injection Audit (`blanks:audit`)

Full reference — flags, the ratchet, the absorb pin, reading a finding, the six invariants, scope: **[blank_audit.md](./blank_audit.md)**. Design rationale (the fast path, why a blank is graded against the injected input not the pristine, the string-interior exclusion) lives in the `blank_audit` module docs.

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
cargo run --profile corpus -p tsv_debug --features audits blank_audit ../zzz/src
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
# gap_audit's one-format-per-site cost; only a KEPT blank pays the full battery (~22%
# of injections over tests/fixtures). ~30 s.
#
# A SECOND snapshot, `blank_absorb_known.txt`, pins the blank-DROP class the six
# invariants deliberately don't grade (a silently deleted blank is its own fixed point,
# so F1 / fuzz / roundtrip / ledger / census are all blind — only a prettier compare on
# the authored shape sees one): every NODE-EDGE class `(node_type, left→right)` where an
# injected blank is absorbed WITHOUT an authored blank already in the gap (that case is
# the sanctioned 2+→1 run collapse, exempt). ⚠️ A BEHAVIOR PIN, not a bug list
# (width_audit's stance — most absorption is sanctioned; prettier collapses those blanks
# too): a NEW class is a QUESTION to triage against prettier, never a silent pass. New
# and stale classes FAIL; graded only on the full default corpus. `--report` lists every
# class with its reproducer.
# Scope: TS + Svelte body; CSS deferred; string/template interiors excluded (a raw
# newline there is lexed as content, not a gap); only format fixed points injected.
```

`deno task blanks:audit:update` regenerates both snapshots after fixing a shape (or sanctioning a new absorb class); it refuses a narrowed run. ⚠️ A fix routinely shrinks the **bug** snapshot while leaving its **absorb** line in place — the absorb key is a node edge and its reproducer only one of the textual shapes that key it, so a line goes stale only when nothing at that edge absorbs anymore. Grading a pinned absorb line means running the prettier compare by hand; [blank_audit.md](./blank_audit.md#the-absorb-pin--the-blank-drop-class) carries that harness and its traps — and grade the `--json` `absorb_variants` work-list (one row per class × textual shape) rather than the per-class reproducers, which under-report by a measured ~40×.

## Blank-Fabrication Audit (`fabrication:audit`)

The **pristine** counterpart to `blanks:audit`. That audit MUTATES a seed — injects a blank into a code gap and grades the response — so its subject is how the formatter reacts to a blank the author *did* write. This one never mutates: it formats the seed as authored and asks whether the output holds a blank run the input did not.

Why it needs its own gate. A fabricated blank is indistinguishable from an authored one once written, so it is silent content the author never approved — and **every other gate is structurally blind to it**:

- **F1 / idempotency** — the fabricated blank is authored as far as pass 2 is concerned, so pass 2 preserves it and the file is a fixed point. The property never trips.
- **`blanks:audit` / `gaps:audit` / `ignore:audit`** — all grade a MUTATED seed, and the first two exempt whole-file format-ignore regions outright.
- **Corpus format compare** — a fabrication prettier also makes is a match, not a divergence.

```bash
# fabrication_audit - format each pristine seed and report every blank RUN the output
# holds that the input did not, minus three structurally sanctioned layout rules.
# Pure Rust, no Deno. Defaults to tests/fixtures; pass dirs/files to audit real code.
cargo run --profile corpus -p tsv_debug --features audits fabrication_audit
cargo run --profile corpus -p tsv_debug --features audits fabrication_audit ../zzz/src
# Also: --json, --update. ~0.2 s over tests/fixtures.
#
# GATED as a RATCHET over `fabrication_audit_known.txt`, keyed by the SHAPE of the two
# lines bracketing the invented blank (`{:catch` ⇢ blank ⇢ `{/await`), not by path — so
# the snapshot is corpus-portable and states the bug rather than a location. Every line
# is a bug; the file shrinking is the goal. Currently EMPTY (born green).
```

**The metric.** Blank *runs*, not blank lines: collapsing `a⏎⏎⏎⏎b` to `a⏎⏎b` removes newlines but not the author's "there is a break here" signal. A finding is `unsanctioned_runs(output) > runs(input)`.

**The three sanctioned fabrications** are structural carve-outs in the audit, deliberately not snapshot lines — mixing sanctioned rules into a known-bug list would make "every line is a bug" false:

1. **Hoisted-section seam** — tsv moves `<script>` / `<style>` / `<svelte:options>` to canonical positions and separates each from its neighbours with a blank. The carve-out is **two-sided**: a glued `</script><div>` puts the closing tag before the run, a glued `</div><style>` puts the opening tag after it.
2. **Empty block body** — a kept-but-empty block section prints in block form, and the empty body between opener and terminator is a blank line (`{:catch error}{/await}` → `{:catch error}⏎⏎{/await}`). Sanctioned by [`empty_branch_collapse`](../tests/fixtures/svelte/blocks/empty_branch_collapse_prettier_divergence/) and [`empty_catch_multiline`](../tests/fixtures/svelte/blocks/await/empty_catch_multiline_prettier_divergence/), whose READMEs state it.
3. **Empty frozen verbatim body** — rule 2 one construct over. A body in a language tsv does not format at that position is copied out between the two delimiter lines the geometry requires, so a whitespace-only body leaves those lines with nothing between them (`<template lang="pug">⏎</template>` → `<template lang="pug">⏎⏎</template>`). Prettier's `preformattedBody` emits it identically. **All three tags reach it**, because the freeze is one rule asked at every position that has a body — a foreign `<template>`, pinned by [`template_foreign_lang_body`](../tests/fixtures/svelte/elements/template_foreign_lang_body/); a foreign `<script>`, pinned by [`nested_script_style_whitespace_only`](../tests/fixtures/svelte/elements/nested_script_style_whitespace_only/); and a foreign `<style>`, pinned by [`style_foreign_lang_nested`](../tests/fixtures/svelte/elements/style_foreign_lang_nested_prettier_divergence/). See [conformance_prettier_svelte.md §Foreign-language embedded bodies](conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies).

   **Known blind spot: the carve-out is keyed on the tag, not the lang.** The line shapes it matches strip attributes, so `<script lang="coffee">` and a plain `<script>` are one token — the sanction exempts a *formattable* body's blank too. What keeps that from being a hole is the printer, not the carve-out: a formattable body cannot reach the shape (empty → `<tag></tag>` with no newlines; whitespace-only → the single delimiter break, adjacent lines with no run between), so a blank there could only have been authored, and an authored one is already in the input count. That premise is pinned by the fixture above; if it ever breaks, the arm goes quiet on the two commonest tags in the corpus rather than failing.

**A section's leading comment run is rule 1 one step removed.** Comments travel with the section, so a glued `<div>block1</div>⏎<!-- comment -->⏎<style>` puts the section's **leading comment** where the tag would be, and the bracketing shape reads `</div` ⇢ `<!--` — a blank prettier emits too. The audit reads forward from the run (`leads_section`): one or more full-line comments (multi-line ones and blank lines between them allowed) ending at a `<script>` / `<style>` / `<svelte:options>` line excuse the run, and the seam blank *between* two comments of that run is the same run read from its middle. Anything else between the comments and the tag — content, or text on a comment's closing line — refuses, which is what keeps this narrower than "a blank before some comment" (a widening once refused for blinding the audit to a whole class of real fabrications). Surfaced by the `svelte/script/ordering` variants that author a comment glued to the last template node; before them an already-formatted corpus carried the blank in its input, so the counts matched and the gap was latent.

**Blind spots.**

- **Net-zero.** The metric compares counts, so a run fabricated in one place while another is dropped elsewhere in the same file nets out and is missed. Closing it needs a position-preserving alignment between input and output, which reflow (and section hoisting, which relocates blanks wholesale) makes unavailable.
- **Vacuous on fixed points.** Where `format(S) == S` the property holds by construction, so over a corpus of already-tsv-formatted files the audit adds nothing over F1. Its yield is on **pristine, not-yet-formatted** code — exactly where a first-format fabrication would otherwise go unnoticed, because every later format is a fixed point.
- **It never MUTATES, so it can only see shapes the corpus already contains** — and that is where this audit and the injection ones leave a hole *between* them rather than in either. `gaps:audit` splices a `multiline` block comment into every gap, which is precisely the payload an interior-newline fabrication needs, but its oracle is the print-once ledger: a comment that is emitted exactly once and merely grows a blank above it is not a finding there. This audit has the right oracle and never generates the payload. So a fabrication that fires only on an authoring absent from `tests/fixtures` is invisible to the pair — the [comments.md](./comments.md) hazard-5 scans were found by hand-writing the shape and diffing against prettier, at three gaps at once, with every gate green. Once such a shape is pinned as a fixture the hole closes for *that* shape (its input is a fixed point, so a regression trips F1 and this audit together); the class stays open. **Standing lead**: a mutating leg here — the injection audits' `multiline` payload, graded on blank runs instead of the ledger — is the discovery arm neither audit currently is.
- **Shape attribution.** A file trips on a count, and every unsanctioned run in *that file* then contributes its shape. So a tripped file can pin an innocent shape alongside the guilty one. Harmless while the snapshot is empty; if it fills, read a line as "a shape present in a file that fabricated", not "this shape fabricated".

## Comment-Census Audit (`census:audit`)

The whole-comment conservation gate: does every comment the author wrote survive formatting, byte-for-byte (modulo re-indent)? Per file, lex the comment trivia off the raw INPUT and the raw formatted OUTPUT — with the audit's own trivia scanners, **never** `parse().comments` — and compare the interior **multisets**, per language bucket. A drop, a duplication, a merge, or an interior rewrite is a plain arithmetic imbalance, no matter which internal layer caused it.

Why it needs its own gate: every other comment instrument reads a channel the parser controls. The print-once ledger guards what a format entry *registered*; `parse().comments` is what the parser chose to carry. A comment a parse path consumes without registering (the CSS `skip_boundary_whitespace_and_comments` class that motivated this audit) never existed as far as those instruments know — the corpus stays green **by absence**, and every corrupted output in that family was a format fixed point, so F1, roundtrip, fuzz, and the authoring audit were all structurally blind too. The census's independence from the parser's comment carrying is its entire design.

```bash
# census_audit - format each pristine seed, lex comment trivia from BOTH raw sides with
# self-contained scanners (audit/census.rs), and compare normalized interior
# multisets per language bucket: `ts` (TS-family files, <script> islands, template
# {expressions}), `css` (.css files, <style> islands), `template` (Svelte <!-- -->).
# MISSING = dropped comment; EXTRA = duplicated/fabricated one; a merge or interior
# rewrite shows as a MISSING + EXTRA pair. Pure Rust, no Deno.
cargo run --profile corpus -p tsv_debug --features audits census_audit                # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits census_audit ../zzz/src  # a real codebase
# Also: --json, --update. ~0.35 s over tests/fixtures.
#
# GATED as a RATCHET over `census_audit_known.txt`, keyed (path, bucket, direction) —
# file-level, like the compile validation ratchet (the file IS the reproducer). Born
# EMPTY: over tests/fixtures it stands as the tripwire that keeps the CSS parse-time-drop
# class it was argued from closed. Whole-comment drops are sanctioned in exactly ONE place — the CSS CDO/CDC
# `<!-- ... -->` span, which tsv (matching parseCss) discards WHOLESALE, CSS between the
# markers included — and that carve-out lives in the scanner (those comments never enter
# the input multiset), so a snapshot line is always a bug. Rejected inputs make no
# format claim and are skipped; a format PANIC is counted, not gated (the panic gates
# own that class).
```

**The scanners** (`audit/census.rs`) are deliberately self-contained rather than driving the product lexers: TS comment *extents* depend on parser context (a regex body is opaque only because the parser said "regex here"), so a raw `next_token` loop mis-lexes real code — and an instrument sharing the product lexer's extent rules would inherit its bugs. TS handles strings, template literals (interpolation stack included), and regexes via the classic previous-token heuristic; CSS handles strings and unquoted `url()` opacity; Svelte is a lexical mode machine — `<script>`/`<style>` raw-text islands bounded by the first matching close tag (exactly Svelte's own rule, so a `</script>` inside a JS string bounds identically), `{...}` expressions in text, attribute, and quoted-attribute-value position, block sigils stepped over so `{/if}` is never a regex head. Interiors normalize by **exactly the line-edge trim the printer is licensed to make, which is a different trim per comment KIND** — prettier's `printComment` transcribed, since that is what tsv mirrors: a line comment (and the hashbang) is emitted `.trimEnd()`-ed, so its trailing edge is trimmed; an *indentable* block (`*`-aligned) reindents, so each of its lines is trimmed both ends; and every other block — single-line, or multi-line and non-indentable — plus a Svelte `<!-- … -->` is emitted verbatim and gets **no trim at all**, so every line edge there is content. The class is JS `\s` (`tsv_lang::is_js_whitespace`), because the trims it models are `String.prototype.trim*` calls; a `<CR>` fold runs first, through the format path's own `normalize_carriage_returns`.

**Where the yield is.** Over `tests/fixtures` the gate is a cheap standing tripwire; the discovery arm is external corpora. Its first sweep over the prettier suites found a live `as const` **code swallow** (`(1 // comment⏎) as const;` → `1 // comment as const;` — the code after the paren pulled into the comment) plus four line-comment **merge** sites (`// a⏎// b` → `// a // b`, the second comment demoted to text) — all invisible to every other standing gate. Point it at real code after any parser/printer comment change.

**Blind spots.**

- **Position-blind by construction.** The multiset compares interiors, not placements — a comment relocated anywhere in the document (even to a semantically wrong place) balances. Placement is `binding:audit`'s and the fixtures' remit.
- **Same-content cancellation.** A dropped `// x` plus a fabricated identical `// x` elsewhere in the same file nets zero, the same net-zero blindness `fabrication:audit` documents.
- **Instrument-symmetry residue.** The scanners misread rare shapes (a regex after `)`, post-`}` division) — but they misread input and output with the same eyes, so the phantoms cancel. A false positive needs the formatter to rewrite text the scanner misreads *differently* across the two sides; none observed over tests/fixtures, zzz, svelte src, or the prettier suites.
- **As-authored only.** Like every pristine audit, a drop in a gap no corpus file puts a comment in stays invisible — `gaps:audit` is the injection arm for that class (with the ledger, not the census, as its oracle).
- **The kind-aware trim is per KIND, not per BUCKET.** `is_indentable_block_comment` has exactly one printer *emitter* behind it — `tsv_ts`'s `build_comment_doc` (its other callers, via `tsv_lang::is_indentable_block`, are layout gates that reindent nothing) — so the reindent licence the Block arm grants belongs to the TS printer alone. A `*`-aligned comment in the **css** bucket gets it too, where the CSS printer in fact emits every comment verbatim (conformance_prettier_css.md §CSS: Comments, "the comment interior stays verbatim"). Nothing rewrites there today, so this hides no live bug; it is why a future CSS comment re-indent would land silently.
- **The Svelte scanner's own attribute terminator is ASCII.** `scan_svelte_tag`'s unquoted-value run stops at `[ \t\n\r>{]`, where Svelte's `regex_invalid_unquoted_attribute_value` spells that class as JS `\s` — so a comment after a non-ASCII-separated unquoted value (`<div a=x<NBSP>/* c */>`) is read as part of the value and never counted. Symmetric, so it cannot false-positive; it is a blind spot at exactly the spellings the parser's own terminator was fixed for. The scanners are deliberately self-contained (above), so the fix is a deliberate one, not a share.
- **An indentable block's first and last lines are over-trimmed.** prettier trims line 0's trailing edge and the last line's leading edge only, and so does tsv's emitter — but the census trims both ends of every line, so a rewrite of line 0's *leading* run or the last line's *trailing* run balances.

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
cargo run --profile corpus -p tsv_debug --features audits width_audit ../zzz/src
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

**A third component, `inner`, keeps a weld out of the fattest shapes.** The two ends put a whole comment and two comments *welded onto one line* in the same bucket: `<!-- a -->` and `<!-- a --><!-- b -->` both open `<!--` and end `-->`. That matters because the fattest shape is exactly that one — measured over `tests/fixtures`, `<!--…-->` carries 218 of the 480 over-width lines (45%, across 134 files), and **every one of them is a single whole comment**. So its members are all *forced* overruns (tsv never rewraps a comment interior), and a weld is the only bug the silhouette could ever hide — the same class the trailing-run comment emitters have produced before, and one the ledger, census, F1 and round-trip are all blind to. `inner` records whether a `-->` or `*/` closes before the line does (`-` when none), rendered spliced (`head…-->…tail`). It costs **one** shape over `tests/fixtures`: the three `IDENT…WORD` lines that were mid-line comment glue split off from the 43 ordinary ones, and `<!--…WORD` becomes `<!--…-->…WORD` outright. Neither marker can occur inside the comment it closes, so a whole comment never reads as a weld.

⚠️ **What a non-`-` `inner` means over real code is NOT what it means over `tests/fixtures`, and the difference was mis-stated here before it was triaged.** Over `tests/fixtures` the only interior closers are the two `-->` weld shapes just described (`<!--…-->…WORD` and `IDENT…-->…WORD`) — no JSDoc-cast or string-interior kind occurs there. Over real code (`../svelte/packages/svelte/src` + `../zzz/src`: 1,255 overruns, 91 shapes) they mint 13 shapes / 24 lines, and reading every one of those lines splits them two ways:

- **9 shapes / 13 lines are minted by a genuine interior comment that is not a weld** — overwhelmingly the JSDoc cast (`… /** @type {T} */ (expr) …`), which really does close a block comment mid-line. `inner` is reporting the truth; it just isn't reporting a bug.
- **4 shapes / 11 lines are the mirror false positive** — a `-->` or `*/` inside a *string*, *template*, *regex*, or the text of a `//` comment, read as interior with no comment involved. Only one of the four is the template-literal case (Svelte's migrator building `<!-- @migration-task … -->` text); string literals, regex literals and comment text produce it too.

Two of those nine are **mixed**, which is the silhouette doing what a silhouette does rather than a defect: one holds a real cast on one line and template-built comment text on another, and one holds both on a single line (a real `*/` cast inside a template whose `-->` is text). So the split above is per-shape, and a shape is not a homogeneous cause.

So the triage note in the snapshot header holds — a new `inner` shape is a **question**, not a verdict — but the likeliest answer on JS is "a JSDoc cast", not "a template built comment text". The gate pays nothing either way: it grades only the default corpus, a run pointed elsewhere reports without grading, and a false one surfaces as a new shape to triage rather than a wrong verdict on a pinned one.

⚠️ **A rejected design worth not re-deriving: the render-time hook.** The tempting version instruments the renderer — a break opportunity *is* a `Line` doc node, so "an over-width line that still held a flat `Line`" needs no lexing and no carve-out list, and forced overruns (a comment, a string, a `<pre>` body: all atoms with no `Line` inside) stay silent by construction. It was built, unit-tested, and **rejected on evidence**: it is blind to exactly the class it was built for. The mid-run comment bug *removed* the break point — it baked the boundary space into the preceding word — so there was no unspent seam to find, and reverting the fix left that check reporting **zero** while the output grew seven over-width lines. A missing seam is invisible to a check that looks for unspent seams. Re-test against a reverted fix before reviving it.

**Blind spots.**

- **Not a bug list.** A pinned shape is a *kind of line that exists*, not a defect. Triage a new one against §Print Width Philosophy before pinning it; the sanctioned overruns are real and numerous (~480 lines over `tests/fixtures`, dominated by fixture prose headers a formatter never rewraps).
- **Shape collision — the residual blind spot, and it is NOT closable by a fourth key component.** A width bug whose line happens to open, close inside, and end like an existing pinned shape passes. The key is a silhouette, not a proof; it catches new *kinds*, and a same-kind regression needs a fixture. The distribution says how concentrated that risk is: the fattest shape holds 45% of the lines and the top three hold 65%, so most of the corpus's absorbing power sits in a handful of buckets. `inner` (above) drains the specific bug class the fattest ones could hide; what remains is a *breakable* line — one with a real seam tsv failed to take — landing on a pinned silhouette. No third component separates that, because **nothing in the finished text distinguishes a seam tsv declined from one it never had** — that is a property of the artifact being measured, not a gap in the key, which is why no amount of further silhouette engineering reaches it. The rejected render-time hook (above) is the design that tried to read the seam instead of the text, and it is blind for its own, worse reason.

  Worked example, found by triaging this audit over real code rather than hypothesized: tsv granted the flat test-call layout to `test('<long name>', (a, b) => { … })` and broke the callback's *parameter list* to chase the width, where prettier keeps the parameters flat — and to two 3-argument shapes prettier's `isTestCall` excludes outright, where prettier breaks every argument out and holds 100 (see [conformance_prettier.md §Print Width Philosophy](./conformance_prettier.md#print-width-philosophy)). Those emitted an over-width line ending in `(`, whose shape is `svelte IDENT…(` — **already pinned**, so the ratchet stayed green on all of it. That is the blind spot behaving exactly as described, and the only thing that reached it was a fixture with a parameterized callback: `test_functions`, the fixture that pins this layout, uses only parameterless ones, so the two cases are held by [test_functions_params](../tests/fixtures/typescript/expressions/calls/test_functions_params/) and [test_functions_timeout](../tests/fixtures/typescript/expressions/calls/test_functions_timeout/) instead.

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
cargo run --profile corpus -p tsv_debug --features audits ignore_audit ../zzz/src
# Also: --json, --report, --jobs N, --limit N, --update. Build with `--profile corpus`
# (optimized + panic=unwind) so a formatter panic is caught + reported.
#
# GATED as a RATCHET (like gap_audit / blank_audit): `ignore_audit_known.txt` pins the known
# findings per (KIND, position). ⚠️ Unlike those two, the file is NOT a burn-down list: the
# head families (list items, value heads, paren / assignment / statement / declaration heads)
# are closed, and the remaining UNHONORED positions are predominantly expression-INTERIOR —
# a directive written mid-expression, rare in authoring and a position where prettier's own
# behavior is emergent rather than designed — held under a standing SANCTION rather than
# queued for opt-ins. Lines outside that class (the UNSTABLE relocation transients, a few
# non-interior positions) remain ordinary pinned bugs. The gate's job is unchanged: a NEW
# (kind, position) pair, a STALE one (no longer fires), or any PANIC (a crash on the
# injected directive — unpinnable) FAILS. Keyed
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
# are a follow-up — and the template one has the most reach of any uncovered surface, because there
# the gap between the directive and the node it freezes is itself whitespace the printer emits, so
# getting it wrong is render-visible rather than a layout choice (conformance_prettier_ignore.md
# §Format-ignore directive); whitespace-perturbation only (a quote/paren-only reformat is invisible — Arm B's
# remit); only format fixed points injected.
```

`deno task ignore:audit:update` regenerates the snapshot after adding a printer opt-in (a now-honored position goes stale → drop its line) or fixing a misbinding, over-freeze, or relocation transient; it refuses a narrowed run.

⚠️ **Blind spot: this audit injects a DIRECTIVE, `gaps:audit` injects a COMMENT, and
neither injects the pair — so a bug that needs both is invisible to both.** Live case,
since fixed: an object property under `prettier-ignore` whose value carried
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
# deeply-nested but ordinary file). Builds synthetic nested inputs across one axis per
# candidate-building construct (svelte elements / {#if} / {#each} / {#await} /
# sibling-`>` dangle / {#snippet} / {#key}; ts member chains, ternaries, conditional
# types, nested calls, and the expand-last arrow family — plain, multi-arg, `new`,
# chain, object-body, conditional-body, `function`, and curried, the last in untyped /
# typed / `new` / chain / object-TERMINAL spellings, the object-terminal one again in
# single-argument, multi-argument and chain forms) at increasing depth, failing if the
# doc-node count grows faster than ~depth^3. An axis earns its place by REACHING a
# builder no other axis does; the curried axes came from real 2^depth regressions that
# the family as it then stood could not see. Deterministic, pure Rust, no Deno. Exits 1
# on any super-linear case.
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
`tsv_debug` has exactly that shape — a `[[bin]] name = "tsv_debug"` beside a lib of the same
name — so with `audit/` and `cli/` declared only in `main.rs` (two trees that are ~36k of the
crate's ~56k lines) the gate would grade roughly a third of it and report green, a deliberately
broken link in either passing. The crate is therefore one lib, with `main.rs` a shim over `cli`,
which also keeps the shared modules from compiling twice under two roots. The second
hole is `#[cfg(feature)]`: five `audit` submodules sit behind `comment_check`, so a default-feature
build cannot resolve a link that names them — hence `--all-features`, which reaches gated code in
every crate, not only this one. A **third** hole is `#[cfg(test)]`: rustdoc does not document test items under any feature
set, so **every doc link inside a `#[cfg(test)] mod tests` is ungated** — including the ones on
test-only `static`s and helpers, which is where a codebase like this one parks its oracles. A
rename that leaves a back-reference in a test module's docs is invisible here and is caught only
by reading the diff. (Found in `tsv_ts::lexer::token`, where the `KEYWORDS` oracle's doc still
named a `const` that a perf change had replaced.)

All three holes fail the same way, which is the thing to remember about
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
and only the task's flags cover every workspace crate.

**Blind spots.** It checks that a link *resolves*, never that the surrounding prose is
true. A doc that describes behavior the code no longer has passes cleanly; so does a
correct link attached to a wrong claim. Two of the three findings above came with prose
errors *beside* the dead link, and those needed reading, not the gate.

**Build world.** `cargo doc` builds its own rmeta; it does not share the `--profile corpus`
world the other `check` audits use, so it adds a short compile of its own.

## Wire-Type Drift Check (`check:ast-types`)

```bash
# scripts/check_ast_types.ts — three arms over crates/tsv_wasm/types/tsv_ast.d.ts:
# (A) `tsv parse` a curated sample set, (B) assert every wire discriminant the fixture
# corpus produces is declared, (C) type a computed cover of the corpus against the .d.ts.
# A and C share one generated file and one `deno check`. ~20 s when the generated content
# changes (nearly all arm C); an unchanged rerun hits deno's check cache at ~4.5 s.
deno task check:ast-types
```

**What it proves.** That `crates/tsv_wasm/types/tsv_ast.d.ts` still describes what the
converter emits. That `.d.ts` is hand-maintained and it **ships** — `@fuzdev/tsv_parse_wasm`
bundles it, so it is the wire contract consumers type against, and nothing in the Rust build
knows it exists. TypeScript's excess-property checking on the generated object literals catches
both directions of drift: a field the converter emits that the `.d.ts` lacks ("may only specify
known properties"), and a field the `.d.ts` requires that the converter does not emit
("Property 'X' is missing"). Renames and value-type changes fall out the same way.

**Three arms, because "does it drift" and "is anything ungraded" are different questions.**

- **A — writer conformance.** The curated `tsv parse` samples. Grades the LIVE writer, so it is
  the arm that fails the moment a `write_*` changes.
- **B — wire-type coverage.** Every `type` discriminant present in the committed fixture wire
  must be declared, or listed in the script's `OPAQUE_WIRE_TYPES` — the CSS node vocabulary,
  which `StyleSheet.children` types as `unknown[]` by design, plus one non-node: `Boolean`, a
  DATA key inside the evaluated `<svelte:options customElement>` props config that the
  text-level scan cannot tell from a discriminator. A set difference over
  text — no type-checking, and the widest reach of the three. A stale opaque entry also fails,
  so the list cannot quietly outlive its reason.
- **C — fixture-corpus conformance.** Arm A over inputs nobody curated: a computed minimal cover
  of the corpus's `expected*.json`, typed against the same `.d.ts`. Stronger than arm A in two
  ways, not merely wider — the committed `expected.json` is the CANONICAL parser's output
  (`fixtures_update_parsed` regenerates it), so this arm grades the `.d.ts` against the ORACLE
  rather than against tsv's own opinion; and its inputs are the whole tree.
  `expected_ours.json` wins where a fixture declares a parser divergence.

⚠️ **The cover's target is the field SLOT — `ParentType.key -> ChildType` — and getting that
unit right is the whole design of arm C.** Excess-property *and* value-type checking fire per
(interface, key, value shape), so a cover is exactly as strong as the unit it spans. This was
wrong twice, both times caught by canarying against a known-real bug rather than by reasoning:

| cover unit | files | found | missed |
| --- | --- | --- | --- |
| node **type** | 99 | — | a wrong `TSInterfaceDeclaration.extends` (the node was reached via a class `implements` clause, the position never) |
| **position** (`Parent.key`) | 267 | that one | a loc-less `TSTypeAnnotation` on block bindings, two attachment hosts, a `TSExpressionWithTypeArguments.expression` that is a `TSQualifiedName` — each a position already covered by some *other* value shape |
| **slot** (`Parent.key->Child`) | 770 | all of them | — |

The cost was measured before choosing: ~1 s for positions against ~19 s for slots, on a `check`
whose total is minutes and whose cost is dominated by build configuration rather than audit
code. Two things keep the price honest. Slots inside an OPAQUE region (parent or child named in
`OPAQUE_WIRE_TYPES`) are excluded from the cover target — they type against `unknown`, so
covering them graded nothing (2893 → 2636 slots, 770 → 698 files, ~20 s). And the figures are
CACHE-MISS costs: `deno check` caches by content, so the full price is paid exactly when the
`.d.ts` or a covered `expected*.json` changed — the commits where the arm has fresh work — and
in cold-cache CI, while an unchanged rerun costs ~4.5 s. Reverting is a one-line change in
`wire_field_slots`; the trade it buys is precisely the class of bug this gate exists for. Same
lesson one level up from the one that motivated arm B, and then again one level up from that.

**Arm C composes with `fixtures_tests`, and that is where its reach over *tsv* comes from.**
Arm C never runs tsv's parser — it types the **committed** `expected*.json`. What makes that a
statement about tsv is the other gate: `cargo test --test fixtures_tests` holds tsv's parse
equal to `expected.json` for every fixture. Compose the two and every field position tsv emits
over the corpus is typed, with neither gate doing both jobs. Two consequences worth stating
because they are easy to forget in either direction: arm C **cannot catch a parser bug** (that
is `fixtures_tests`' remit, and a parser regression leaves arm C perfectly green), and the
composition is only as strong as its weaker link — a narrowed `fixtures_tests`, or an
`expected.json` gone stale against the live oracle, breaks the conclusion about *tsv* while arm
C keeps reporting truthfully about the `.d.ts`. Oracle freshness is
[`conformance`'s](../CLAUDE.md#corpus-comparison) job, as always.

**Blind spots.**

- **A position under an `unknown`-typed field is not graded** — arm B covers only the *names*
  under it. Five such fields remain, and the shape of the list is the point: three are the CSS
  tree below the stylesheet root (`StyleSheet.children` / `.attributes`,
  `StyleSheetFile.children`), opaque by an explicit decision; the other two are single nodes
  (a regex `value`, which acorn itself emits as `{}`, and `<svelte:options customElement>`,
  which the corpus samples four times). Everything else is typed. Widening one is how a region
  gets *behind* the gate rather than merely inside it — `Script.content` was the big one, and
  typing it (`unknown` → `Program`) is what put the `<script>` side of all ~4200 `.svelte`
  fixtures under arm C at all.
- **It asks only whether the two sides AGREE on a shape arm A reaches** — never whether that
  shape is the one acorn / `parseCss` / Svelte actually produce. Arm C narrows this a long way
  by grading the committed oracle wire, but the parse conformance gates remain the authority.
- **Arm B is a NAME check over raw text** — two failure shapes follow, both live in the corpus
  and both absorbed deliberately: a discriminant declared for an unrelated reason counts
  (`AttachedComment`'s `'Line' | 'Block'` covers the CSS `Block` node's name for free — which
  is why `Block` is listed opaque anyway), and a DATA key spelled `"type"` reads as a
  discriminant (`Boolean`, the `customElement` config entry above). Arm C is what walks the
  typed paths; a reachability-scoped name check would remove the first shape and is a candidate
  follow-up.

The per-field checklist in
[crates/tsv_wasm/CLAUDE.md §TS Type Maintenance](../crates/tsv_wasm/CLAUDE.md#ts-type-maintenance)
is what carries a writer change; this gate is the backstop.

## Canonical-Pin Agreement Audit (`pins:audit`)

```bash
# scripts/check_canonical_pins.ts --pins. Read-only Deno, no build, no sidecar.
deno task pins:audit
```

**What it proves.** That the canonical oracle is *one* version rather than five — and, since
a version and an option are both pins on what the oracle EMITS, that the prettier OPTIONS
agree too (`benches/js/lib/canonical.ts` `PRETTIER_OPTIONS` vs the sidecar's inline call,
`check_prettier_option_agreement`). The five pin sites that must be identical:

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

The doctrine this gate enforces — what counts as "one document", which holds are deliberate, and
the enumerated dual-stable remainder — is [§Authoring Convergence
Philosophy](./conformance_prettier.md#authoring-convergence-philosophy). ⚠️ Read the blind spots
there before treating a green run as evidence: this audit fails on **non-idempotency only**, so
the `diverge (dual-stable)` bucket — sanctioned holds and accidental ones alike — is reported but
never graded and carries no ratchet file.

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
# 2-cycle was green on fixtures while failing on ../zzz).
cargo run -p tsv_debug authoring_audit                  # audit tests/fixtures (pure Rust)
cargo run -p tsv_debug authoring_audit ../zzz/src    # audit a real codebase
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
cargo run -p tsv_debug authoring_audit ../zzz/src --prettier --dump-dir /tmp/audit
```

## Print-Width Razor Sweep (`razor:audit`)

```bash
# razor_audit - walk each Svelte seed ACROSS the print-width razor and grade the
# output at every width. Pure Rust, no Deno. Defaults to tests/fixtures, .svelte
# seeds only. Exits 1 on any finding.
cargo run --profile corpus -p tsv_debug razor_audit                 # sweep all fixtures
cargo run --profile corpus -p tsv_debug razor_audit --width 8       # cheaper/narrower sweep
cargo run --profile corpus -p tsv_debug razor_audit ../zzz/src   # sweep a real codebase
# Also: --json, --limit N.
```

**The dimension no other gate varies.** `authoring:audit` mutates the *spelling* of a
document's whitespace, `fuzz:audit` its *structure*, `gaps`/`blanks` inject at *sites* — every
one of them formats each document at the single width its content happens to have. The Svelte
inline-layout family's bugs are **width-keyed**: a rule fires only once a construct crosses
column 100, so a document one character short of the razor exercises none of it. This audit
supplies that variation by padding a text word, which shifts everything downstream by `k`
columns, so `--width k` formats each seed at `k + 1` geometries instead of one. Seeds are
padded from their own **fixed point**, not from the bytes on disk, so the sweep perturbs one
known geometry rather than confounding width with the first format's reflow.

**Two properties per width, and neither subsumes the other:**

- **F1** (`format(out) == out`) catches the half where two authorings of one document disagree
  forever.
- **The line-head boundary space** catches the half F1 **cannot see**. When a text run's
  leading boundary is baked into its first word instead of claimed as a break point, the space
  rides the fill's fresh-line drop to the head of a continuation line — and after a predecessor
  whose break is *forced* (a tag, a component, a `svelte:*`), that mangled form keeps its own
  break under every authoring, so it is **its own fixed point**. F1, the fuzzer, the round-trip
  and `render:audit` all pass straight through it (the space is render-free — Svelte collapses
  the run to one space either way); only a column separates the two forms.

**The oracle is structural, and that is measured, not assumed.** A raw text scan for "line
starts with indent + a space" is unusable: 406 lines across the fixture tree's own *output*
files already do, dominated by block-comment continuations (` *`, ` */`), expression alignment
(` )`, ` }`), multiline attribute values and `<pre>` content — all legitimate. So the scan asks
the **parse**: a violation is a space at a line head *inside a fragment `Text` node*. The
exclusions mirror the printer's own verbatim-emission dispatch rather than re-deriving a rule
— `<pre>`/`<textarea>`, `<script>`/`<style>`, a foreign-language `<template>`, non-`Fragment`
text decodings, and format-ignore in **both** its node and **range** forms. (Missing the range
form was a real false-positive class caught on the first run: it accused 476 lines of the
author's own frozen bytes.)

**Validated against a known bug rather than asserted.** Built pre-fix (the tail-boundary claim
reverse-applied, gated on an `objcopy .text` comparison so no stale binary could fake either
column), it reports 2412 findings — every one inside the fixture that pins that bug, both
kinds, zero false positives; on the fixed tree it reports **0** across 45,905 graded widths.

**What only it can see.** A fused element+tail measurement at an inline-sibling wrap is an F1
break at every width where the wrapped element lays its own content out block-style — and no
other gate can see it, because the strayed pass is reachable only at widths no fixture happens
to sit at; the sweep reaches it past `--width 17` (`inline_sibling_drop_tail_wide_long` pins the
razor). Green over both the fixture tree and real code, and gated in `deno task check` (~5 s at
the default width).

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
cargo run --profile corpus -p tsv_debug --quiet render_audit ../zzz/src
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

**What it is blind to: a shape the corpus does not contain.** It grades the files that exist, so
it proves only that formatting preserved the render *of real code* — a render-visible deletion in
a shape no corpus file happens to carry is invisible here, and stays invisible however far the
corpus grows. Measured, not hypothesized: the boundary space before a **run-ending sibling**
(`<p>text1 <span>x</span> <!-- c -->text2</p>`, and its `{@debug}` twin) was deleted for every
container kind, yet a pre/post format diff over **14,318 real `.svelte` files** moved **zero** of
them — the shape occurs in none of them. The mangled form is also its own fixed point, so F1,
fuzz and round-trip are blind by construction, and the comment instruments (ledger, census,
swallow) count *comments*, never the whitespace beside them. Nothing but a fixture reaches this
class, which is why a render-visible finding earns one even when every corpus gate is green
([inline_adjacent_comment_space](../tests/fixtures/svelte/elements/inline_adjacent_comment_space/)).

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
cargo run -p tsv_debug fuzz --iterations 0 ../zzz/src       # pristine pass only = an F1 sweep
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
cargo run -p tsv_debug lex_diff ../zzz/src --golden /tmp/lex.golden --write  # capture golden
cargo run -p tsv_debug lex_diff ../zzz/src --golden /tmp/lex.golden          # check against it
# Options: --write (capture instead of check), --verbose (first divergent line per file)
```

## Variant Whitespace-Direction Audit (`variants:audit`)

```bash
# variant_audit - the `_compact` / `_spaces` variant names are a matched pair of
# OPPOSITE claims about one input, and the pair is the point: compact removes
# whitespace the formatter normalizes away, spaces adds it, so between them a
# fixture is squeezed from both sides. The N rules prove only that a variant LANDS
# on input; nothing asked which direction it travelled from, and the corpus drifted
# (a `_compact` padding `interface Empty {  }`, a `_spaces` stripping `h1 + p` to
# `h1+p`). A wrong-way variant is a DUPLICATE of its sibling under the other name,
# and the direction it was meant to cover goes untested silently.
cargo run -p tsv_debug variant_audit          # audit (exit 1 on any wrong-way variant)
cargo run -p tsv_debug variant_audit --list   # every graded variant + emptied/widened counts,
#                                               then the ungraded (not whitespace-only) ones
# Also: --json. Gated in `deno task check` via the `variants:audit` task. Pure Rust —
# no Deno, no formatter run.
```

**The instrument.** A whitespace-only variant shares its input's non-whitespace
character stream, so the two align on it: at every position the input holds a
whitespace run and the variant holds the run that replaced it. Exact — no lexer,
no removability heuristic. Two events are graded:

- **EMPTIED** — input's run non-empty, variant's empty. `_spaces` must never.
- **WIDENED** — *neither* run contains a newline and the variant's is strictly
  longer. `_compact` must never.

⚠️ **The newline exclusion on WIDENED is load-bearing.** Raw length over-flags: a
`_spaces` variant joining a deeply-indented input replaces `\n\t\t\t` (4 chars)
with `  ` (2) — shorter, yet plainly adding horizontal space. Comparing lengths
only where both sides are newline-free asks the horizontal question in isolation;
the vertical axis is left to EMPTIED, which no re-indentation can trip.

⚠️ **The whitespace class is `is_collapsible_ws_char`, never `is_ascii_whitespace`.**
Here the class decides how the two files ALIGN, so a character wrongly counted as
whitespace shifts one stream against the other and every later gap is graded
against the wrong partner. The narrow class is also conservative: a real
whitespace character treated as content merely splits one gap into two, both
still graded.

**Scope: the bare suffix only.** Graded names are `unformatted_`,
`unformatted_ours_` and `unformatted_prettier_` followed by exactly `compact` or
`spaces`. Each prefix also names the **reference** the gaps align against — the form
the variant must normalize to, which is `input.*` for the first two (N4/N5) and
`output_prettier.*` for `unformatted_prettier_*` (N8). Aligning every prefix against
`input` would grade that one against a form it makes no claim about, and the mismatch
would surface as an alignment failure — i.e. land in the blind spot below rather than
raise an error. A qualified name (`unformatted_unicode_spaces`,
`unformatted_ours_hug_spaced`, `unformatted_ours_tail_weld`,
`unformatted_compactish`) makes a narrower claim than the bare directional one and
is out of scope by construction — the sanctioned home for a variant that must move
whitespace *both* ways because the weld itself is its subject. The
oracle-generated forms (`prettier_variant_compact`, `variant_compact`,
`divergent_variant_compact`, `prettier_intermediate*`) are prettier's own output,
so grading them would put the audit in an argument with the oracle.

**Vacuity.** `check_graded_nonzero` plus a private `VARIANTS_GRADED_MIN` floor —
the shared `FIXTURES_FORMATTED_MIN` counts formatted files, a different
population. The floor catches the partial collapse zero cannot see: a walk that
still finds fixtures but stops recognizing variants (a bucket rename, a prefix
typo, an extension mismatch). Re-pin deliberately when a variant is renamed out of
scope.

⚠️ **Blind spot: the unalignable variants** (~303 today, ~13% of the variants in
scope). A variant whose non-whitespace stream differs from its reference cannot be
aligned, so it cannot be graded. They are counted, and listed by `--list` / `--json`,
rather than dropped — a silently-excluded class reads as a covered one.

The composition is measured, not assumed: a clear majority differ by exactly a
**trailing comma** before a closer (`}` / `)` / `]` / `>`), then quote flips
(`'x'` → `"x"`), then an added paren or a leading `|`. Only that tail is a *naming*
problem (a whitespace name over a genuinely mixed edit); the trailing-comma class is a
whitespace variant plus one mechanical token `trailingComma: 'none'` deletes, and its
direction is exactly what one wants graded — normalizing a trailing comma before a
closer would recover about half the blind spot. Both that and re-homing the tail are
deliberately outside this gate today.

**What it does NOT prove.** Only the direction, never the *extent*: a `_compact`
variant that empties one gap and leaves twenty removable ones passes. Extent is
nonetheless **decidable** — empty one gap, re-format, and see whether it still lands on
the reference — and measuring it is what says why the gate stops short. About 21% of
the non-empty gaps in `_compact` variants are removable that way, but the rest are
mandatory separators, and forcing maximality would delete injection sites
`gaps:audit` / `blanks:audit` / `ignore:audit` enumerate from the seed text
([docs/gap_audit.md](./gap_audit.md)) — a whitespace edit strands the reproducer of a
pinned bug.

⚠️ **One subclass is held at zero by convention rather than by the gate: a HORIZONTAL
gap touching a block comment.** Comment-adjacent gaps measured 59% removable (816 of
1383) — three times the rest — because gluing is exactly what makes a comment's
binding interesting: `owned_by_node` is *defined* by glue
([docs/comments.md](./comments.md)), so a space-padded comment in a compact variant
exercises nothing its own input does not. Every newline-free one that both formatters
still normalize is glued (334 of the 368 horizontal candidates); the 34 left are
structurally required — tsv refuses the glue (`<div/* c */>` reparses the `/` as a
self-close), or prettier would no longer normalize the glued form back to input
(N3/N10).

**The newline-carrying ones are deliberately left alone** (1015 of them), on the same
reasoning that makes WIDENED horizontal-only above: whether a comment sits on its own
line is *authorship*, not volume — it is what `is_own_line_comment` and
`comment_hugs_next` read — so welding one onto the next line rewrites the authoring
rather than compacting it, and takes a fixture's own explanatory prose down with it.
When adding a `_compact` variant, glue its comments **horizontally** and leave their
lines; a glued form that stops normalizing is the signal to leave that gap, not a
reason to pad the rest.

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
#      docs/*.md, every fixture README, root CLAUDE.md / README.md, each crate's
#      CLAUDE.md, the shipped
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
# key dump. `Refusal::every_variant` (the oracle behind that check) is hand-maintained rather than
# compiler-enforced, but drift is loud: a unit test scans the enum's source for the declared variant
# names and asserts each is present (`refusal_buckets.rs`), and `all_bucket_keys_covers_the_catalog`
# pins the produced key set — a missing variant is two named `cargo test` failures, not silence.
# Pure Rust (no Deno). Exits non-zero on any finding. Gated in `deno task check`.
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
cargo run -p tsv_debug canonicalize_audit ../zzz/src ../gro/src  # real-corpus sweep
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
