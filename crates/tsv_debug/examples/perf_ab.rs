//! The measuring driver behind `npm run perf:ab` (`scripts/perf_ab.ts`): one language of a
//! corpus, timed phase for phase the way `tsv_debug profile` times it, printed as one JSON line.
//!
//! **Why an example and not a `profile` flag.** An A/B builds this file into BOTH trees — the
//! candidate and an older base ref, which may be a release tag. The task copies the current
//! checkout's copy over each snapshot before building, so both sides run the same measuring
//! code and only the measured libraries differ. That only works for a file that compiles
//! against an old tree unchanged: an example is discovered by cargo with no manifest or
//! subcommand registration to patch, and this one reads nothing but the language crates' hidden
//! entry points, through its `api` arms (below): the folded parse (`parse_folded`, or
//! `parse_folded_into` into stores kept warm, by `tsv_ts::__private::WarmStores` or, in an older
//! tree, by the stores' own `Default` and `park`), the format (`format_folded_in`, through
//! `parsed_ref` beside the stores), the carriage-return fold and `DocArena`.
//! `profile` itself could not be overlaid — it carries every mode the subcommand grew since.
//! A base tree that lacks one of those entry points fails the build, loudly.
//!
//! **One driver, one arm per entry-point generation.** The entry points this driver calls
//! change as the parse moves from the `Bump` into stores, so the calls sit in an `api`
//! module with one arm per generation of the trees it measures, and everything else — the
//! walk, the timing, the statistics — is shared. The unmarked arm is the current tree's, which
//! every `cargo check --all-targets` compiles; an older generation's arm is selected by a
//! `perf_ab_api` cfg the task passes to this example's own compilation (`cargo rustc … --
//! --cfg`, so no library is built differently) after probing the side's tree for it
//! (`DRIVER_APIS` in the task). An older arm compiles only against trees of its generation and
//! is never type-checked in the current one. When a change moves an entry point the driver
//! calls, its arm becomes the previous generation's (a new `perf_ab_api` value, with the
//! task's probe for it) and the unmarked arm takes the new calls.
//!
//! - The current arm parses TypeScript and Svelte into warm stores through a
//!   `tsv_ts::__private::WarmStores` handle, which takes them from the previous iteration before
//!   the timed regions and parks them after, as the CLI's `Formatter` keeps them in a slot it
//!   owns; the arm names no store type and no lifetime, so its text stays the same while the
//!   stores change beneath the handle. The TypeScript parse takes the stores alone, the Svelte
//!   parse the `Bump` beside them, and CSS has no stores and parses into the `Bump` alone.
//! - `perf_ab_api = "strip_bump"`, a tree whose strip still takes the `Bump` for its edits, calls
//!   this driver's entry points as the current tree does, so it builds the current arm; only the
//!   strip driver (`perf_ab_strip.rs`) has an arm of its own for it.
//! - `perf_ab_api = "no_warm_stores"`, a tree with the stores but no `WarmStores`, parses as the
//!   current arm does, keeping the stores itself: taken out of an `Option` before the timed
//!   regions and parked back into it after them.
//! - `perf_ab_api = "ts_parse_bump"`, a tree whose TypeScript parse still takes the `Bump`
//!   beside the stores, parses as `no_warm_stores` does but hands the `Bump` to that parse too.
//! - `perf_ab_api = "no_stores"`, a tree older than the parse stores, parses every language
//!   into the `Bump` alone.
//!
//! Usage: `perf_ab <corpus-dir> --lang ts|svelte|css --iterations N [--parse-only]`.
//!
//! - The phases are `profile`'s format mode: the fold and the parse in the parse column, the
//!   format in the format column; one AST `Bump`, one `DocArena` and the arm's warm stores
//!   reused across every file and iteration, reset or parked between them outside the timed
//!   regions.
//! - The file set is `profile`'s directory walk (the discovery prune policy, the formattable
//!   extensions, `input_invalid_*` dropped), filtered to the files `--lang` parses with.
//! - `--parse-only` skips the format, so an `instructions:u` count around this process splits by
//!   phase (format = full − parse-only); `--iterations 0` reads the files and times nothing,
//!   which is the run's fixed cost.
//!
//! - The arena resets and the stores' take and park sit outside the timed regions (as in
//!   `profile`, whose wall numbers this matches), but inside the process an `instructions:u`
//!   count covers — so the instruction column carries them (the `DocArena` reset lands in
//!   format = full − parse-only, the take and park in parse = parse-only − fixed) and the
//!   wall column does not. Excluding them from a process-level count is not possible, and timing
//!   them would part the wall column from `profile`'s; a change that moves work into or out of a
//!   reset shows in the instruction column alone.
//! - Formatting has no error channel; a formatter panic aborts this process (the release
//!   profile's `panic = "abort"`), which the task reports as a harness failure. The output is
//!   kept (`black_box`) and its length summed, so a run whose formatted output changes between
//!   runs of one binary is caught as nondeterministic.
//!
//! Output (stdout): `{"lang","files","bytes","errors","iterations","parse_us","format_us",
//! "output_bytes"}` — the per-file medians summed over the language, in microseconds, and the
//! formatted bytes of each file's first iteration, summed. A file that fails to parse is
//! counted in `errors` and left out of the sums.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;
use tsv_lang::doc::arena::DocArena;
use tsv_lang::printing::{FoldedSource, normalize_carriage_returns};

struct Args {
    corpus: PathBuf,
    parser_type: ParserType,
    iterations: usize,
    parse_only: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut corpus = None;
    let mut parser_type = None;
    let mut iterations = 10;
    let mut parse_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lang" => {
                parser_type = Some(match args.next().as_deref() {
                    Some("ts") => ParserType::TypeScript,
                    Some("svelte") => ParserType::Svelte,
                    Some("css") => ParserType::Css,
                    other => return Err(format!("--lang takes ts|svelte|css, got {other:?}")),
                });
            }
            "--iterations" => {
                let value = args.next().ok_or("--iterations takes a count")?;
                iterations = value
                    .parse()
                    .map_err(|e| format!("--iterations {value}: {e}"))?;
            }
            "--parse-only" => parse_only = true,
            _ if arg.starts_with("--") => return Err(format!("unknown flag {arg}")),
            _ if corpus.is_none() => corpus = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    Ok(Args {
        corpus: corpus.ok_or("missing <corpus-dir>")?,
        parser_type: parser_type.ok_or("missing --lang")?,
        iterations,
        parse_only,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!(
                "perf_ab: {message}\nusage: perf_ab <corpus-dir> --lang ts|svelte|css --iterations N [--parse-only]"
            );
            return ExitCode::from(2);
        }
    };
    if !args.corpus.is_dir() {
        eprintln!(
            "perf_ab: corpus is not a directory: {}",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }
    // The stack reservation every tsv binary states — a deep real file must not overflow here
    // when it formats fine through `profile`.
    tsv_cli::cli::stack::run_on_sized_stack(move || run(&args))
}

fn run(args: &Args) -> ExitCode {
    let mut files = Vec::new();
    collect_files(&args.corpus, &mut files);
    files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    files.retain(|p| {
        !p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("input_invalid"))
            && ParserType::from_extension(&p.to_string_lossy()) == args.parser_type
    });

    let mut arena = bumpalo::Bump::new();
    let mut doc_arena = DocArena::new();
    let mut warm = api::Warm::default();
    let mut ok_files = 0usize;
    let mut bytes = 0usize;
    let mut errors = 0usize;
    let mut parse_us = 0.0;
    let mut format_us = 0.0;
    let mut output_bytes = 0usize;
    let mut parse_times = Vec::with_capacity(args.iterations);
    let mut format_times = Vec::with_capacity(args.iterations);

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("perf_ab: read error {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        parse_times.clear();
        format_times.clear();
        let mut failed = false;
        let mut file_output_bytes = 0;
        for iteration in 0..args.iterations {
            let timed = time_once(
                &source,
                args.parser_type,
                args.parse_only,
                &arena,
                &doc_arena,
                &mut warm,
            );
            // Teardown outside the timed regions, as `profile` does.
            arena.reset();
            doc_arena.reset();
            match timed {
                Ok((parse, format, output_len)) => {
                    if iteration == 0 {
                        file_output_bytes = output_len;
                    }
                    parse_times.push(parse);
                    format_times.push(format);
                }
                Err(e) => {
                    eprintln!("perf_ab: {}: {e}", path.display());
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            errors += 1;
            continue;
        }
        ok_files += 1;
        bytes += source.len();
        output_bytes += file_output_bytes;
        parse_us += median_us(&mut parse_times);
        format_us += median_us(&mut format_times);
    }

    let lang = match args.parser_type {
        ParserType::TypeScript => "ts",
        ParserType::Svelte => "svelte",
        ParserType::Css => "css",
    };
    println!(
        "{{\"lang\":\"{lang}\",\"files\":{ok_files},\"bytes\":{bytes},\"errors\":{errors},\"iterations\":{},\"parse_us\":{parse_us},\"format_us\":{format_us},\"output_bytes\":{output_bytes}}}",
        args.iterations
    );
    ExitCode::SUCCESS
}

/// One iteration of `profile`'s format mode, in the language `parser_type` names, through the
/// entry points of the tree this driver is built against (the `api` module).
fn time_once(
    source: &str,
    parser_type: ParserType,
    parse_only: bool,
    arena: &bumpalo::Bump,
    doc_arena: &DocArena,
    warm: &mut api::Warm,
) -> Timed {
    match parser_type {
        ParserType::TypeScript => api::ts(source, parse_only, arena, doc_arena, warm),
        ParserType::Svelte => api::svelte(source, parse_only, arena, doc_arena, warm),
        ParserType::Css => api::css(source, parse_only, arena, doc_arena, warm),
    }
}

/// One iteration's parse and format walls and the formatted length (both zero under
/// `--parse-only`), or why the file failed.
type Timed = Result<(Duration, Duration, usize), String>;

/// The timed phases every language and every `api` arm shares: the fold and `parse` timed as
/// the parse, then `format` (skipped under `parse_only`) as the format. `state` is what both
/// closures read — an arm's warm stores, or nothing — handed to each in turn so neither holds
/// it across the other.
fn time_phases<S, P>(
    source: &str,
    parse_only: bool,
    state: &mut S,
    parse: impl FnOnce(&FoldedSource<'_>, &mut S) -> Result<P, tsv_lang::Error>,
    format: impl FnOnce(&P, &FoldedSource<'_>, &S) -> String,
) -> Timed {
    let t0 = Instant::now();
    let folded = normalize_carriage_returns(source);
    let ast = parse(&folded, state).map_err(|e| format!("parse error: {e}"))?;
    let parse = t0.elapsed();
    if parse_only {
        return Ok((parse, Duration::ZERO, 0));
    }
    let t1 = Instant::now();
    let len = formatted_len(format(&ast, &folded, state));
    Ok((parse, t1.elapsed(), len))
}

/// The entry points of the trees this driver measures: the current tree's, and under a
/// `perf_ab_api` value those of an older generation (module doc).
///
/// Each arm's `Warm` is what it keeps across files and iterations besides the AST `Bump`: the
/// parked stores, taken before the timed regions and parked after them, so a parse reuses warm
/// stores as the oldest arm reuses its `Bump`.
#[cfg(not(any(
    perf_ab_api = "no_warm_stores",
    perf_ab_api = "ts_parse_bump",
    perf_ab_api = "no_stores"
)))]
mod api {
    use super::{Timed, time_phases};
    use tsv_lang::doc::arena::DocArena;

    /// The stores parked between iterations, rewound, shared by the TypeScript and Svelte
    /// parses (a Svelte document's islands parse into `tsv_ts`'s stores).
    pub(super) type Warm = tsv_ts::__private::WarmStores;

    pub(super) fn ts(
        source: &str,
        parse_only: bool,
        _arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        warm: &mut Warm,
    ) -> Timed {
        warm.with(|stores| {
            time_phases(
                source,
                parse_only,
                stores,
                |folded, stores| tsv_ts::__private::parse_folded_into(folded, None, stores),
                |program, folded, stores| {
                    let parsed = tsv_ts::__private::parsed_ref(program, stores);
                    tsv_ts::__private::format_folded_in(parsed, folded, doc_arena)
                },
            )
        })
    }

    pub(super) fn svelte(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        warm: &mut Warm,
    ) -> Timed {
        warm.with(|stores| {
            time_phases(
                source,
                parse_only,
                stores,
                |folded, stores| tsv_svelte::__private::parse_folded_into(folded, arena, stores),
                |root, folded, stores| {
                    let parsed = tsv_svelte::__private::parsed_ref(root, stores);
                    tsv_svelte::__private::format_folded_in(parsed, folded, doc_arena)
                },
            )
        })
    }

    pub(super) fn css(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        _warm: &mut Warm,
    ) -> Timed {
        time_phases(
            source,
            parse_only,
            &mut (),
            |folded, ()| tsv_css::__private::parse_folded(folded, arena),
            |stylesheet, folded, ()| {
                tsv_css::__private::format_folded_in(stylesheet, folded, doc_arena)
            },
        )
    }
}

/// The entry points of a tree with the stores but no `WarmStores` handle: as the current arm,
/// the stores kept in an `Option` of the arm's own. Built only against such a tree, never
/// against the current one.
#[cfg(perf_ab_api = "no_warm_stores")]
mod api {
    use super::{Timed, time_phases};
    use tsv_lang::doc::arena::DocArena;
    use tsv_ts::ast::AstStores;

    /// The stores parked between iterations, rewound, shared by the TypeScript and Svelte
    /// parses (a Svelte document's islands parse into `tsv_ts`'s stores).
    #[derive(Default)]
    pub(super) struct Warm {
        stores: Option<AstStores<'static>>,
    }

    pub(super) fn ts(
        source: &str,
        parse_only: bool,
        _arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        warm: &mut Warm,
    ) -> Timed {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let timed = time_phases(
            source,
            parse_only,
            &mut stores,
            |folded, stores| tsv_ts::__private::parse_folded_into(folded, None, stores),
            |program, folded, stores| {
                let parsed = tsv_ts::__private::parsed_ref(program, stores);
                tsv_ts::__private::format_folded_in(parsed, folded, doc_arena)
            },
        );
        warm.stores = Some(stores.park());
        timed
    }

    pub(super) fn svelte(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        warm: &mut Warm,
    ) -> Timed {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let timed = time_phases(
            source,
            parse_only,
            &mut stores,
            |folded, stores| tsv_svelte::__private::parse_folded_into(folded, arena, stores),
            |root, folded, stores| {
                let parsed = tsv_svelte::__private::parsed_ref(root, stores);
                tsv_svelte::__private::format_folded_in(parsed, folded, doc_arena)
            },
        );
        warm.stores = Some(stores.park());
        timed
    }

    pub(super) fn css(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        _warm: &mut Warm,
    ) -> Timed {
        time_phases(
            source,
            parse_only,
            &mut (),
            |folded, ()| tsv_css::__private::parse_folded(folded, arena),
            |stylesheet, folded, ()| {
                tsv_css::__private::format_folded_in(stylesheet, folded, doc_arena)
            },
        )
    }
}

/// The entry points of a tree whose TypeScript parse still takes the `Bump` beside the stores:
/// TypeScript and Svelte parse into warm stores, both through the `Bump`. Built only against
/// such a tree, never against the current one.
#[cfg(perf_ab_api = "ts_parse_bump")]
mod api {
    use super::{Timed, time_phases};
    use tsv_lang::doc::arena::DocArena;
    use tsv_ts::ast::AstStores;

    /// The stores parked between iterations, rewound, shared by the TypeScript and Svelte
    /// parses (a Svelte document's islands parse into `tsv_ts`'s stores).
    #[derive(Default)]
    pub(super) struct Warm {
        stores: Option<AstStores<'static>>,
    }

    pub(super) fn ts(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        warm: &mut Warm,
    ) -> Timed {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let timed = time_phases(
            source,
            parse_only,
            &mut stores,
            |folded, stores| tsv_ts::__private::parse_folded_into(folded, None, arena, stores),
            |program, folded, stores| {
                let parsed = tsv_ts::__private::parsed_ref(program, stores);
                tsv_ts::__private::format_folded_in(parsed, folded, doc_arena)
            },
        );
        warm.stores = Some(stores.park());
        timed
    }

    pub(super) fn svelte(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        warm: &mut Warm,
    ) -> Timed {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let timed = time_phases(
            source,
            parse_only,
            &mut stores,
            |folded, stores| tsv_svelte::__private::parse_folded_into(folded, arena, stores),
            |root, folded, stores| {
                let parsed = tsv_svelte::__private::parsed_ref(root, stores);
                tsv_svelte::__private::format_folded_in(parsed, folded, doc_arena)
            },
        );
        warm.stores = Some(stores.park());
        timed
    }

    pub(super) fn css(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        _warm: &mut Warm,
    ) -> Timed {
        time_phases(
            source,
            parse_only,
            &mut (),
            |folded, ()| tsv_css::__private::parse_folded(folded, arena),
            |stylesheet, folded, ()| {
                tsv_css::__private::format_folded_in(stylesheet, folded, doc_arena)
            },
        )
    }
}

/// The entry points of a tree older than the parse stores: every parse into the `Bump` alone.
/// Built only against such a tree, never against the current one.
#[cfg(perf_ab_api = "no_stores")]
mod api {
    use super::{Timed, time_phases};
    use tsv_lang::doc::arena::DocArena;

    /// Nothing is kept warm beside the `Bump`.
    #[derive(Default)]
    pub(super) struct Warm;

    pub(super) fn ts(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        _warm: &mut Warm,
    ) -> Timed {
        time_phases(
            source,
            parse_only,
            &mut (),
            |folded, ()| tsv_ts::__private::parse_folded(folded, None, arena),
            |ast, folded, ()| tsv_ts::__private::format_folded_in(ast, folded, doc_arena),
        )
    }

    pub(super) fn svelte(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        _warm: &mut Warm,
    ) -> Timed {
        time_phases(
            source,
            parse_only,
            &mut (),
            |folded, ()| tsv_svelte::__private::parse_folded(folded, arena),
            |ast, folded, ()| tsv_svelte::__private::format_folded_in(ast, folded, doc_arena),
        )
    }

    pub(super) fn css(
        source: &str,
        parse_only: bool,
        arena: &bumpalo::Bump,
        doc_arena: &DocArena,
        _warm: &mut Warm,
    ) -> Timed {
        time_phases(
            source,
            parse_only,
            &mut (),
            |folded, ()| tsv_css::__private::parse_folded(folded, arena),
            |stylesheet, folded, ()| {
                tsv_css::__private::format_folded_in(stylesheet, folded, doc_arena)
            },
        )
    }
}

/// The formatted output's length, the output kept from the optimizer and dropped here — inside
/// the format timer, where `profile`'s `let _ =` drops it.
fn formatted_len(output: String) -> usize {
    std::hint::black_box(output).len()
}

/// The median of `durations` in microseconds (the mean of the middle two for an even count;
/// zero for none) — `profile`'s per-file statistic.
fn median_us(durations: &mut [Duration]) -> f64 {
    durations.sort_unstable();
    let len = durations.len();
    if len == 0 {
        return 0.0;
    }
    let us = |d: Duration| d.as_secs_f64() * 1_000_000.0;
    if len % 2 == 1 {
        us(durations[len / 2])
    } else {
        f64::midpoint(us(durations[len / 2 - 1]), us(durations[len / 2]))
    }
}

/// `profile`'s directory walk: recurse, pruning the discovery safety nets, hidden directories
/// and the build-output heuristic; keep the extensions `tsv format` discovers.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() {
            if tsv_discover::is_safety_net(name)
                || name.starts_with('.')
                || tsv_discover::HEURISTIC_DIRS.contains(&name)
            {
                continue;
            }
            collect_files(&path, files);
        } else if tsv_discover::is_formattable(name) {
            files.push(path);
        }
    }
}
