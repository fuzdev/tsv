# TypeScript Compiler Corpus Conformance

tsv's TypeScript parser graded against the TypeScript compiler's own test corpus,
with **tsc as the validity oracle**. This is the parser's correctness gate for
TypeScript the way [conformance_test262.md](./conformance_test262.md) is its gate
for ECMAScript: test262 says what JavaScript is, tsc says what TypeScript is.

The corpus is a sibling checkout, `../typescript` (microsoft/TypeScript), pinned by
commit in `benches/js/lib/gate_counts.ts` (`GATE_CHECKOUT_IDS`). Three tools read
it, and only the first is a gate:

- **`deno task conformance:ts-repo`** — the gate this document describes. Entry:
  [`benches/js/diagnostics/ts_repo_compare.ts`](../benches/js/diagnostics/ts_repo_compare.ts).
  A leg of `deno task conformance`, so it runs on release (publish Step 3b).
- **`deno task bench:harvest:ts-repo`** — builds the tsc-corpus **coverage** set the
  conformance bench measures every parser over ([`harvest_ts_repo.ts`](../benches/js/harvest_ts_repo.ts);
  the bench side: [benches/js/CLAUDE.md](../benches/js/CLAUDE.md) §Corpus).
- **`deno task ts-repo:over-acceptance`** — a per-tool **profile** over the files
  tsc's parser rejects, never a gate ([`ts_repo_over_acceptance.ts`](../benches/js/diagnostics/ts_repo_over_acceptance.ts)).

The gate and the harvest share one vocabulary
([`benches/js/lib/ts_repo.ts`](../benches/js/lib/ts_repo.ts)): what a parse unit is,
how a baseline maps to its test, and what a grammar error looks like. They scope
themselves differently on purpose (the gate walks the whole tree and grades `.d.ts`;
the harvest keeps `conformance` + `compiler` and skips `.d.ts`) — each side argues
its choice where it makes it.

## The oracle

**tsc's committed baselines.** The TypeScript harness writes
`tests/baselines/reference/<name>.errors.txt` whenever a compile produces
diagnostics — per settings variant (`<name>(target=es5).errors.txt`) when a test
declares several, so a lookup keys on the un-suffixed test name and gathers every
variant (`baseline_test_key`). A **`TS1xxx`** code anywhere in them means tsc's
grammar rejected the test; none means tsc accepted it. Grammar errors are
target-independent, so a code in any variant counts.

**Why tsc, not acorn.** acorn-typescript is tsv's drop-in **shape** target (the wire
JSON contract), not its correctness oracle: it is both over-lenient and over-strict
against the real compiler. Grading against tsc auto-resolves acorn's leniency cases
to reject-parity with no sanction, and acorn's verdict is kept only as a **sub-label**
on tsv's over-rejections (does acorn share the gap, or is it tsv's alone). The three
oracles are stated once in
[crates/tsv_ts/CLAUDE.md §Architecture Position](../crates/tsv_ts/CLAUDE.md#architecture-position).

**A `TS1xxx` code is not proof of a parser rejection.** The range spans tsc's parser
and its checker's `checkGrammar*` family: ambient-context statements (TS1036),
ambient `async` (TS1040), decorator placement (TS1206) are all checker-raised over a
tree the parser built without complaint. Wherever the difference matters the gate
runs **tsc's parser live** on the same bytes (`createSourceFile(...).parseDiagnostics`,
through `benches/js/lib/tsc.ts`) and reads its verdict directly.

## Scope

- **Root: the whole `tests/cases` tree.** The parser tests are not where parser bugs
  live — the checker and emitter trees are ordinary TypeScript written to trip
  semantic rules, and a parse gap in shallow test code is likelier reachable in real
  code than one in the parser torture suite. A root narrower than the whole tree has
  sat green over real over-rejections in exactly those trees.
- **`.tsx` is skipped** (JSX grammar, out of tsv's scope) and counted.
- **`.d.ts` is graded.** A declaration file is ordinary TypeScript to tsv (no
  declaration mode exists; `tsv format` discovers one like any `.ts`), so a parse gap
  in one ships. Two soft spots come with it, both reported rather than gated: a
  construct valid only under tsc's filename-keyed declaration-file rule (`await` as a
  name in a `.d.ts` module) is a `declaration_file` sanction, and files under
  `tests/cases/projects/` that no scenario compiles have no baseline at all — a
  missing baseline reads as "tsc accepts", and tsc's live parser is what catches the
  one stale emit artifact there.
- **Multi-file tests are split into units.** A test carrying `// @filename:`
  directives concatenates several virtual files; feeding it to a parser whole is
  meaningless. The gate splits it where tsc's harness does (`split_test_units`
  mirrors `makeUnitsFromTest`: the directive name is case-insensitive, the `//` is
  anchored at line start and is exactly two slashes) and grades each **TypeScript**
  unit (`.ts`, `.mts`, `.cts`, and their `.d.*` forms). `.tsx` units, `.js` units (tsc
  parses JavaScript under its own rules), and non-code units (`package.json`,
  `.css`) are counted, never graded. A multi-file test whose baselines carry a grammar
  error anywhere is skipped whole as `multi_file_tainted`: the baseline cannot say
  which unit, so only a clean test makes a positive claim about every unit.
- **Nothing is stripped.** Compiler directives (`// @target: es5`) are comments to a
  parser. The one shape that changes — a `#!` shebang under a directive line — is
  refused by tsc's own parser on the same bytes and lands in `tsc_parser_rejects`.

## Buckets

Every parse unit lands in exactly one bucket. Read down the ladder; the first row
that applies wins.

| bucket | tsv | tsc baseline | attribution | gates? | pinned? |
| --- | --- | --- | --- | --- | --- |
| `accept_parity` | accepts | valid | — | | exact |
| `over_acceptance.parser` | accepts | invalid | tsc's live **parser** refuses too | | exact |
| `over_acceptance.checker` | accepts | invalid | tsc's parser accepts; a checker-side grammar check refuses | | exact |
| `reject_parity` | rejects | invalid | — | | |
| `script_goal` | rejects | valid | tsc reads the file as a script and tsv accepts it at `Goal::Script` | | |
| `tsc_parser_rejects` | rejects | valid | tsc's live parser refuses the raw bytes | | |
| `sanctioned` | rejects | valid | acorn accepts; `TS_REPO_SANCTIONS` | | |
| `gap_known` | rejects | valid | acorn accepts; `KNOWN_GAPS` | | |
| `gap_unexpected` | rejects | valid | acorn accepts; **untracked** | **yes** | |
| `beyond_acorn.sanctioned` | rejects | valid | acorn rejects; `BEYOND_ACORN_SANCTIONS` | | |
| `beyond_acorn.known` | rejects | valid | acorn rejects; `BEYOND_ACORN_KNOWN_GAPS` | | |
| `beyond_acorn.unexpected` | rejects | valid | acorn rejects; **untracked** | **yes** | |

Multi-file units go through the same ladder. Because only clean tests are graded, a
unit is either accept-parity (`units_accept_parity`, pinned with `units_scanned`)
or an over-rejection — the over-acceptance and reject-parity rows cannot occur there.

**The two auto-attributed rows are measurement artifacts, not gaps.**

- `script_goal`: tsv parses at `Goal::Module` by default, where `await` is reserved;
  tsc infers a script for a file with no `import`/`export`, where `await` is an
  ordinary name (`var f = (await) => {}`, `async function await()`). The gate retries
  at `Goal::Script` **only when tsc read the file as a script**, so a module-goal
  refusal of a file tsc calls a module still falls through to the acorn split.
  The goal axis itself: [conformance_test262.md §Strict Mode Only](./conformance_test262.md#design-decision-strict-mode-only-explicit-goal-axis).
- `tsc_parser_rejects`: the baseline is silent because the harness compiled
  something other than the bytes on disk — a UTF-16 file the harness decoded and every
  tool here reads as UTF-8 (tsc's TS1490), a shebang the harness's directive-strip
  moved to byte 0, a stale artifact no scenario compiles. tsc's parser refusing the
  same bytes is the proof.

## The ledgers

Four lists, one bar. A **sanction** is an over-rejection tsv keeps on purpose; a
**known gap** is one tsv will fix, tracked so the gate is green at baseline and only
a *new* over-rejection fails it. Known-gap lists may only **shrink**. Every entry is a
path-substring pattern (`<basename>.ts`, or `/<basename>.ts` where one basename is
a suffix of another; a multi-file unit matches `<path>::<unit>`) with a reason, and
every ledger is **freshness-checked** on a full-corpus run: an entry that matches
nothing fails the gate, so a fixed gap must leave its ledger the day it is fixed.

- **`TS_REPO_SANCTIONS`** ([`parse_sanctions.ts`](../benches/js/lib/parse_sanctions.ts))
  — acorn accepts, tsv declines deliberately. The bar is the same as the acorn suite's
  list beside it: deprecated syntax tsv drops for its successor (import assertions
  `assert {…}` for `with {…}`), or a cataloged taste divergence. "The canonical parser is
  merely lenient" is never a sanction.
- **`KNOWN_GAPS`** (in the gate) — acorn accepts, tsv is wrong. Empty at baseline.
- **`BEYOND_ACORN_SANCTIONS`** (in the gate) — acorn rejects too, and tsv keeps
  rejecting. Four categories, each an argument:
  - `grammar` — a rule that lives in a **production** (ecma262's, the TS grammar's,
    or a finished proposal's): a spelling the grammar has no production for. tsc's
    parser reads past it for recovery and its checker reports; the spec governs.
    A for-in/of head with a call, `new`, `this`, update, literal, or bare-cast target;
    `with`; a `#name` outside the three PrivateIdentifier productions; a rest element
    that is not last; `new.targt`; a non-string import-attribute value; a default or
    named clause behind `import defer`.
  - `tsc_recovery` — a TypeScript-only position where tsc's parser is deliberately
    **looser than the published grammar** so the checker can say something useful: an
    expression where a heritage type reference belongs (`implements A?.B`), any
    property name where an enum member's identifier belongs (`enum E { 1 }`). No other
    parser follows; matching it means tracking tsc's recovery strategy, not its grammar.
  - `tsc_extension` — syntax tsc parses that is not TypeScript at all: JSDoc type
    forms in a `.ts` file (`?T`, `T!`, `<?>`, parsed so TS8020 can be reported), U+0085
    as a line break (outside ECMAScript's WhiteSpace and LineTerminator).
  - `declaration_file` — valid only under tsc's filename-keyed `.d.ts` rule.
- **`BEYOND_ACORN_KNOWN_GAPS`** (in the gate) — acorn rejects too, and tsv is wrong.
  Two categories:
  - `reject_flip` — a shape already graded as a pending reject→defer flip: tsc's
    parser accepts and its checker reports, so under the reject-vs-defer line below it
    belongs on the defer side. The flip closes the entry. Signature parameter defaults
    and parameter properties, a field named `constructor`, an accessor `this` parameter.
  - `beyond_acorn_gap` — a genuine gap acorn shares: the grammar has the production,
    tsc and prettier's `typescript` parser accept, and tsv should diverge from acorn
    toward it. The fix is a `_svelte_divergence` fixture (canonical rejects, tsv
    parses). A typed arrow inside a conditional's consequent (`a ? (b) : c => d : e`),
    `for (using x = …;;)`, a string-named import specifier behind `type`.

## Reading the numbers

- **`accept_parity` up is a parity gain, down is usually a regression** — but it counts
  only the agreeing-accept half. A file leaving for `reject_parity` (tsv learning to
  refuse what tsc refuses) drops it with agreement unchanged. The two `unexpected`
  buckets staying empty is what settles a drop.
- **Over-acceptance is two numbers, and only one is a question.** `checker` is the
  deferred-early-error posture by design ([CLAUDE.md §Strict Mode Only](../CLAUDE.md#strict-mode-only),
  [conformance_svelte.md §TypeScript Corrections](./conformance_svelte.md#typescript-corrections)):
  tsc's own parser built the tree and a later grammar check refused it, exactly where
  tsv's future diagnostics layer will refuse it. `parser` is tsv taking what tsc's parser
  refuses; each of those is a question for the reject-vs-defer line with two possible
  answers — a production tsv should also refuse (a bare `super` as a value, say), or a
  recovery-only reject tsv is right to defer (`++await x`, where the production holds and
  only the assignment-target early error is broken). Both are pinned so a widening moves
  a number somebody reads; neither fails the gate.
- **The baseline-code histogram is the diagnostics layer's backlog.** `-v` prints file
  counts per `TS1xxx` code over both over-acceptance buckets (`--json` carries it as
  `over_acceptance.codes`). The heads are the deferred early-error families by name:
  strict-mode reserved words and `arguments` (TS1212, TS1100, TS1213), the ambient
  family (TS1036, TS1183, TS1039), duplicate properties and `delete x` (TS1117,
  TS1102), regex flags (TS1501). It is measured, so it never goes stale the way a
  hand-kept table does.
- **`script_goal` and `tsc_parser_rejects` should be boring.** A rise in either is a
  checkout move or a harness change, not a parser change.

## Pins

`TS_REPO_PINS` in [`gate_counts.ts`](../benches/js/lib/gate_counts.ts): `scanned`,
`accept_parity`, `over_acceptance_parser`, `over_acceptance_checker`, `units_scanned`,
`units_accept_parity`. Exact, so a checkout pull, a widening, a collapsed oracle, or a
harness change must be re-pinned deliberately. Each pin's argument is on the constant;
the re-pin ritual and provenance are [gate_counts.md](./gate_counts.md).

## Command interface

```bash
deno task conformance:ts-repo            # build the corpus FFI, then run the gate
deno task conformance:ts-repo:run        # skip the rebuild (freshness-guarded; BENCH_STALE_OK=1 overrides)
deno task conformance:ts-repo:run -v     # every ledgered and auto-attributed entry, plus the code histogram
deno task conformance:ts-repo:run --json 2>summary.txt >report.json
deno task conformance:ts-repo:run ../typescript/tests/cases/conformance/parser   # one subtree
```

Summary to stderr, JSON to stdout with `--json`. A subtree run grades a slice and
skips the full-corpus hygiene (ledger freshness, pins), so a trailing slash on the
default root is normalized rather than mistaken for a subtree. Exit 1 on an untracked
over-rejection, a stale ledger entry, a pin mismatch, a missing or partial checkout,
or an empty scan; nothing green-skips.

The JSON carries every bucket by name (`accept_parity`, `reject_parity`,
`over_acceptance.{parser,checker,codes}`, `script_goal`, `tsc_parser_rejects`,
`sanctioned`, `gap_known`, `gap_unexpected`, `beyond_acorn.{sanctioned,known,unexpected}`
with their `*_by_category` counts), the `multi_file` counters, and every skip count. A
unit entry carries `path` (the test) and `unit` (the virtual file); a single-file entry
has no `unit`.

## The reject-vs-defer line

The verdict rule every ledger entry names. The discriminator is the spec's own
layering, not a tsv-invented label:

- A rule that lives in a **production** — including its grammar parameters
  (`[Await]`, `[Yield]`), its arities (`get x()` / `set x(v)` are productions), and its
  `[no LineTerminator here]` gates — is the **parser's**; violating it is a parse error.
- A rule under **Static Semantics: Early Errors** is the **diagnostics layer's**: tsv
  parses it and defers.
- The early-error phrasing *"It is a Syntax Error if any source text is matched by this
  production"* is grammar in disguise — the production exists only as an error hook, so
  it rejects.
- **TypeScript's analog:** what `parser.ts` diagnoses is grammar; what tsc raises from
  the binder or checker, `checkGrammar*` included, is *normally* the diagnostics layer's.
  Normally, not always: where tsc's parser is deliberately looser than the published
  grammar for recovery, the checker-raised code is not evidence that the rule is an
  early error (the `tsc_recovery` category). The tiebreaker is the simpler uniform
  rule that matches prettier on the forms real code holds.

## Triage

The burn-down loop, one family per pull request, fixtures first.

1. **Run** `deno task conformance:ts-repo:run -v --json 2>summary.txt >report.json`.
   A red gate names each untracked over-rejection with tsv's error; the JSON carries
   every bucket.
2. **Group** the untracked entries by tsv's error message. One message is usually one
   family; the ledgers are written per family.
3. **Probe** the family against the four oracles: tsc's parser and tsc's baseline (both
   in the report), acorn (the sub-label), and prettier's `typescript` and `babel-ts`
   parsers. A construct all four accept is a gap. A construct only tsc's recovering
   parser accepts is a `tsc_recovery` or `grammar` sanction. A construct tsc's parser
   accepts and checker refuses, that prettier formats, is a flip candidate.
4. **Grade** it against the line above and pick the ledger: sanction (keep) or known
   gap (fix), with the category and a one-line reason that names the production or
   the code. The reason is the record — the gate reads it back on every run.
5. **Fix** a known gap fixture-first ([fixture_workflow.md](./fixture_workflow.md)):
   a `_svelte_divergence` fixture where acorn rejects and tsv should parse, an
   `input_invalid_*` file where tsv should keep rejecting. Delete the ledger entry in
   the same change; the freshness check refuses a stale one.
6. **Re-pin** the counts that moved, each with its reason on the constant.

## What this gate does not do

- **No AST comparison.** tsc's tree is never read; the shape oracle is acorn-typescript
  (`conformance:ts-fixtures` over acorn's own suite, `corpus:compare:parse` over the
  real-code corpus and the prettier suites). The tsc corpus never enters the `gates`
  corpus view, so there is no AST diff and no prettier format comparison over it.
- **No verdict on `.tsx`**, on `.js` units, or on a multi-file test with a grammar
  error somewhere in it.
- **No coverage number.** Coverage counts accepts and so can only reward
  permissiveness; the bench's conformance surface measures it over the harvest's
  tsc-valid set, and `ts-repo:over-acceptance` profiles the opposite axis. This gate
  grades tsv alone, per file, against tsc.
