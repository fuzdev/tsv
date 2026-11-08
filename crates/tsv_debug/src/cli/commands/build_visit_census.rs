//! How many doc-build dispatches the formatter makes over a corpus, per language and per
//! dispatch family.
//!
//! The population [`super::ast_census`] counts is what a parse builds; this counts what a
//! format then dispatches on. Rebuilds are the point: a node built once per layout candidate
//! is dispatched once per candidate, so each row reports distinct nodes beside its visits
//! and the ratio between them.
//!
//! # What a visit is
//!
//! ⚠️ **A visit is one dispatch, not one `&'arena T` read**, and no total here bounds those
//! reads in either direction. A read through such a reference is an auto-deref, which leaves
//! no site to hook. A visit may read several of its node's edges, and the printers' layout
//! predicates (paren decisions, shape tests, chain analysis, comment gates) read edges with
//! no dispatch at all. The other way, a node reached through a by-value slot or a by-value
//! list element reads no edge of its own: it costs at most one read per list, or none. The
//! split between edges and by-value slots is per field: `ast_census --slots` gives each
//! field's population, and the field's type in the AST says which it is.
//!
//! The counted sites are `build_statement_doc`, the type printer's `probe_type_build`, the
//! full expression dispatch (the non-leaf family) and the two leaf arms ahead of it, which
//! answer a literal or an identifier before the full dispatch's site.
//!
//! # Dispatches the census doesn't see
//!
//! A node built past those sites is not counted:
//!
//! - **Binary chains.** One chain counts once, at most: the flattened left spine is walked
//!   without a dispatch. A chain reached by `build_flat_chain_expression_doc`, a call or
//!   array argument (`build_arg_expression_doc`), a return or throw argument, a statement
//!   condition or an embedded expression root does not count at all.
//! - **Member and call chains.** The chain printer flattens the spine, so a link below the
//!   chain's head counts only where something recurses into it.
//! - **Nested ternaries** and a ternary in argument or return position, which take the
//!   conditional builders directly.
//! - **Expanded object and array arguments** (`build_arg_expression_doc_expanded`), and the
//!   object and array patterns that take their builders directly.
//! - **Sequences** that a statement, a `for` head or a return intercepts.
//! - **Names.** An identifier a printer builds with `identifier_name_doc` / `ident_name_doc`
//!   or the `build_identifier_doc` family directly, rather than through the expression
//!   dispatch, is not a leaf visit: member properties, variable, function and class names,
//!   a constructor's parameter properties, import and export specifiers, type names and
//!   labels, and a plain or rest parameter of a function or constructor type, a call,
//!   construct or method signature, or a `declare` or overload signature. A string-literal
//!   key of an object, pattern, class member or type member takes its own builder and is
//!   not one either; every other key, and every other parameter, dispatches.
//! - **Literals outside the expression dispatch**: a module source, an import attribute's
//!   value, a literal type, an enum member's name.
//! - **Types** a type printer arm takes without `build_type_doc`: a union, an intersection,
//!   a conditional's inner branches, or a type literal reached directly.
//! - **Svelte plain paths.** A template value that is a plain path owning no comment prints
//!   as its own bytes (`fused_plain_path`) and is never built. Svelte's own template nodes
//!   have no site at all; only the TypeScript embedded in a document is counted.
//! - **Everything in CSS**: its printer has no build sites, so its families report as not
//!   applicable rather than zero.
//!
//! A chain-argument share hit is deliberately not a visit: it rebuilds nothing.
//!
//! `doc_walks` is the doc walks' visit counter, build-phase and render-phase walks together;
//! it is reported because it costs nothing, and it is not an AST count.
//!
//! Needs the `_internal_fanout_check` feature (`_internal_audits` turns it on); built without
//! it the command refuses, since every counter would read zero.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use argh::FromArgs;
use tsv_cli::cli::input::ParserType;
use tsv_lang::fanout_probe::{self, Budget, Build, Tally};

use crate::audit::panic_hook::panic_message;
use crate::audit::parallel::run_pool;
use crate::cli::CliError;
use crate::format_source::format_source;

use super::profile::resolve_profile_files;

/// Count the doc-build dispatches the formatter makes over a corpus.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "build_visit_census")]
pub struct BuildVisitCensusCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// worker threads (default: available parallelism). Results are `--jobs`-invariant
    #[argh(option)]
    jobs: Option<usize>,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

/// Every counter unbounded: a census grades a real corpus, not a shape that may blow up.
const BUDGET: Budget = Budget {
    builds: u64::MAX,
    walks: u64::MAX,
    scans: u64::MAX,
    predicates: u64::MAX,
    nodes: u64::MAX,
};

/// The dispatch families, in report order.
const FAMILIES: [&str; 4] = ["expression_nonleaf", "expression_leaf", "type", "statement"];
const EXPRESSION_NONLEAF: usize = 0;
const EXPRESSION_LEAF: usize = 1;
const TYPE: usize = 2;
const STATEMENT: usize = 3;

/// The languages, in report order, with whether any build site can fire under them.
const LANGUAGES: [(&str, bool); 3] = [("typescript", true), ("svelte", true), ("css", false)];

fn language_index(parser: ParserType) -> usize {
    match parser {
        ParserType::TypeScript => 0,
        ParserType::Svelte => 1,
        ParserType::Css => 2,
    }
}

#[derive(Clone, Copy, Default)]
struct FamilyCount {
    visits: u64,
    distinct: u64,
}

impl FamilyCount {
    fn add(&mut self, other: Self) {
        self.visits += other.visits;
        self.distinct += other.distinct;
    }

    #[expect(clippy::cast_precision_loss)]
    fn ratio(self) -> f64 {
        if self.distinct == 0 {
            0.0
        } else {
            self.visits as f64 / self.distinct as f64
        }
    }
}

#[derive(Default)]
struct LanguageTotals {
    files: u64,
    rejected: u64,
    source_bytes: u64,
    doc_walks: u64,
    families: [FamilyCount; FAMILIES.len()],
}

impl LanguageTotals {
    fn merge(&mut self, other: &Self) {
        self.files += other.files;
        self.rejected += other.rejected;
        self.source_bytes += other.source_bytes;
        self.doc_walks += other.doc_walks;
        for (mine, theirs) in self.families.iter_mut().zip(other.families) {
            mine.add(theirs);
        }
    }

    /// One graded file's counters: visits are a node's builds summed, distinct its entries.
    fn add_tally(&mut self, tally: &Tally) {
        self.doc_walks += tally.walks;
        for &(key, n) in &tally.builds {
            let family = match key.dispatch {
                Build::Expression => EXPRESSION_NONLEAF,
                Build::Type => TYPE,
                Build::Statement => STATEMENT,
            };
            self.families[family].add(FamilyCount {
                visits: u64::from(n),
                distinct: 1,
            });
        }
        for &(_, n) in &tally.leaf_builds {
            self.families[EXPRESSION_LEAF].add(FamilyCount {
                visits: u64::from(n),
                distinct: 1,
            });
        }
    }
}

#[derive(Default)]
struct Census {
    languages: [LanguageTotals; LANGUAGES.len()],
    /// `path: panic message`, for every file whose format panicked.
    panics: Vec<String>,
}

impl Census {
    fn merge(&mut self, other: Self) {
        for (mine, theirs) in self.languages.iter_mut().zip(&other.languages) {
            mine.merge(theirs);
        }
        self.panics.extend(other.panics);
    }
}

fn census_file(path: &Path, census: &mut Census) {
    let parser = ParserType::from_extension(&path.to_string_lossy());
    let totals = &mut census.languages[language_index(parser)];
    let Ok(source) = std::fs::read_to_string(path) else {
        totals.rejected += 1;
        return;
    };
    fanout_probe::arm(BUDGET);
    let result = catch_unwind(AssertUnwindSafe(|| format_source(&source, parser)));
    let tally = fanout_probe::disarm();
    match result {
        Ok(Ok(_)) => {
            totals.files += 1;
            totals.source_bytes += source.len() as u64;
            totals.add_tally(&tally);
        }
        Ok(Err(_)) => totals.rejected += 1,
        Err(payload) => {
            census
                .panics
                .push(format!("{}: {}", path.display(), panic_message(&*payload)));
        }
    }
}

impl BuildVisitCensusCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if !fanout_probe::ENABLED {
            eprintln!(
                "Error: build_visit_census needs the work counters: build with `--features \
                 _internal_fanout_check` (or `_internal_audits`). Without them every count \
                 would read zero."
            );
            return Err(CliError::Failed);
        }
        let (files, skipped) = resolve_profile_files(&self.paths, |_| true)?;
        let mut census = run_pool(
            &files,
            self.jobs,
            |path, census: &mut Census| census_file(path, census),
            Census::merge,
        )?;

        if !census.panics.is_empty() {
            census.panics.sort_unstable();
            for line in &census.panics {
                eprintln!("panic: {line}");
            }
            eprintln!(
                "Error: {} file(s) panicked in format; a census missing them is not a total.",
                census.panics.len()
            );
            return Err(CliError::Failed);
        }
        let graded: u64 = census.languages.iter().map(|l| l.files).sum();
        if graded == 0 {
            eprintln!("Error: no file formatted, so there is nothing to report.");
            return Err(CliError::Failed);
        }
        let visits: u64 = census
            .languages
            .iter()
            .flat_map(|l| l.families)
            .map(|f| f.visits)
            .sum();
        let probed: u64 = census
            .languages
            .iter()
            .zip(LANGUAGES)
            .filter(|(_, (_, applicable))| *applicable)
            .map(|(l, _)| l.files)
            .sum();
        if probed > 0 && visits == 0 {
            eprintln!(
                "Error: {probed} TypeScript or Svelte file(s) formatted and no visit was \
                 counted: a comment-only or markup-only corpus, or a build site that stopped \
                 reporting. Either way there is nothing to report."
            );
            return Err(CliError::Failed);
        }

        if self.json {
            print_json(&census, skipped);
        } else {
            print_table(&census, skipped);
        }
        Ok(())
    }
}

fn print_table(census: &Census, skipped: usize) {
    let family_w = FAMILIES.iter().map(|f| f.len()).max().unwrap_or(0);
    println!(
        "{:<10}  {:<family_w$}  {:>12}  {:>12}  {:>7}",
        "language", "family", "visits", "distinct", "ratio"
    );
    for (totals, (language, applicable)) in census.languages.iter().zip(LANGUAGES) {
        for (family, count) in FAMILIES.iter().zip(totals.families) {
            if applicable {
                println!(
                    "{:<10}  {:<family_w$}  {:>12}  {:>12}  {:>7.3}",
                    language,
                    family,
                    count.visits,
                    count.distinct,
                    count.ratio()
                );
            } else {
                println!(
                    "{:<10}  {:<family_w$}  {:>12}  {:>12}  {:>7}",
                    language, family, "n/a", "n/a", "n/a"
                );
            }
        }
    }
    println!();
    for (totals, (language, _)) in census.languages.iter().zip(LANGUAGES) {
        println!(
            "{:<10}  {} files, {} source bytes, {} doc walks, {} rejected",
            language, totals.files, totals.source_bytes, totals.doc_walks, totals.rejected
        );
    }
    println!();
    println!(
        "A visit is one doc-build dispatch, not an AST edge read (dispatches not seen: the \
         command's module doc); {skipped} invalid-fixture skips."
    );
}

fn print_json(census: &Census, skipped: usize) {
    let languages: Vec<_> = census
        .languages
        .iter()
        .zip(LANGUAGES)
        .map(|(totals, (language, applicable))| {
            let families = applicable.then(|| {
                FAMILIES
                    .iter()
                    .zip(totals.families)
                    .map(|(family, count)| {
                        serde_json::json!({
                            "family": family,
                            "visits": count.visits,
                            "distinct": count.distinct,
                            "ratio": count.ratio(),
                        })
                    })
                    .collect::<Vec<_>>()
            });
            serde_json::json!({
                "language": language,
                "applicable": applicable,
                "files": totals.files,
                "rejected": totals.rejected,
                "source_bytes": totals.source_bytes,
                "doc_walks": totals.doc_walks,
                "families": families,
            })
        })
        .collect();
    super::print_json_pretty(&serde_json::json!({
        "languages": languages,
        "invalid_fixture_skips": skipped,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsv_lang::fanout_probe::BuildKey;

    fn key(dispatch: Build, start: u32) -> BuildKey {
        BuildKey {
            dispatch,
            kind: 0,
            start,
            end: start + 1,
        }
    }

    /// A node's builds sum into visits and the node itself is one distinct, per family.
    #[test]
    fn a_tally_folds_into_its_families() {
        let mut totals = LanguageTotals::default();
        totals.add_tally(&Tally {
            builds: vec![
                (key(Build::Expression, 0), 3),
                (key(Build::Expression, 2), 1),
                (key(Build::Type, 4), 2),
                (key(Build::Statement, 6), 1),
            ],
            leaf_builds: vec![(key(Build::Expression, 8), 5)],
            walks: 7,
            ..Tally::default()
        });
        let read = |i: usize| (totals.families[i].visits, totals.families[i].distinct);
        assert_eq!(read(EXPRESSION_NONLEAF), (4, 2));
        assert_eq!(read(EXPRESSION_LEAF), (5, 1));
        assert_eq!(read(TYPE), (2, 1));
        assert_eq!(read(STATEMENT), (1, 1));
        assert_eq!(totals.doc_walks, 7);
    }

    #[test]
    fn a_family_with_no_distinct_nodes_reports_a_zero_ratio() {
        assert!(FamilyCount::default().ratio().abs() < f64::EPSILON);
    }
}
