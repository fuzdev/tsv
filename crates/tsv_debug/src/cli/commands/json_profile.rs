//! Profile the parse→JSON materialization sub-steps (the FFI parse path).
//!
//! `tsv_ffi`'s `tsv_parse_<lang>` runs `parse` + `convert_ast_json_string`.
//! This command times each sub-step of the `Value`-based pipeline separately,
//! the direct typed pipeline (typed offset translation +
//! `serde_json::to_string(&public_ast)`), and the shipped
//! `convert_ast_json_string`.
//!
//! The sub-step decomposition differs per language because the
//! `convert_ast_json` implementations differ:
//!
//! - **typescript**: convert (typed public AST) → to_value → translate →
//!   to_string. The shipped path instead runs convert → typed translate
//!   (multibyte only) → direct to_string — both pipelines are timed.
//! - **svelte**: convert → to_value → attach (template expression comments,
//!   mutates the `Value`) → translate → to_string; the shipped fast path
//!   (direct serialization) applies only when the source is eligible,
//!   others take the `Value` path
//! - **css**: convert (builds the `Value` directly — no typed-AST tree) →
//!   translate → to_string; the direct path doesn't apply and
//!   `convert_ast_json_string` is a plain wrapper, but the whole-call pair
//!   is still timed and identity-checked like the other languages
//!
//! Per-language fast-path eligibility is documented in
//! `docs/architecture.md` §Closed Scope, Open Convention.
//!
//! The direct path must be byte-identical to the `Value` path (serde_json is
//! built with `preserve_order`, keeping struct-field key order; the typed
//! translation walk must match the `Value` walk). The command checks this per
//! file and reports mismatches — for typescript on **every** file including
//! multibyte (the typed walk runs before `direct` is captured); for svelte on
//! ASCII files only, where a mismatch means the attach pass actually moved
//! comments into the tree (`convert_ast_json_string` gates on this and falls
//! back to the `Value` path). The shipped function is identity-checked against
//! the `Value` path on **every** file, eligible or not.
//!
//! **Sub-step timings exclude drop costs** — each intermediate (public tree,
//! `Value`, strings) outlives its timed region. Recursively freeing a large
//! tree is a significant fraction of a whole call (~30% measured), so the
//! report also times both pipelines as single calls with their internal drops
//! included (`value baseline` / `shipped`) — that whole-call pair is the
//! apples-to-apples delta the FFI boundary actually pays.
//!
//! Run with `--release`; debug-build numbers aren't meaningful.

use argh::FromArgs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;
use tsv_lang::{ByteToCharMap, LocationTracker};

use super::profile::{format_duration, format_size, lang_label, median_us, resolve_profile_files};

/// Bench-corpus exclusions, mirrored from `benches/deno/lib/corpus.ts`:
/// declaration files and build output add noise without exercising new
/// code paths.
fn is_bench_corpus_excluded(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with(".d.ts") || s.contains("/build/") || s.contains("/dist/")
}

/// Profile parse→JSON materialization sub-steps (convert / to_value / translate / to_string).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "json_profile")]
pub struct JsonProfileCommand {
    /// number of iterations (default: 5)
    #[argh(option, default = "5")]
    iterations: usize,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

impl JsonProfileCommand {
    pub fn run(self) {
        let (files, skipped) = resolve_profile_files(&self.paths, is_bench_corpus_excluded);

        let mut results = Vec::new();
        let mut parse_errors = 0usize;

        for path in &files {
            match profile_file(path, self.iterations) {
                Ok(result) => results.push(result),
                Err(_) => parse_errors += 1,
            }
        }

        if results.is_empty() {
            eprintln!("No files profiled successfully ({parse_errors} parse errors).");
            return;
        }

        let aggregates = aggregate(&results);

        if self.json {
            print_json(
                &aggregates,
                &results,
                self.iterations,
                parse_errors,
                skipped,
            );
        } else {
            print_report(&aggregates, self.iterations, parse_errors, skipped);
        }
    }
}

/// Per-file medians for each sub-step (µs). Steps not applicable to a
/// language stay 0 (`to_value`/`attach`/`direct` for css, `attach` for ts,
/// `typed_translate` for svelte/css).
struct FileResult {
    path: PathBuf,
    size: usize,
    wire_bytes: usize,
    parser_type: ParserType,
    multibyte: bool,
    parse_us: f64,
    convert_us: f64,
    to_value_us: f64,
    attach_us: f64,
    translate_us: f64,
    to_string_us: f64,
    typed_translate_us: f64,
    direct_us: f64,
    value_us: f64,
    shipped_us: f64,
    /// Whether the direct path's output is byte-identical to the `Value`
    /// path's. `None` when not comparable (css always; svelte multibyte
    /// sources, which have no typed translation walk). Comparable on every
    /// typescript file — the typed walk translates multibyte trees before
    /// `direct` is captured.
    direct_match: Option<bool>,
    /// Whether `convert_ast_json_string` (the shipped FFI path) is
    /// byte-identical to the `Value` path. Comparable on every file (for
    /// css the shipped fn is a plain wrapper over the same `Value` path,
    /// so the check verifies the wrapper).
    shipped_match: Option<bool>,
}

impl FileResult {
    /// Everything `tsv_parse_<lang>` does after the parse itself.
    fn materialize_us(&self) -> f64 {
        self.convert_us + self.to_value_us + self.attach_us + self.translate_us + self.to_string_us
    }
}

/// Raw per-iteration durations for each sub-step.
#[derive(Default)]
struct StepDurations {
    parse: Vec<Duration>,
    convert: Vec<Duration>,
    to_value: Vec<Duration>,
    attach: Vec<Duration>,
    translate: Vec<Duration>,
    to_string: Vec<Duration>,
    typed_translate: Vec<Duration>,
    direct: Vec<Duration>,
    value: Vec<Duration>,
    shipped: Vec<Duration>,
}

/// Per-file facts recorded on the first iteration only.
struct IterMeta {
    multibyte: bool,
    wire_bytes: usize,
    direct_match: Option<bool>,
    shipped_match: Option<bool>,
}

fn profile_file(path: &Path, iterations: usize) -> Result<FileResult, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let parser_type = ParserType::from_extension(&path.to_string_lossy());

    let mut steps = StepDurations::default();
    let mut meta: Option<IterMeta> = None;

    for _ in 0..iterations {
        match parser_type {
            ParserType::TypeScript => profile_ts_once(&source, &mut steps, &mut meta)?,
            ParserType::Svelte => profile_svelte_once(&source, &mut steps, &mut meta)?,
            ParserType::Css => profile_css_once(&source, &mut steps, &mut meta)?,
        }
    }

    let meta = meta.ok_or("no iterations ran")?;
    let median = |mut v: Vec<Duration>| {
        v.sort();
        median_us(&v)
    };
    Ok(FileResult {
        path: path.to_path_buf(),
        size: source.len(),
        wire_bytes: meta.wire_bytes,
        parser_type,
        multibyte: meta.multibyte,
        parse_us: median(steps.parse),
        convert_us: median(steps.convert),
        to_value_us: median(steps.to_value),
        attach_us: median(steps.attach),
        translate_us: median(steps.translate),
        to_string_us: median(steps.to_string),
        typed_translate_us: median(steps.typed_translate),
        direct_us: median(steps.direct),
        value_us: median(steps.value),
        shipped_us: median(steps.shipped),
        direct_match: meta.direct_match,
        shipped_match: meta.shipped_match,
    })
}

/// One TS iteration. Sub-steps mirror `tsv_ts::convert_ast_json` plus the FFI
/// `to_string` boundary; `typed_translate` + `direct` mirror the shipped
/// typed pipeline (`convert_ast_json_string`). The typed walk mutates the
/// public tree in place, so it runs only after `to_value` has captured the
/// untranslated tree.
#[allow(clippy::expect_used)] // mirrors convert_ast_json's own expect on Serialize
fn profile_ts_once(
    source: &str,
    steps: &mut StepDurations,
    meta: &mut Option<IterMeta>,
) -> Result<(), String> {
    let t = Instant::now();
    let ast = tsv_ts::parse(source).map_err(|e| format!("parse error: {e}"))?;
    steps.parse.push(t.elapsed());

    let t = Instant::now();
    let tracker = LocationTracker::new(source);
    let mut public_ast = tsv_ts::ast::convert::convert_program(
        &ast,
        source,
        &tracker,
        tsv_ts::ast::convert::Schema::Acorn,
    );
    steps.convert.push(t.elapsed());

    let t = Instant::now();
    let mut value =
        serde_json::to_value(&public_ast).expect("AST types derive Serialize correctly");
    steps.to_value.push(t.elapsed());

    let t = Instant::now();
    let map = ByteToCharMap::new(source);
    tsv_ts::ast::convert::translate_byte_to_char_offsets(&mut value, &map, &tracker);
    steps.translate.push(t.elapsed());

    let t = Instant::now();
    let wire = serde_json::to_string(&value).expect("Value serialization cannot fail");
    steps.to_string.push(t.elapsed());

    // The map build is inside the timed region: the shipped
    // `convert_ast_json_string` constructs its own map per call, and the
    // `Value` translate sub-step above times its map build too — excluding
    // it here would bias the walk-vs-walk comparison in the typed walk's
    // favor.
    let t = Instant::now();
    let typed_map = ByteToCharMap::new(source);
    tsv_ts::ast::convert::translate_byte_to_char_offsets_typed(
        &mut public_ast,
        &typed_map,
        &tracker,
    );
    steps.typed_translate.push(t.elapsed());

    let t = Instant::now();
    let direct = serde_json::to_string(&public_ast).expect("AST types derive Serialize correctly");
    steps.direct.push(t.elapsed());

    let first = meta.is_none();
    if first {
        *meta = Some(IterMeta {
            multibyte: map.has_multibyte(),
            wire_bytes: wire.len(),
            direct_match: Some(wire == direct),
            shipped_match: None, // filled in by profile_whole_calls
        });
    }

    profile_whole_calls(
        steps,
        meta,
        first,
        (public_ast, value, direct),
        wire,
        || {
            serde_json::to_string(&tsv_ts::convert_ast_json(&ast, source))
                .expect("Value serialization cannot fail")
        },
        || tsv_ts::convert_ast_json_string(&ast, source),
    );
    Ok(())
}

/// Time the `Value` baseline (`to_string(convert_ast_json)`) and shipped
/// (`convert_ast_json_string`) paths as whole calls. Unlike the sub-step
/// timings, these include dropping the intermediates (tree, `Value`,
/// map/tracker) inside the timed region — exactly what the FFI boundary pays
/// per call. Records the shipped identity check on the first iteration.
///
/// The caller's pipeline `intermediates` are dropped first so both timings
/// see the same heap state the pipeline steps see at iteration start;
/// `wire` is kept only on the first iteration, for the identity check.
fn profile_whole_calls<I>(
    steps: &mut StepDurations,
    meta: &mut Option<IterMeta>,
    first: bool,
    intermediates: I,
    wire: String,
    value_fn: impl Fn() -> String,
    shipped_fn: impl Fn() -> String,
) {
    black_box(&intermediates);
    drop(intermediates);
    let wire_check = if first {
        Some(wire)
    } else {
        drop(wire);
        None
    };

    let t = Instant::now();
    let value = value_fn();
    steps.value.push(t.elapsed());
    black_box(value);

    let t = Instant::now();
    let shipped = shipped_fn();
    steps.shipped.push(t.elapsed());

    if let (Some(wire), Some(m)) = (wire_check, meta.as_mut()) {
        m.shipped_match = Some(wire == shipped);
    }
    black_box(shipped);
}

/// One Svelte iteration. Sub-steps mirror `tsv_svelte::convert_ast_json`,
/// which has an extra `Value`-mutating pass: template expression comment
/// attachment between to_value and translate.
#[allow(clippy::expect_used)] // mirrors convert_ast_json's own expect on Serialize
fn profile_svelte_once(
    source: &str,
    steps: &mut StepDurations,
    meta: &mut Option<IterMeta>,
) -> Result<(), String> {
    let t = Instant::now();
    let ast = tsv_svelte::parse(source).map_err(|e| format!("parse error: {e}"))?;
    steps.parse.push(t.elapsed());

    let t = Instant::now();
    let public_ast = tsv_svelte::ast::convert::convert_root(&ast, source);
    steps.convert.push(t.elapsed());

    let t = Instant::now();
    let mut value =
        serde_json::to_value(&public_ast).expect("AST types derive Serialize correctly");
    steps.to_value.push(t.elapsed());

    let t = Instant::now();
    let script_spans = tsv_svelte::script_content_spans(&ast);
    tsv_svelte::ast::convert::attach_template_expression_comments(
        &mut value,
        &ast.comments,
        &script_spans,
        source,
    );
    steps.attach.push(t.elapsed());

    let t = Instant::now();
    let map = ByteToCharMap::new(source);
    let tracker = LocationTracker::new(source);
    tsv_ts::ast::convert::translate_byte_to_char_offsets(&mut value, &map, &tracker);
    steps.translate.push(t.elapsed());

    let t = Instant::now();
    let wire = serde_json::to_string(&value).expect("Value serialization cannot fail");
    steps.to_string.push(t.elapsed());

    let t = Instant::now();
    let direct = serde_json::to_string(&public_ast).expect("AST types derive Serialize correctly");
    steps.direct.push(t.elapsed());

    let first = meta.is_none();
    if first {
        let multibyte = map.has_multibyte();
        *meta = Some(IterMeta {
            multibyte,
            wire_bytes: wire.len(),
            direct_match: if multibyte {
                None
            } else {
                Some(wire == direct)
            },
            shipped_match: None, // filled in by profile_whole_calls
        });
    }

    profile_whole_calls(
        steps,
        meta,
        first,
        (public_ast, value, direct),
        wire,
        || {
            serde_json::to_string(&tsv_svelte::convert_ast_json(&ast, source))
                .expect("Value serialization cannot fail")
        },
        || tsv_svelte::convert_ast_json_string(&ast, source),
    );
    Ok(())
}

/// One CSS iteration. `tsv_css::convert_ast_json` builds the `Value` directly
/// during conversion (no typed public-AST tree), so there's no separate
/// to_value step and no direct typed-serialization path to compare. The
/// whole-call pair is still timed and identity-checked — the shipped
/// `convert_ast_json_string` is a plain wrapper, so the pair documents that
/// there's no shipped win for css and the check verifies the wrapper.
#[allow(clippy::expect_used)] // Value serialization cannot fail
fn profile_css_once(
    source: &str,
    steps: &mut StepDurations,
    meta: &mut Option<IterMeta>,
) -> Result<(), String> {
    let t = Instant::now();
    let ast = tsv_css::parse(source).map_err(|e| format!("parse error: {e}"))?;
    steps.parse.push(t.elapsed());

    let t = Instant::now();
    let mut value = tsv_css::ast::convert::convert_css_nodes_standalone(&ast.nodes, source);
    steps.convert.push(t.elapsed());

    let t = Instant::now();
    let map = ByteToCharMap::new(source);
    tsv_css::ast::convert::translate_byte_to_char_offsets(&mut value, &map);
    steps.translate.push(t.elapsed());

    let t = Instant::now();
    let wire = serde_json::to_string(&value).expect("Value serialization cannot fail");
    steps.to_string.push(t.elapsed());

    let first = meta.is_none();
    if first {
        *meta = Some(IterMeta {
            multibyte: map.has_multibyte(),
            wire_bytes: wire.len(),
            direct_match: None,
            shipped_match: None, // filled in by profile_whole_calls
        });
    }

    profile_whole_calls(
        steps,
        meta,
        first,
        value,
        wire,
        || {
            serde_json::to_string(&tsv_css::convert_ast_json(&ast, source))
                .expect("Value serialization cannot fail")
        },
        || tsv_css::convert_ast_json_string(&ast, source),
    );
    Ok(())
}

/// Sums of per-file medians for one language.
#[derive(Default)]
struct LangAggregate {
    files: usize,
    size: usize,
    wire_bytes: usize,
    multibyte_files: usize,
    parse_us: f64,
    convert_us: f64,
    to_value_us: f64,
    attach_us: f64,
    translate_us: f64,
    to_string_us: f64,
    typed_translate_us: f64,
    direct_us: f64,
    value_us: f64,
    shipped_us: f64,
    materialize_us: f64,
    direct_match: usize,
    direct_mismatch: usize,
    shipped_match: usize,
    shipped_mismatch: usize,
}

fn aggregate(results: &[FileResult]) -> Vec<(ParserType, LangAggregate)> {
    let mut out: Vec<(ParserType, LangAggregate)> = Vec::new();
    for r in results {
        let agg = match out.iter_mut().find(|(t, _)| *t == r.parser_type) {
            Some((_, agg)) => agg,
            None => {
                out.push((r.parser_type, LangAggregate::default()));
                // SAFETY: just pushed
                #[allow(clippy::unwrap_used)]
                let last = out.last_mut().unwrap();
                &mut last.1
            }
        };
        agg.files += 1;
        agg.size += r.size;
        agg.wire_bytes += r.wire_bytes;
        agg.multibyte_files += usize::from(r.multibyte);
        agg.parse_us += r.parse_us;
        agg.convert_us += r.convert_us;
        agg.to_value_us += r.to_value_us;
        agg.attach_us += r.attach_us;
        agg.translate_us += r.translate_us;
        agg.to_string_us += r.to_string_us;
        agg.typed_translate_us += r.typed_translate_us;
        agg.direct_us += r.direct_us;
        agg.value_us += r.value_us;
        agg.shipped_us += r.shipped_us;
        agg.materialize_us += r.materialize_us();
        match r.direct_match {
            Some(true) => agg.direct_match += 1,
            Some(false) => agg.direct_mismatch += 1,
            None => {}
        }
        match r.shipped_match {
            Some(true) => agg.shipped_match += 1,
            Some(false) => agg.shipped_mismatch += 1,
            None => {}
        }
    }
    out
}

fn pct(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        part / whole * 100.0
    } else {
        0.0
    }
}

fn print_report(
    aggregates: &[(ParserType, LangAggregate)],
    iterations: usize,
    parse_errors: usize,
    skipped: usize,
) {
    for (parser_type, a) in aggregates {
        let is_css = matches!(parser_type, ParserType::Css);
        let is_ts = matches!(parser_type, ParserType::TypeScript);
        eprintln!(
            "{} — {} files, {} source, {} wire JSON, {} multibyte",
            lang_label(*parser_type),
            a.files,
            format_size(a.size),
            format_size(a.wire_bytes),
            a.multibyte_files,
        );
        eprintln!("  parse            {:>10}", format_duration(a.parse_us));
        eprintln!(
            "  materialization  {:>10}  (convert_ast_json + to_string)",
            format_duration(a.materialize_us)
        );
        let step = |label: &str, us: f64| {
            eprintln!(
                "    {label:<14} {:>10}  {:>4.0}%",
                format_duration(us),
                pct(us, a.materialize_us)
            );
        };
        if is_css {
            step("convert→Value", a.convert_us);
        } else {
            step("convert", a.convert_us);
            step("to_value", a.to_value_us);
        }
        if a.attach_us > 0.0 {
            step("attach", a.attach_us);
        }
        step("translate", a.translate_us);
        step("to_string", a.to_string_us);
        if !is_css {
            eprintln!(
                "  direct to_string {:>10}  (to_string(&public_ast), skips Value)",
                format_duration(a.direct_us)
            );
            if is_ts {
                // The shipped typed pipeline, sub-step sum (drops excluded):
                // the typed walk early-returns on ASCII files.
                eprintln!(
                    "  typed translate  {:>10}  (map build + typed walk; walk is multibyte-only)",
                    format_duration(a.typed_translate_us)
                );
                let typed_pipeline = a.convert_us + a.typed_translate_us + a.direct_us;
                eprintln!(
                    "  typed pipeline   {:>10}  = convert + typed translate + direct → {:.2}x of materialization",
                    format_duration(typed_pipeline),
                    if a.materialize_us > 0.0 {
                        typed_pipeline / a.materialize_us
                    } else {
                        0.0
                    },
                );
            }
        }
        eprintln!(
            "  value baseline   {:>10}  = to_string(convert_ast_json) whole call, incl. drops",
            format_duration(a.value_us),
        );
        eprintln!(
            "  shipped          {:>10}  = convert_ast_json_string whole call → {:.2}x of value baseline",
            format_duration(a.shipped_us),
            if a.value_us > 0.0 {
                a.shipped_us / a.value_us
            } else {
                0.0
            },
        );
        if !is_css {
            let comparable = a.direct_match + a.direct_mismatch;
            eprintln!(
                "  direct == value on {}/{comparable} comparable files{}",
                a.direct_match,
                if is_ts { "" } else { " (ASCII only)" },
            );
        }
        let shipped_comparable = a.shipped_match + a.shipped_mismatch;
        eprintln!(
            "  shipped == value on {}/{shipped_comparable} files",
            a.shipped_match
        );
        eprintln!();
    }

    let mut notes = vec![format!(
        "iterations: {iterations} (sums of per-file medians)"
    )];
    if parse_errors > 0 {
        notes.push(format!("{parse_errors} files skipped (parse errors)"));
    }
    if skipped > 0 {
        notes.push(format!("{skipped} input_invalid skipped"));
    }
    eprintln!("{}", notes.join(", "));
}

fn print_json(
    aggregates: &[(ParserType, LangAggregate)],
    results: &[FileResult],
    iterations: usize,
    parse_errors: usize,
    skipped: usize,
) {
    let languages: serde_json::Map<String, serde_json::Value> = aggregates
        .iter()
        .map(|(parser_type, a)| {
            let mut lang = serde_json::json!({
                "files": a.files,
                "size_bytes": a.size,
                "wire_bytes": a.wire_bytes,
                "multibyte_files": a.multibyte_files,
                "parse_us": a.parse_us,
                "convert_us": a.convert_us,
                "to_value_us": a.to_value_us,
                "attach_us": a.attach_us,
                "translate_us": a.translate_us,
                "to_string_us": a.to_string_us,
                "typed_translate_us": a.typed_translate_us,
                "direct_us": a.direct_us,
                "value_us": a.value_us,
                "shipped_us": a.shipped_us,
                "materialize_us": a.materialize_us,
                "direct_match": a.direct_match,
                "direct_mismatch": a.direct_mismatch,
                "shipped_match": a.shipped_match,
                "shipped_mismatch": a.shipped_mismatch,
            });
            // the typed pipeline exists only for typescript — the shipped
            // svelte path falls back per file and css has no typed tree
            if matches!(parser_type, ParserType::TypeScript) {
                lang["typed_pipeline_us"] =
                    serde_json::Value::from(a.convert_us + a.typed_translate_us + a.direct_us);
            }
            (lang_label(*parser_type).to_string(), lang)
        })
        .collect();

    let files: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path.to_string_lossy(),
                "lang": lang_label(r.parser_type),
                "size_bytes": r.size,
                "wire_bytes": r.wire_bytes,
                "multibyte": r.multibyte,
                "parse_us": r.parse_us,
                "convert_us": r.convert_us,
                "to_value_us": r.to_value_us,
                "attach_us": r.attach_us,
                "translate_us": r.translate_us,
                "to_string_us": r.to_string_us,
                "typed_translate_us": r.typed_translate_us,
                "direct_us": r.direct_us,
                "value_us": r.value_us,
                "shipped_us": r.shipped_us,
                "materialize_us": r.materialize_us(),
                "direct_match": r.direct_match,
                "shipped_match": r.shipped_match,
            })
        })
        .collect();

    let output = serde_json::json!({
        "iterations": iterations,
        "parse_errors": parse_errors,
        "skipped": skipped,
        "languages": languages,
        "files": files,
    });

    // SAFETY: serde_json Value types always serialize successfully
    #[allow(clippy::unwrap_used)]
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{json_str}");
}
