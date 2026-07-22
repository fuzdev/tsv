use super::*;

/// The merge-path family codes — a *missing* of one of these is classified as a
/// merge-phase gap, not a same-table cascade bug.
const MERGE_CODES: [u32; 4] = [2397, 2649, 2664, 2671];

/// The TS1xxx codes the binder itself emits (strict-mode + private-identifier
/// checks) — they prove nothing about parse state, so a baseline carrying only
/// these does not trigger the recovery-AST carve-out (predicate v1, rule a).
const BIND_EMITTED_TS1XXX: [u32; 12] = [
    1100, 1101, 1102, 1210, 1212, 1213, 1214, 1215, 1262, 1344, 1359, 18012,
];

/// The family baselines whose family diagnostics come from a standard-library
/// conflict. These **match** via the lib base; the classifier is kept as a
/// regression guard — a *missing* in one of these is bucketed to
/// [`MissingCause::Lib`] (pinned 0) rather than the hard-zero
/// [`MissingCause::Other`], so a lib-detection regression fails loudly.
const LIB_CONFLICT_BASELINES: [&str; 5] = [
    "intersectionsOfLargeUnions2.ts",
    "jsExportMemberMergedWithModuleAugmentation2.ts",
    "promiseDefinitionTest.ts",
    "recursiveComplicatedClasses.ts",
    "variableDeclarationInStrictMode1.ts",
];

/// The family baselines whose remaining missing family diagnostics are genuinely
/// deferred: they need the type engine (a literal-type or `unique symbol` computed
/// member name, resolved via tsgo's `lateBindMember`) that this bind+check
/// implementation has no counterpart for. A *missing* in one of these is bucketed to
/// [`MissingCause::DeferredLateBound`] (exact-pinned) rather than the hard-zero
/// [`MissingCause::Other`], so the honest residual stays visible without gating a
/// release on a type-engine gap. Basenames, mirroring `LIB_CONFLICT_BASELINES`.
const LATE_BOUND_BASELINES: [&str; 4] = [
    "dynamicNamesErrors.ts",
    "symbolDeclarationEmit12.ts",
    "symbolProperty37.ts",
    "symbolProperty44.ts",
];

/// The flow-family baselines whose remaining missing TS7027 diagnostics are
/// genuinely deferred to the CFA type engine: the construction-only fast-path
/// shim (a strict subset of what tsgo reports) can't resolve them because they
/// come from tsgo's `isReachableFlowNode` fallback — never-returning call
/// signatures, `asserts` type predicates, switch exhaustiveness, and the
/// structural reachability fallback. A *missing* in one of these is bucketed to
/// [`MissingCause::DeferredCfa`] (exact-pinned) rather than the hard-zero
/// [`MissingCause::Other`], so the honest CFA residual stays visible without gating
/// a release on a type-engine gap. Basenames, mirroring `LATE_BOUND_BASELINES`.
///
/// `assertionTypePredicates1.ts` never reaches the cfa bucket — its entry is a
/// defensive no-op kept for completeness. tsv currently **parse-rejects** it (an
/// over-rejection of the setter assertion-predicate form `set p(x: this is T)`,
/// counted in the parse-divergence census), so it is graded not at all; and were
/// the parser fixed, its baseline's non-bind TS1xxx (TS1228) would carve it out by
/// predicate v1 rule (a) instead — either way its 5 flow instances never land in
/// the cfa bucket. The live cfa residual is **26**: neverReturningFunctions1
/// 22, exhaustiveSwitchStatements1 1, unreachableSwitchTypeofAny 1,
/// unreachableSwitchTypeofUnknown 1 (**25** across the four non-carved named
/// baselines), plus reachabilityChecks8 1 (the structural `isReachableFlowNode`
/// fallback tsv's faithful for-construction correctly doesn't emit).
const DEFERRED_CFA_BASELINES: [&str; 6] = [
    "neverReturningFunctions1.ts",
    "assertionTypePredicates1.ts",
    "exhaustiveSwitchStatements1.ts",
    "unreachableSwitchTypeofAny.ts",
    "unreachableSwitchTypeofUnknown.ts",
    "reachabilityChecks8.ts",
];

/// Classify one missing family diagnostic by its baseline + code. Each
/// name-keyed cause also requires the code to be in its family (dup for
/// lib/late-bound, flow for cfa), mirroring the merge branch — a wrong-family
/// missing inside a named baseline falls through to the hard-zero `Other`
/// instead of being silently absorbed. This keeps the classification honest on
/// filtered/triage runs, not only on a full run where the code-keyed pins
/// catch it.
fn classify_missing(basename: &str, code: u32) -> MissingCause {
    if MERGE_CODES.contains(&code) {
        MissingCause::Merge
    } else if LIB_CONFLICT_BASELINES.contains(&basename) && DUP_CODES.contains(&code) {
        MissingCause::Lib
    } else if LATE_BOUND_BASELINES.contains(&basename) && DUP_CODES.contains(&code) {
        MissingCause::DeferredLateBound
    } else if DEFERRED_CFA_BASELINES.contains(&basename) && FLOW_CODES.contains(&code) {
        MissingCause::DeferredCfa
    } else {
        MissingCause::Other
    }
}

/// How a crash-excluded test fails, and whether its liveness is probeable.
// `GenuineAbort` is the designed flag for a future stack-overflow entry (none on
// the pinned corpus); it is un-probeable, so it is never re-run.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum CrashKind {
    /// A debug-build `debug_assert!` panic `catch_unwind` contains — probeable:
    /// the sweep re-runs it under `catch_unwind` and FAILS if it no longer
    /// panics (a fix landed, so the entry is stale and must be dropped).
    CatchablePanic,
    /// An uncatchable stack-overflow *abort* even on [`SKELETON_STACK`] — not
    /// probeable (probing would abort the whole run), so it is trusted, not tested.
    GenuineAbort,
}

/// Tests that crash the tsv parser — carved out by basename, counted, and
/// reported (never silently). Each entry names its cause + kind; the list is a
/// tracked-defect ledger, not a way to hide bugs. A [`CrashKind::CatchablePanic`]
/// entry is liveness-probed every run (see [`probe_crash_exclusion`]).
///
/// Currently empty — no tracked parser crasher in the in-scope corpus. (The
/// former `exportDeclarationInInternalModule.ts` entry — `export * from
/// <identifier>;` tripping a `debug_assert!` in `parse_string_literal` — no
/// longer panics.)
const CRASH_EXCLUSIONS: &[(&str, CrashKind)] = &[];

/// The [`CrashKind`] of a crash-excluded test, or `None` if not excluded.
fn crash_exclusion_kind(basename: &str) -> Option<CrashKind> {
    CRASH_EXCLUSIONS
        .iter()
        .find(|(n, _)| *n == basename)
        .map(|(_, k)| *k)
}

/// The baseline shape used to bucket a parse-rejected variant.
enum BaselineShape {
    None,
    Ts1xxxOnly,
    Other,
}

pub(super) fn run_skeleton_inner(
    checkout: &Path,
    options: &RunOptions,
) -> Result<SkeletonReport, String> {
    let start = Instant::now();
    let corpus = discover_corpus(checkout)?;
    let baselines = discover_baselines(&baselines_dir(checkout))?;

    // Baseline lookup keyed by (suite, config-name) — exactly the runner's join.
    let mut ondisk: HashMap<(&str, String), &Baseline> = HashMap::new();
    for baseline in &baselines {
        if let Some((suite, name)) = baseline.relative_path.split_once('/') {
            ondisk.insert((suite, name.to_string()), baseline);
        }
    }

    let mut report = SkeletonReport::default();
    let mut resolver = LibResolver::new(checkout);
    let watchdog = TestWatchdog::spawn();

    for test in &corpus {
        // Test-level triage filter (`--test <substr>`): match the roundtrip identity.
        if !options.filter.keeps_test(&test.relative_path) {
            continue;
        }
        watchdog.enter(&test.relative_path);
        if SKIPPED_TESTS.contains(&test.basename.as_str()) {
            continue;
        }
        if let Some(kind) = crash_exclusion_kind(&test.basename) {
            report.excluded_crashes += 1;
            // Liveness probe: a catchable-panic entry must still panic; if it no
            // longer does, the ledger entry is stale and the run fails.
            if kind == CrashKind::CatchablePanic && !probe_crash_exclusion(test) {
                report.stale_exclusions.push(test.basename.clone());
            }
            continue;
        }

        let content = read_corpus_file(&test.path)?;
        let settings = extract_settings(&content);
        let units = split_units(&content, &test.basename);

        // Test-level in-scope filter: single-file (one non-config unit), not
        // JSX-scoped, not JS-flavored.
        if units.len() != 1 || is_config_file_name(&units[0].name) {
            continue;
        }
        if is_jsx_scoped(test, &settings) || is_js_flavored(test, &settings) {
            continue;
        }

        let expansion = expand(&settings);
        if expansion.cap_exceeded {
            continue;
        }
        let in_scope: Vec<&Variant> = expansion
            .variants
            .iter()
            .filter(|v| !variant_is_unsupported(&v.config))
            .collect();
        if in_scope.is_empty() {
            continue;
        }

        report.in_scope_tests += 1;
        grade_test(
            test,
            &units[0],
            &in_scope,
            &ondisk,
            &mut resolver,
            options,
            &mut report,
        );
    }
    watchdog.finish();

    // Fold in the resolver's lib-base census (parse-once/fold-once counts + gates).
    report.lib_files_bound = resolver.files_bound();
    report.lib_sets_built = resolver.sets_built();
    report.lib_parse_errors = {
        let mut errors: Vec<String> = resolver
            .parse_errors()
            .iter()
            .map(|(f, e)| format!("{f}: {e}"))
            .collect();
        errors.sort_unstable();
        errors
    };
    report.lib_missing_files = {
        let mut files: Vec<String> = resolver.missing_files().to_vec();
        files.sort_unstable();
        files
    };
    report.lib_unknown_names = {
        let mut names: Vec<String> = resolver.unknown_libs().to_vec();
        names.sort_unstable();
        names.dedup();
        names
    };
    report.lib_external_no_globals = resolver.external_no_globals();

    report.wall_ms = start.elapsed().as_millis();
    Ok(report)
}

/// Re-run a catchable-panic crash exclusion under `catch_unwind`, returning
/// whether it **still panics**. A `false` (it completed) means the tracked defect
/// is fixed and the ledger entry is stale.
fn probe_crash_exclusion(test: &CorpusTest) -> bool {
    let Ok(content) = read_corpus_file(&test.path) else {
        // Can't read it -> can't disprove the panic; treat as still-live.
        return true;
    };
    let units = split_units(&content, &test.basename);
    let arena = Bump::new();
    let source_units: Vec<SourceUnit<'_>> = units
        .iter()
        .map(|u| SourceUnit::new(&u.name, &u.content))
        .collect();
    // Silence the default panic hook for the deliberate probe (we expect it to
    // panic; the message would otherwise leak to stderr and read as a failure).
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        let _ = check_program(&source_units, &arena, &CheckOptions::default());
    }))
    .is_err();
    std::panic::set_hook(prev);
    panicked
}

/// Extract a caught panic payload's message (the `&str` / `String` cases the
/// standard panic machinery produces).
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(non-string panic payload)".to_string()
    }
}

/// Parse+bind one single-file test once, then — per in-scope variant — merge it
/// against that variant's resolved lib base and grade the result. Parse+bind is
/// variant-independent; only the merge (and thus the lib-conflict family) varies by
/// the resolved lib set, so a variant with a Promise/Symbol/… global conflicts at
/// one target and is clean at another.
/// Build `tsv_check`'s options from a variant's resolved directive config, mapping
/// the harness tri-state to the checker's. `preserveConstEnums` feeds
/// `ShouldPreserveConstEnums` (the `isolatedModules` contribution is not modeled —
/// a rare-in-dead-code residual that can only under-report).
pub(super) fn check_options_for(config: &BTreeMap<String, String>) -> CheckOptions {
    use crate::tsc_conformance::options_meta::{Tristate as OptTri, resolve_bool};
    let map = |t: OptTri| match t {
        OptTri::Unset => tsv_check::Tristate::Unknown,
        OptTri::False => tsv_check::Tristate::False,
        OptTri::True => tsv_check::Tristate::True,
    };
    CheckOptions {
        allow_unreachable_code: map(resolve_bool(config, "allowunreachablecode")),
        allow_unused_labels: map(resolve_bool(config, "allowunusedlabels")),
        preserve_const_enums: resolve_bool(config, "preserveconstenums") == OptTri::True,
    }
}

fn grade_test(
    test: &CorpusTest,
    unit: &Unit,
    in_scope: &[&Variant],
    ondisk: &HashMap<(&str, String), &Baseline>,
    resolver: &mut LibResolver,
    options: &RunOptions,
    report: &mut SkeletonReport,
) {
    // Parse + bind on a fresh arena, contained against panics (the tsv parser is the
    // panic source; the merge over owned data that follows is deterministic).
    let arena = Bump::new();
    let bound = match catch_unwind(AssertUnwindSafe(|| {
        bind_program(&[SourceUnit::new(&unit.name, &unit.content)], &arena)
    })) {
        Ok(bound) => bound,
        Err(payload) => {
            report.panics.push(PanicRecord {
                test: test.relative_path.clone(),
                payload: panic_payload_message(&*payload),
            });
            return;
        }
    };

    // The single unit's parse outcome (parse_reports is never empty for one input).
    let reports = bound.parse_reports();
    let Some(&(_, parse)) = reports.first() else {
        return;
    };
    let parsed = matches!(parse, ParseReport::Parsed(_));

    // The unit's line map — reused across the test's variants for the parsed case.
    let line_map = parsed.then(|| LocationTracker::new_ecmascript_with_map(&unit.content));

    for variant in in_scope {
        let name = config_name(&test.basename, &variant.description);
        let baseline = ondisk.get(&(test.suite, name.clone())).copied();

        // Variant-level triage filters, applied BEFORE counting so a filtered sweep's
        // denominators reflect only the graded slice (any active filter skips the pins).
        if !options.filter.keeps_variant(&variant.config) {
            continue;
        }
        if !options
            .filter
            .keeps_code(|code| baseline.is_some_and(|b| baseline_carries_code(b, code)))
        {
            continue;
        }
        if !options
            .filter
            .keeps_family(|code| baseline.is_some_and(|b| baseline_carries_code(b, code)))
        {
            continue;
        }

        report.in_scope_variants += 1;
        if variant.config.contains_key("moduledetection") {
            report.module_detection_variants += 1;
        }

        let verdict: &'static str = match parse {
            ParseReport::Rejected { .. } => {
                report.parse_rejected_total += 1;
                match baseline_shape(baseline) {
                    BaselineShape::None => report.parse_rejected_no_baseline += 1,
                    BaselineShape::Ts1xxxOnly => report.parse_rejected_ts1xxx_only += 1,
                    BaselineShape::Other => report.parse_rejected_other += 1,
                }
                "parse_rejected"
            }
            ParseReport::Parsed(facts) => {
                if facts.used_script_retry {
                    report.script_retry += 1;
                }
                // Resolve this variant's lib set (cached) and merge the bound program
                // against it — the merge diagnostics are the lib-conflict family. The
                // lib resolution (parse+bind of each `.d.ts`) is the only remaining
                // panic source past the initial bind, so contain it per variant: a
                // future lib parse panic is recorded, not sweep-fatal.
                let check_opts = check_options_for(&variant.config);
                let checked = catch_unwind(AssertUnwindSafe(|| {
                    let base = resolver.base_for(&variant.config);
                    let result = check_bound(&bound, base.as_deref(), &check_opts);
                    (base, result)
                }));
                let (base, result) = match checked {
                    Ok(pair) => pair,
                    Err(payload) => {
                        report.panics.push(PanicRecord {
                            test: test.relative_path.clone(),
                            payload: panic_payload_message(&*payload),
                        });
                        report.failing_variants.push(FailingVariant {
                            suite: test.suite.to_string(),
                            config: name.clone(),
                            reason: "panic",
                            diff: format!("# {}/{name}  (lib-resolution panic)\n", test.suite),
                        });
                        record_manifest(
                            report,
                            options,
                            test,
                            &name,
                            baseline.is_some(),
                            true,
                            "panic",
                        );
                        continue;
                    }
                };
                let lib_files = base.as_ref().map_or(&[][..], |b| b.lib_files.as_slice());

                match baseline {
                    None => {
                        report.expect_clean_graded += 1;
                        if result.diagnostics.is_empty() {
                            report.clean_pass += 1;
                            "clean_pass"
                        } else {
                            report.clean_fail.push(CleanFail {
                                variant: format!("{}/{name}", test.suite),
                                diagnostics: result.diagnostics.len(),
                            });
                            report.failing_variants.push(FailingVariant {
                                suite: test.suite.to_string(),
                                config: name.clone(),
                                reason: "clean_fail",
                                diff: render_clean_fail_diff(test, &name, &result.diagnostics),
                            });
                            "clean_fail"
                        }
                    }
                    Some(b) => {
                        report.baselined_parsed += 1;
                        // `parsed` => `line_map` is `Some`; the `None` arm is dead.
                        let ours_family = match line_map.as_ref() {
                            Some((tracker, map)) => {
                                let mapper = LocationMapper { tracker, map };
                                build_ours_family(
                                    &result.diagnostics,
                                    &unit.name,
                                    &mapper,
                                    lib_files,
                                )
                            }
                            None => Vec::new(),
                        };
                        grade_family(test, &name, b, &ours_family, report)
                    }
                }
            }
        };

        record_manifest(
            report,
            options,
            test,
            &name,
            baseline.is_some(),
            parsed,
            verdict,
        );
    }

    // Node total: counted once per test (all variants share the parse+bind).
    report.total_nodes += bound.total_node_count();
}

/// Record one per-variant verdict row for `--emit-manifest` (a no-op unless a
/// manifest is being collected).
fn record_manifest(
    report: &mut SkeletonReport,
    options: &RunOptions,
    test: &CorpusTest,
    config: &str,
    baselined: bool,
    parsed: bool,
    verdict: &'static str,
) {
    if options.collect_manifest {
        report.manifest_entries.push(ManifestEntry {
            suite: test.suite.to_string(),
            test: test.relative_path.clone(),
            config: config.to_string(),
            baselined,
            parsed,
            verdict,
        });
    }
}

/// Whether a baseline carries a given TS code (the `--code` / `--family` filters).
/// Uses the category-generic [`parse_base_diags`] (not the error-only
/// `parse_summary_block`), so a `--code 7027` / `--family flow` triage matches
/// suggestion- and message-category flow lines too, not just error-category ones.
fn baseline_carries_code(baseline: &Baseline, code: u32) -> bool {
    let Ok(content) = std::fs::read_to_string(&baseline.path) else {
        return false;
    };
    let want = i32::try_from(code).unwrap_or(-1);
    parse_base_diags(&content).iter().any(|d| d.code == want)
}

/// Render one failing family variant's ours-vs-baseline diff for a `.diff` artifact.
fn render_family_diff(
    test: &CorpusTest,
    name: &str,
    reason: &str,
    ours: &[FamilyEntry],
    base: &[FamilyEntry],
) -> String {
    use std::fmt::Write as _;
    let mut s = format!("# {}/{name}  ({reason})\n", test.suite);
    let _ = writeln!(s, "## ours family ({})", ours.len());
    for e in ours {
        let _ = writeln!(
            s,
            "  {}({},{}): TS{}",
            e.key.file, e.key.line, e.key.col, e.key.code
        );
    }
    let _ = writeln!(s, "## baseline family ({})", base.len());
    for e in base {
        let _ = writeln!(
            s,
            "  {}({},{}): TS{}",
            e.key.file, e.key.line, e.key.col, e.key.code
        );
    }
    s
}

/// Render an expect-clean variant's spurious diagnostics for a `.diff` artifact.
fn render_clean_fail_diff(test: &CorpusTest, name: &str, diags: &[Diagnostic]) -> String {
    use std::fmt::Write as _;
    let mut s = format!(
        "# {}/{name}  (clean_fail — expect-clean but {} diagnostic(s))\n## ours ({})\n",
        test.suite,
        diags.len(),
        diags.len(),
    );
    for d in diags {
        let _ = writeln!(s, "  TS{} @ [{}..{}]", d.code, d.span.start, d.span.end);
    }
    s
}

/// The number of program units in the single-file sweep (a lib FileId is
/// `>= UNITS_LEN`, translating to `lib_files[FileId - UNITS_LEN]`).
const UNITS_LEN: u32 = 1;

/// Build our family multiset for one variant's diagnostics, resolving each FileId to
/// a display name. A **lib-file primary** (FileId beyond the program units) is
/// dropped — the baseline masks it (`lib.x.d.ts(--,--)`) — and a lib-sourced related
/// carries the lib file name with a masked location so it matches the baseline's
/// `lib.x.d.ts:--:--` related by `(code, file)`.
fn build_ours_family(
    diagnostics: &[Diagnostic],
    unit_name: &str,
    mapper: &LocationMapper<'_>,
    lib_files: &[String],
) -> Vec<FamilyEntry> {
    diagnostics
        .iter()
        .filter(|d| FAMILY_CODES.contains(&d.code))
        .filter_map(|d| {
            let file = d.file?;
            // A lib-file primary is masked in the baseline — exclude it.
            if file.index() >= UNITS_LEN as usize {
                return None;
            }
            let (_, pos) = mapper.pos_and_position(d.span.start);
            let key = FamilyDiag {
                file: unit_name.to_string(),
                line: pos.line as u32,
                col: pos.column as u32 + 1,
                code: d.code,
            };
            let related = d
                .related
                .iter()
                .map(|r| resolve_related(r, unit_name, mapper, lib_files))
                .collect();
            Some(FamilyEntry { key, related })
        })
        .collect()
}

/// Resolve one related-info entry's FileId to a [`RelatedKey`]: an in-unit related
/// carries its computed location; a lib-sourced related carries the lib file name
/// and a masked (`None`) location.
fn resolve_related(
    r: &Diagnostic,
    unit_name: &str,
    mapper: &LocationMapper<'_>,
    lib_files: &[String],
) -> RelatedKey {
    match r.file {
        Some(f) if f.index() < UNITS_LEN as usize => {
            let (_, pos) = mapper.pos_and_position(r.span.start);
            RelatedKey {
                code: r.code,
                file: unit_name.to_string(),
                loc: Some((pos.line as u32, pos.column as u32 + 1)),
            }
        }
        Some(f) => {
            let idx = f.index() - UNITS_LEN as usize;
            RelatedKey {
                code: r.code,
                file: lib_files.get(idx).cloned().unwrap_or_default(),
                loc: None,
            }
        }
        None => RelatedKey {
            code: r.code,
            file: unit_name.to_string(),
            loc: None,
        },
    }
}

/// One family diagnostic in baseline coordinates: `(file, 1-based line, 1-based
/// UTF-16 col, code)`.
#[derive(Clone, PartialEq, Eq, Hash)]
struct FamilyDiag {
    file: String,
    line: u32,
    col: u32,
    code: u32,
}

/// One related-info entry in baseline coordinates: `(code, file, location)`. A
/// `--,--` (default-library) location is [`None`] and compares by code+file only.
#[derive(Clone, PartialEq, Eq, Hash)]
struct RelatedKey {
    code: u32,
    file: String,
    loc: Option<(u32, u32)>,
}

/// A family primary plus its related-info entries — the unit the related-info
/// channel grades (a matched primary's related sets are compared as multisets).
struct FamilyEntry {
    key: FamilyDiag,
    related: Vec<RelatedKey>,
}

/// Grade one parsed-with-baseline variant's family diagnostics against its
/// baseline, folding the buckets into `report` and returning the per-variant
/// verdict (for the `--emit-manifest` row). Applies predicate v1 rule (a)
/// (recovery-AST carve-out) first, then the primary-code channel and — for the
/// matched primaries — the independent related-info channel.
fn grade_family(
    test: &CorpusTest,
    name: &str,
    baseline: &Baseline,
    ours: &[FamilyEntry],
    report: &mut SkeletonReport,
) -> &'static str {
    let Ok(content) = std::fs::read_to_string(&baseline.path) else {
        return "baseline_unreadable";
    };
    let base_all = parse_base_diags(&content);

    // Predicate v1 rule (a): tsv parses clean (it did — this variant parsed) and
    // the baseline carries a non-bind TS1xxx code -> recovery-AST incomparable.
    let has_nonbind_ts1xxx = base_all.iter().any(|d| {
        (1000..2000).contains(&d.code)
            && u32::try_from(d.code).is_ok_and(|c| !BIND_EMITTED_TS1XXX.contains(&c))
    });
    let base_family: Vec<FamilyEntry> = base_all
        .iter()
        .filter_map(|d| {
            let code = u32::try_from(d.code).ok()?;
            if !FAMILY_CODES.contains(&code) {
                return None;
            }
            Some(FamilyEntry {
                key: FamilyDiag {
                    file: d.file.clone()?,
                    line: d.line?,
                    col: d.col?,
                    code,
                },
                related: d.related.clone(),
            })
        })
        .collect();
    let has_family = !base_family.is_empty();

    if has_nonbind_ts1xxx {
        report.carve_out_rule_a += 1;
        if has_family {
            report.carve_out_rule_a_family += 1;
        }
        return "carve_out";
    }

    report.family_graded_variants += 1;
    if has_family {
        report.family_positive_variants += 1;
    }

    let ours_keys: Vec<FamilyDiag> = ours.iter().map(|e| e.key.clone()).collect();
    let base_keys: Vec<FamilyDiag> = base_family.iter().map(|e| e.key.clone()).collect();
    let buckets = family_buckets(&ours_keys, &base_keys);
    report.family_match += buckets.matched;
    report.family_span_mismatch += buckets.span_mismatch;
    report.family_extra += buckets.extra;
    for (code, count) in &buckets.matched_by_code {
        *report.family_match_by_code.entry(*code).or_default() += *count;
    }
    if buckets.extra > 0 && report.extra_samples.len() < 20 {
        report
            .extra_samples
            .push(format!("{}/{name} (+{})", test.suite, buckets.extra));
    }
    if buckets.span_mismatch > 0 && report.span_mismatch_samples.len() < 20 {
        report.span_mismatch_samples.push(format!(
            "{}/{name} (~{})",
            test.suite, buckets.span_mismatch
        ));
    }
    // An unexplained hard-fail bucket (extra / span mismatch) gets a rendered
    // ours-vs-baseline diff artifact; the pinned deferred-late-bound `missing` is
    // expected, so it does not.
    if buckets.extra > 0 || buckets.span_mismatch > 0 {
        let reason = if buckets.extra > 0 {
            "family_extra"
        } else {
            "family_span_mismatch"
        };
        report.failing_variants.push(FailingVariant {
            suite: test.suite.to_string(),
            config: name.to_string(),
            reason,
            diff: render_family_diff(test, name, reason, ours, &base_family),
        });
    }
    for (code, count) in &buckets.missing_by_code {
        report.family_missing += *count;
        *report.family_missing_by_code.entry(*code).or_default() += *count;
        let cause = classify_missing(&test.basename, *code);
        *report.missing_by_cause.entry(cause).or_default() += *count;
        if cause == MissingCause::Other && report.missing_other_samples.len() < 20 {
            report
                .missing_other_samples
                .push(format!("{}/{name} TS{code} x{count}", test.suite));
        }
    }

    // The related-info channel (independent of the primary verdict): grade related
    // multisets only for the primaries that matched.
    let rel = grade_related(ours, &base_family);
    report.related_match += rel.matched;
    report.related_span_mismatch += rel.span_mismatch;
    report.related_extra += rel.extra;
    report.related_missing += rel.missing;
    if rel.extra > 0 && report.related_extra_samples.len() < 20 {
        report
            .related_extra_samples
            .push(format!("{}/{name} (+{})", test.suite, rel.extra));
    }
    if rel.missing > 0 && report.related_missing_samples.len() < 20 {
        report
            .related_missing_samples
            .push(format!("{}/{name} (-{})", test.suite, rel.missing));
    }

    // The per-variant verdict (extra dominates — it is the hard gate).
    if buckets.extra > 0 {
        "family_extra"
    } else if buckets.span_mismatch > 0 {
        "family_span_mismatch"
    } else if !buckets.missing_by_code.is_empty() {
        "family_missing"
    } else if has_family {
        "family_match"
    } else {
        "baselined_clean"
    }
}

/// A baseline summary diagnostic with its parsed related-info entries.
struct BaseDiag {
    file: Option<String>,
    line: Option<u32>,
    col: Option<u32>,
    /// The `TS<code>` (i32 — the harness's `TS-1` and non-family codes appear here).
    code: i32,
    related: Vec<RelatedKey>,
}

/// Parse a baseline into summary diagnostics with related info, via the full
/// [`parse_baseline`] model (100% of the pinned baselines round-trip through it).
/// Falls back to the related-free summary parse on the rare structural surprise,
/// so the primary channel never shifts.
fn parse_base_diags(content: &str) -> Vec<BaseDiag> {
    use crate::tsc_conformance::baseline::Loc;
    match parse_baseline(content) {
        Ok(parsed) => parsed
            .diags
            .iter()
            .map(|d| {
                let (line, col) = match d.loc {
                    Some(Loc::Numbered { line, col }) => (Some(line), Some(col)),
                    _ => (None, None),
                };
                let related = d
                    .related
                    .iter()
                    .filter_map(|s| parse_related_line(s))
                    .collect();
                BaseDiag {
                    file: d.file.clone(),
                    line,
                    col,
                    code: d.code,
                    related,
                }
            })
            .collect(),
        Err(_) => parse_summary_block(content)
            .into_iter()
            .map(|d| BaseDiag {
                file: d.file,
                line: d.line,
                col: d.col,
                code: d.code as i32,
                related: Vec::new(),
            })
            .collect(),
    }
}

/// Parse one `!!! related TS<code> <file>:<line>:<col>: <msg>` line into a
/// [`RelatedKey`], or `None` for a chain-continuation line (no `!!! related`
/// prefix). A `--:--` location parses to [`None`] (a masked default-lib position).
fn parse_related_line(line: &str) -> Option<RelatedKey> {
    let rest = line.strip_prefix("!!! related TS")?;
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    let code: u32 = rest.get(..end)?.parse().ok()?;
    let after = rest.get(end..)?.strip_prefix(' ')?; // `<file>:<line>:<col>: <msg>`
    // The first `": "` separates the location from the message (a filename holds
    // no space, and line/col are digits-or-`--`).
    let boundary = after.find(": ")?;
    let locpart = after.get(..boundary)?; // `<file>:<line>:<col>`
    let (rest2, col) = locpart.rsplit_once(':')?;
    let (file, line_s) = rest2.rsplit_once(':')?;
    let loc = if line_s == "--" && col == "--" {
        None
    } else {
        Some((line_s.parse().ok()?, col.parse().ok()?))
    };
    Some(RelatedKey {
        code,
        file: file.to_string(),
        loc,
    })
}

/// The related-info buckets across a variant's matched primaries.
#[derive(Default)]
struct RelatedBuckets {
    matched: usize,
    extra: usize,
    span_mismatch: usize,
    missing: usize,
}

/// Grade related-info multisets for the primaries that match by
/// `(file,line,col,code)`. Ours and the baseline are grouped by primary key;
/// matched primaries are paired positionally and their related sets diffed
/// (exact `(code,file,loc)` match, masked `--,--` by `(code,file)`, then
/// `(code,file)` span-mismatch pairing of the leftovers).
fn grade_related(ours: &[FamilyEntry], base: &[FamilyEntry]) -> RelatedBuckets {
    let mut ours_by: HashMap<&FamilyDiag, Vec<&[RelatedKey]>> = HashMap::new();
    for e in ours {
        ours_by.entry(&e.key).or_default().push(&e.related);
    }
    let mut base_by: HashMap<&FamilyDiag, Vec<&[RelatedKey]>> = HashMap::new();
    for e in base {
        base_by.entry(&e.key).or_default().push(&e.related);
    }

    let mut out = RelatedBuckets::default();
    for (key, ours_sets) in &ours_by {
        let Some(base_sets) = base_by.get(key) else {
            continue;
        };
        let paired = ours_sets.len().min(base_sets.len());
        for i in 0..paired {
            related_diff(ours_sets[i], base_sets[i], &mut out);
        }
    }
    out
}

/// Diff one matched primary's related multisets, folding into `out`.
fn related_diff(ours: &[RelatedKey], base: &[RelatedKey], out: &mut RelatedBuckets) {
    // Exact `(code,file,loc)` matches first.
    let mut ours_counts: HashMap<&RelatedKey, usize> = HashMap::new();
    for r in ours {
        *ours_counts.entry(r).or_default() += 1;
    }
    let mut base_counts: HashMap<&RelatedKey, usize> = HashMap::new();
    for r in base {
        *base_counts.entry(r).or_default() += 1;
    }
    // Leftovers grouped by `(code, file)` for masked-match and span-mismatch pairing.
    let mut left_ours: HashMap<(u32, &str), usize> = HashMap::new();
    let mut left_base_located: HashMap<(u32, &str), usize> = HashMap::new();
    let mut left_base_masked: HashMap<(u32, &str), usize> = HashMap::new();

    for (r, &oc) in &ours_counts {
        let bc = base_counts.get(*r).copied().unwrap_or(0);
        let m = oc.min(bc);
        out.matched += m;
        if oc > m {
            *left_ours.entry((r.code, r.file.as_str())).or_default() += oc - m;
        }
    }
    for (r, &bc) in &base_counts {
        let oc = ours_counts.get(*r).copied().unwrap_or(0);
        let m = oc.min(bc);
        if bc > m {
            let bucket = if r.loc.is_none() {
                &mut left_base_masked
            } else {
                &mut left_base_located
            };
            *bucket.entry((r.code, r.file.as_str())).or_default() += bc - m;
        }
    }

    // Masked baseline related (default-lib `--,--`) matches ours by `(code,file)`.
    for (key, bcount) in &mut left_base_masked {
        if let Some(ocount) = left_ours.get_mut(key) {
            let m = (*ocount).min(*bcount);
            out.matched += m;
            *ocount -= m;
            *bcount -= m;
        }
    }

    // Remaining located leftovers: `(code,file)` pairing = span mismatch; the rest
    // is extra (ours) / missing (baseline).
    let keys: std::collections::HashSet<(u32, &str)> = left_ours
        .keys()
        .chain(left_base_located.keys())
        .chain(left_base_masked.keys())
        .copied()
        .collect();
    for key in keys {
        let oc = left_ours.get(&key).copied().unwrap_or(0);
        let bc = left_base_located.get(&key).copied().unwrap_or(0)
            + left_base_masked.get(&key).copied().unwrap_or(0);
        let sm = oc.min(bc);
        out.span_mismatch += sm;
        out.extra += oc - sm;
        out.missing += bc - sm;
    }
}

/// The four family buckets for one variant.
struct FamilyBuckets {
    matched: usize,
    extra: usize,
    span_mismatch: usize,
    /// The exact matches, per code (for the committed report's per-code table).
    matched_by_code: HashMap<u32, usize>,
    /// The unattributed misses, per code (for cause classification).
    missing_by_code: HashMap<u32, usize>,
}

/// Compare our family multiset against the baseline's: exact `(file,line,col,code)`
/// matches, then greedy `(file,code)` span-mismatch pairing of the leftovers, with
/// the residue split into extra (ours) and missing (baseline).
fn family_buckets(ours: &[FamilyDiag], base: &[FamilyDiag]) -> FamilyBuckets {
    let mut ours_counts: HashMap<&FamilyDiag, usize> = HashMap::new();
    for d in ours {
        *ours_counts.entry(d).or_default() += 1;
    }
    let mut base_counts: HashMap<&FamilyDiag, usize> = HashMap::new();
    for d in base {
        *base_counts.entry(d).or_default() += 1;
    }

    let mut matched = 0usize;
    let mut matched_by_code: HashMap<u32, usize> = HashMap::new();
    // Leftover counts grouped by (file, code) for span-mismatch pairing.
    let mut left_ours: HashMap<(&str, u32), usize> = HashMap::new();
    let mut left_base: HashMap<(&str, u32), usize> = HashMap::new();

    for (d, &oc) in &ours_counts {
        let bc = base_counts.get(d).copied().unwrap_or(0);
        let m = oc.min(bc);
        matched += m;
        if m > 0 {
            *matched_by_code.entry(d.code).or_default() += m;
        }
        if oc > m {
            *left_ours.entry((d.file.as_str(), d.code)).or_default() += oc - m;
        }
    }
    for (d, &bc) in &base_counts {
        let oc = ours_counts.get(d).copied().unwrap_or(0);
        let m = oc.min(bc);
        if bc > m {
            *left_base.entry((d.file.as_str(), d.code)).or_default() += bc - m;
        }
    }

    // Pair leftovers within each (file, code) group: min = span mismatch, the
    // ours residue = extra, the baseline residue = missing.
    let mut span_mismatch = 0usize;
    let mut extra = 0usize;
    let mut missing_by_code: HashMap<u32, usize> = HashMap::new();
    let keys: std::collections::HashSet<(&str, u32)> =
        left_ours.keys().chain(left_base.keys()).copied().collect();
    for &(file, code) in &keys {
        let oc = left_ours.get(&(file, code)).copied().unwrap_or(0);
        let bc = left_base.get(&(file, code)).copied().unwrap_or(0);
        let sm = oc.min(bc);
        span_mismatch += sm;
        extra += oc - sm;
        if bc - sm > 0 {
            *missing_by_code.entry(code).or_default() += bc - sm;
        }
    }

    FamilyBuckets {
        matched,
        extra,
        span_mismatch,
        matched_by_code,
        missing_by_code,
    }
}

/// Classify a parse-rejected variant's baseline shape for the census.
fn baseline_shape(baseline: Option<&Baseline>) -> BaselineShape {
    let Some(baseline) = baseline else {
        return BaselineShape::None;
    };
    let Ok(content) = std::fs::read_to_string(&baseline.path) else {
        return BaselineShape::Other;
    };
    let diags = parse_summary_block(&content);
    if !diags.is_empty() && diags.iter().all(|d| (1000..2000).contains(&d.code)) {
        BaselineShape::Ts1xxxOnly
    } else {
        BaselineShape::Other
    }
}
