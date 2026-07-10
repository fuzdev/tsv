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
- `binder/` — the fused lower+bind pre-order walk: assigns dense 1-based
  `NodeId`s, fills SoA side columns (`parents`/`kinds`/`spans`/
  `subtree_end` — the latter makes descendant tests O(1) interval checks),
  builds the address→NodeId map, derives per-file facts (module-ness from
  import/export presence). **Borrow-only discipline**: visitors take
  `&'arena` references and never clone AST nodes — the AST derives `Clone`,
  and one accidental `.clone()` silently mints differently-addressed copies
  that break the address map; nothing type-level enforces this, so it is a
  reviewed convention.
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
result is fully owned — nothing borrows out.

## Which tool answers which question

- `tsv_debug tsc_conformance run` — the standing gate: sweeps the in-scope
  corpus (single-file, non-JSX, non-JS-flavored, non-skipped), grades
  expect-clean variants AND the bind/merge duplicate-conflict family
  (TS2300/2451/2567/2528 + merge-path codes) as codes+spans multisets
  (extra = 0 is a hard gate; missing is classified by deferred cause),
  publishes the parse-divergence census; exact `RUN_*` pins.
- `tsv_debug profile --bind <paths>` — parse vs lower+bind timing + peak
  RSS (VmHWM); the binder's standing perf anchor form.
- `tsv_debug tsc_conformance check-test <name> [--variant k=v] [--json]` —
  the inner dev loop: one test, our diagnostics vs the baseline summary.
- `tsv_debug tsc_conformance query|roundtrip|index` — the oracle-side
  surfaces (baseline aggregations; parser/renderer byte-identity; corpus
  index self-checks).
