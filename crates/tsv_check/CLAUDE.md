# tsv_check

> TypeScript binder + checker targeting exact TS7/tsgo error conformance —
> early scaffolding: the pipeline skeleton is real (parse → lower+bind →
> check → sort/dedup), the semantic phases are landing family by family.

## Position & invariants

- **Zero cost to shipped artifacts.** No format/parse artifact links this
  crate — `tsv_cli`/`tsv_ffi`/`tsv_wasm`/`tsv_napi` never reference it; the
  only consumer is `tsv_debug` (the conformance harness). Verify with
  `cargo tree -i tsv_check`. The existing parser and formatter are never
  modified in service of checking.
- **Faithful semantics, novel engine.** Observable behavior (which
  diagnostics exist, their codes/spans/order, budget limits, circularity
  outcomes) is ported from tsgo — the reference checkout is a pinned
  `../typescript-go` — while representation (dense u32 ids, SoA side
  columns, arena borrowing) is tsv's own.
- **Reference anchors.** Semantic-core functions carry a
  `// tsgo: <file.go> <function>` comment tying them to their counterpart
  in the pinned checkout. A deliberate structural departure is documented
  at the departure site; drift from the reference is always intentional,
  never incidental.
- **The oracle is tsgo's committed `.errors.txt` baselines** over the tsc
  test corpus, graded by `tsv_debug tsc_conformance` (see the root
  CLAUDE.md §tsgo Typechecker-Conformance Harness).
- `unsafe_code = "forbid"` (workspace lints inherited).

## Module map

- `lib.rs` — public API surface.
- `program.rs` — pipeline assembly, ported from tsgo
  `program.go GetDiagnosticsOfAnyProgram`: per-unit parse with the goal
  rule (`Goal::Module` first, `Goal::Script` retry on failure, both-fail =
  program parse-rejection), the **parse-error short-circuit** (any unit
  rejects ⇒ zero bind/check diagnostics program-wide), per-file
  bind-then-check concatenation, final sort+dedup.
- `merge.rs` — the single-threaded globals merge between bind and check,
  ported from tsgo `initializeChecker`'s phase order (script locals +
  globalThis check → global augmentations → undefined check → deferred
  ambient modules → module augmentations), with `mergeSymbol` /
  `reportMergeSymbolError` (the same three-way conflict selection as the
  binder cascade), `getExcludedSymbolFlags`, and the `lookupOrIssueError`
  dedup. Operates on scripts' file locals and `declare global` /
  ambient-module augmentation exports — never an external module's locals.
  Per-file `FileMerge` inputs are program-independent (owned strings,
  declaration-order iteration — never map order). Also owns the lib layer:
  `LibFile` (a lib's bound product), `LibBase::build` (fold a resolved lib
  set in priority order into an immutable global base, `globalThis`
  seeded), and the base-aware merge (`merge_symbol_against_base` — programs
  consult overlay-then-base; test symbols never mutate the base; base decls
  translate into program FileId space only on a conflict).
- `binder/` — **two cooperating walks per file** (deliberately NOT one
  fused walk — functions-first symbol binding reorders symbol creation
  within a statement list, which would break strict pre-order id
  intervals; see `binder/mod.rs`'s header):
  - `mod.rs` — the SoA lowering walk: dense 1-based `NodeId`s, side columns
    (`parents`/`kinds`/`spans`/`subtree_end` — the latter makes descendant
    tests O(1) interval checks), the address→NodeId map, per-file facts
    (module-ness via the `isAnExternalModuleIndicatorNode` port).
  - `sym.rs` — the container-threaded, functions-first symbol-bind walk:
    `getContainerFlags`, `declareSymbolEx` + the duplicate/conflict cascade
    (TS2451/2300/2567/2528 with per-prior-declaration related info),
    internal-name mangling (incl. private `#` names), the dual local/export
    collapse (documented at the site; revisited at multi-file).
  - `symbols.rs` — `Symbol`, `SymbolFlags` + the `*Excludes` conflict-mask
    const tables (ported bit-for-bit from tsgo's `symbolflags.go`), pooled
    declaration lists, `TableId` symbol tables.
  - `atoms.rs` — the checker's own program-scoped name interner (a fresh
    `string-interner` instance — never the parser's per-document
    `SharedInterner`), reserved internal-name atoms.

  **Borrow-only discipline**: visitors take
  `&'arena` references and never clone AST nodes — the AST derives `Clone`,
  and one accidental `.clone()` silently mints differently-addressed copies
  that break the address map; nothing type-level enforces this, so it is a
  reviewed convention.
- `check/` — the post-bind **syntactic** check pass (`check_file_members`), a
  standalone `CheckWalk` over `&Program` that never consults the binder's
  symbol tables (walking the shared interface member table would break
  declaration-merging). It descends every syntactic position a type literal or
  type-parameter list can hide — class / interface / type-literal bodies, every
  type-annotation / assertion / predicate / function-type site (a general
  `TSType` recursion), class/interface heritage type arguments, decorators
  (class / member / parameter), and template-literal-type interpolations — and
  runs the per-node check-time checks: `duplicate_members.rs` ports
  `checkObjectTypeForDuplicateDeclarations` (the two-map property/accessor
  state machine → TS2300, disjoint from the bind cascade by construction) and
  `checkTypeParameters` (per-declaration duplicate type-param identity). Its
  output folds into each file's diagnostics in `program.rs` before the
  program-wide sort/dedup. The traversal's `visit_type_params` is the seam
  future per-node checks hook into.
- `diag.rs` — `Diagnostic` (code, file, span, category, message + args,
  nested chain + related-info) and the canonical ordering kernels, ported
  from tsgo `internal/ast/diagnostic.go`: `compare_diagnostics`
  (path → start → end → code → args → chain size → chain content →
  related-info), `equal_no_related_info` (full-chain equality, related-info
  excluded), `sort_and_deduplicate` (+ related-info merge). Pure kernels,
  unit-tested per comparator leg.
- `ids.rs` — `NodeId` (`NonZeroU32`, 1-based pre-order; `Option<NodeId>`
  niche-packs to 4 bytes) and `FileId` newtypes.
- `hash.rs` — crate-private Fx-style multiply-xor hasher +
  `FxHashMap`/`FxHashSet` aliases (no external hashing dependency).

## Public API

```rust
let arena = bumpalo::Bump::new();
let units = [SourceUnit::new("a.ts", source)];
let result: CheckResult = check_program(&units, &arena);
// result.diagnostics — canonically sorted + deduplicated
// result.files[i].parse — ParseReport::Parsed(ParsedFacts) | Rejected
// result.parse_rejected — the short-circuit fired
```

The caller owns the arena (the same contract as `tsv_ts::parse`); the
result is fully owned — nothing borrows out. For lib-aware checking:
`bind_program` (parse+bind once, variant-independent, fully owned) →
`check_bound(&bound, Some(&lib_base))`; `bind_lib` produces a cacheable
`LibFile`; `check_program_with_lib` is the one-shot form.

## Which tool answers which question

- `tsv_debug tsc_conformance run` — the standing gate: sweeps the in-scope
  corpus (single-file, non-JSX, non-JS-flavored, non-skipped), grades
  expect-clean variants AND the duplicate-conflict family
  (TS2300/2451/2567/2528 + merge-path TS2397/2649/2664/2671) as codes+spans
  multisets — bind + merge + lib **and** the check-time TS2300 subset
  (duplicate members / type parameters, from the `check` pass). extra = 0 is a
  hard gate; a missing is classified `merge` / `lib` / `deferred_late_bound`
  (an exact pin — the type-engine-dependent `lateBindMember` residual) /
  `other` (a HARD-zero invariant — any unclassified family miss fails the run).
  It also grades related-info on matched primaries as its own pinned channel
  and publishes the parse-divergence census; exact `RUN_*` pins.
  Triage filters (`--test`/`--code`/`--variant`) skip the pins;
  `--emit-manifest` and `--report` (the committed
  `benches/js/results/report.tsc-conformance.{json,md}`) serve tooling. A
  release-gating leg of `deno task conformance` (`conformance:tsc-check`).
- `tsv_debug profile --bind <paths>` — parse vs lower+bind timing + peak
  RSS (VmHWM); the binder's standing perf anchor form.
- `tsv_debug tsc_conformance check-test <name> [--variant k=v] [--json]` —
  the inner dev loop: one test, our diagnostics vs the baseline summary.
- `tsv_debug tsc_conformance query|roundtrip|index` — the oracle-side
  surfaces (baseline aggregations; parser/renderer byte-identity; corpus
  index self-checks).
