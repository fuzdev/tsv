//! Parser allocation demand over a corpus: how many nodes of each arena-allocated type a
//! parse builds, per source byte, and how much of it the finished tree keeps.
//!
//! [`super::ast_census`] walks the wire, so it counts survivors. This counts what the parser
//! allocates (`tsv_lang::ast_census_probe`, feature `_internal_ast_census`), which is what an
//! AST held in per-type stores (one contiguous array per node type, addressed by index) must
//! be sized for: the speculative parses a rewind abandons and the nodes the cover grammar and
//! the by-value expression boundary strand are demand, and no walk of the finished tree can
//! see them.
//!
//! # What a row is
//!
//! One row per element type a parser placed in its arena, named by that type: a single
//! allocation counts one, a flushed list counts its length. A list of references reads
//! `&T (ids)` and an `Option` of one `Option<&T> (ids)`; those are id lists, not rows of
//! `T`, whose nodes are counted where they were allocated. A pattern is an `Expression` row,
//! except an array pattern's elements, which are a by-value list and read `Option<Expression>`
//! (a hole is a row there, and an element is not an `Expression` row).
//!
//! - `minted`: every row placed, whatever became of it.
//! - `abandoned`: minted inside a parse a rewind threw away (TypeScript's checkpoint
//!   rewinds and whole-parse retry, Svelte's each-head re-parse and type-extent scan, CSS's
//!   prelude and selector retries).
//! - `dup_dead`: rows stranded by the by-value clone class whose copy was allocated again:
//!   an expression copied out of the arena and placed elsewhere, its original then
//!   unreachable (a computed object key, `{@debug}`'s elements and the sequence they came
//!   from).
//! - `superseded`: rows replaced by a re-allocation (the `<` node's lazy re-allocation, a
//!   cover-grammar literal or list converted to its pattern), passed through (a flattened
//!   instantiation's node), or a wrapper the parse discards (the `function` shell around a
//!   `{#snippet}` head's parameters).
//! - `moved_inline`: rows stranded by a by-value move into a parent's own slot: the node is
//!   alive inside its parent, the arena row is not (a class field's initializer, a
//!   computed class or interface key, an enum initializer, `export default` / `export =`,
//!   a decorator, a `for` head, an array pattern's unchanged element). Not waste; a by-value
//!   slot that became a store edge would make this a row.
//! - `built_inline`: nodes built by value in a parent's slot and never placed at all (a
//!   non-computed class or interface key, a catch parameter). No row, counted beside
//!   `moved_inline` so a by-value slot's demand is visible.
//! - `live`: `minted - abandoned - dup_dead - superseded - moved_inline`: the rows the
//!   finished tree reaches through a reference edge or a list. A node held by value in a
//!   parent is not in it; `moved_inline` and `built_inline` count those. A JSDoc cast's
//!   `CommentBoundPair` wrapper is live with no wire object of its own, so `Expression` reads
//!   above the wire's count. Derived, so the survivor side is the `wire` column, below.
//! - `dup_kept`: minted rows that copy another row with both reachable (a shorthand
//!   property's key and value). Informational, inside `live`.
//! - `scratch_peak`: the largest total of open list prefixes of this type at any moment,
//!   maximum over files, and the same per source byte (summed per-file peaks over bytes).
//! - `wire` / `delta`: the wire's count of the node named like the row, when the wire has
//!   one, and `live - wire`. Rows with no wire name (`Expression`, `Statement`, `TSType`,
//!   `ObjectProperty`, the id lists) leave it blank, and a row whose wire name is shared by a
//!   node the row does not store reads a wider gap; `ast_census --slots` reaches those
//!   through their fields.
//!
//! # Denominators
//!
//! A per-byte column divides by the bytes of the documents that can hold the row: `tsv_ts`
//! rows by TypeScript and JavaScript files plus Svelte documents, `tsv_css` rows by CSS
//! files plus Svelte documents, `tsv_svelte` rows by Svelte documents alone, anything else
//! by all of them. A Svelte document's bytes are its whole text, its embedded script and
//! style included, so the TypeScript and CSS denominators are an upper bound on the bytes
//! those languages' rows actually came from.
//!
//! # What this does not produce, and its blind spots
//!
//! Capacity and growth-copy bytes depend on a store's pre-size and reuse policy, neither of
//! which exists yet. A doubling simulation over this demand would report a policy nobody
//! chose, so it is left out.
//!
//! Waste is claimed where a reading of the parsers found a node strand; a site nothing
//! claims shows up as `live` above the wire. A claim that exceeds a row's minted count in any
//! file fails the run. An abandoned speculative parse that the parser keeps past its own
//! retry (the second acorn-follower parse failing, the first kept) is rewound precisely; a
//! failed selector or prelude read that a caller swallows is rewound at the swallow.
//!
//! Each file parses once at the language's default goal; a file that doesn't parse is
//! counted as rejected and contributes nothing.

use argh::FromArgs;

use crate::cli::CliError;

/// Count the nodes the parsers allocate, per store-backed type, per source byte.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ast_demand_census")]
pub struct AstDemandCensusCommand {
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

impl AstDemandCensusCommand {
    #[cfg(not(feature = "_internal_ast_census"))]
    pub(crate) fn run(self) -> Result<(), CliError> {
        let _ = (self.json, self.jobs, self.paths);
        eprintln!(
            "Error: ast_demand_census needs the parser's demand counters: build with `--features \
             _internal_ast_census`. Without them every count would read zero."
        );
        Err(CliError::Failed)
    }

    #[cfg(feature = "_internal_ast_census")]
    pub(crate) fn run(self) -> Result<(), CliError> {
        census::run(&self.paths, self.jobs, self.json)
    }
}

#[cfg(feature = "_internal_ast_census")]
mod census {
    use std::collections::{BTreeMap, HashMap};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::Path;

    use serde_json::Value;
    use tsv_cli::cli::input::ParserType;
    use tsv_lang::ast_census_probe::{self, Row, Tally};

    use crate::audit::panic_hook::panic_message;
    use crate::audit::parallel::run_pool;
    use crate::audit::properties::tsv_parse_to_value;
    use crate::cli::CliError;
    use crate::cli::commands::profile::{pct, resolve_profile_files};

    /// A row's store: the crate its element type lives in and how it reads in the report.
    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(super) struct StoreKey {
        pub(super) krate: String,
        pub(super) label: String,
    }

    /// The crate and last path segment of a plain type name (`a::b::C`).
    fn crate_and_tail(path: &str) -> (&str, &str) {
        let krate = path.split("::").next().unwrap_or(path);
        let tail = path.rsplit("::").next().unwrap_or(path);
        (krate, tail)
    }

    /// `type_name`'s spelling of an element type, as the report reads it: a reference is an
    /// id list's element (`&T (ids)`), an `Option` of one is `Option<&T> (ids)`, and an
    /// `Option` of a value keeps its element. Anything else (a tuple, `str`) stays as printed.
    pub(super) fn store_key(type_name: &str) -> StoreKey {
        // `type_name` prints a type's lifetime arguments as `<'_>`
        let type_name = type_name.replace("<'_>", "");
        let type_name = type_name.as_str();
        let key = |krate: &str, label: String| StoreKey {
            krate: krate.to_string(),
            label,
        };
        if let Some(inner) = type_name
            .strip_prefix("core::option::Option<&")
            .and_then(|s| s.strip_suffix('>'))
        {
            let (krate, tail) = crate_and_tail(inner);
            return key(krate, format!("Option<&{tail}> (ids)"));
        }
        if let Some(inner) = type_name
            .strip_prefix("core::option::Option<")
            .and_then(|s| s.strip_suffix('>'))
        {
            let (krate, tail) = crate_and_tail(inner);
            return key(krate, format!("Option<{tail}>"));
        }
        if let Some(inner) = type_name.strip_prefix('&') {
            let (krate, tail) = crate_and_tail(inner);
            return key(krate, format!("&{tail} (ids)"));
        }
        let (krate, tail) = crate_and_tail(type_name);
        key(krate, tail.to_string())
    }

    #[derive(Clone, Copy, Default)]
    pub(super) struct Sums {
        width: u64,
        pub(super) minted: u64,
        pub(super) abandoned: u64,
        pub(super) duplicate_dead: u64,
        pub(super) superseded: u64,
        pub(super) duplicate_kept: u64,
        pub(super) moved_inline: u64,
        pub(super) built_inline: u64,
        scratch_peak_max: u64,
        scratch_peak_sum: u64,
    }

    impl Sums {
        /// Everything the row's claims remove from `minted`.
        fn claimed(self) -> u64 {
            self.abandoned + self.duplicate_dead + self.superseded + self.moved_inline
        }

        /// `minted` less every claim. Claims are checked against `minted` per file
        /// ([`Census::add_tally`]), so this cannot underflow in a run that reports.
        pub(super) fn live(self) -> u64 {
            self.minted - self.claimed()
        }

        #[cfg(test)]
        pub(super) fn scratch_peak_max_for_test(self) -> u64 {
            self.scratch_peak_max
        }

        fn add(&mut self, row: Row) {
            self.width = self.width.max(row.width as u64);
            self.minted += row.minted;
            self.abandoned += row.abandoned;
            self.duplicate_dead += row.duplicate_dead;
            self.superseded += row.superseded;
            self.duplicate_kept += row.duplicate_kept;
            self.moved_inline += row.moved_inline;
            self.built_inline += row.built_inline;
            self.scratch_peak_max = self.scratch_peak_max.max(row.scratch_peak);
            self.scratch_peak_sum += row.scratch_peak;
        }

        fn merge(&mut self, other: Self) {
            self.width = self.width.max(other.width);
            self.minted += other.minted;
            self.abandoned += other.abandoned;
            self.duplicate_dead += other.duplicate_dead;
            self.superseded += other.superseded;
            self.duplicate_kept += other.duplicate_kept;
            self.moved_inline += other.moved_inline;
            self.built_inline += other.built_inline;
            self.scratch_peak_max = self.scratch_peak_max.max(other.scratch_peak_max);
            self.scratch_peak_sum += other.scratch_peak_sum;
        }
    }

    #[derive(Default)]
    pub(super) struct Census {
        pub(super) files: u64,
        pub(super) rejected: u64,
        pub(super) ts_bytes: u64,
        pub(super) css_bytes: u64,
        pub(super) svelte_bytes: u64,
        pub(super) stores: BTreeMap<StoreKey, Sums>,
        pub(super) wire: HashMap<String, u64>,
        panics: Vec<String>,
        over_claims: Vec<String>,
    }

    impl Census {
        pub(super) fn source_bytes(&self) -> u64 {
            self.ts_bytes + self.css_bytes + self.svelte_bytes
        }

        /// The bytes of the documents that can hold a row of `krate` (module doc).
        pub(super) fn denominator(&self, krate: &str) -> u64 {
            match krate {
                "tsv_ts" => self.ts_bytes + self.svelte_bytes,
                "tsv_css" => self.css_bytes + self.svelte_bytes,
                "tsv_svelte" => self.svelte_bytes,
                _ => self.source_bytes(),
            }
        }

        #[cfg(test)]
        pub(super) fn has_over_claims(&self) -> bool {
            !self.over_claims.is_empty()
        }

        pub(super) fn add_tally(&mut self, path: &str, tally: &Tally) {
            for (name, row) in &tally.rows {
                let mut sums = Sums::default();
                sums.add(*row);
                if sums.claimed() > sums.minted {
                    self.over_claims.push(format!(
                        "{path}: {name}: {} claimed of {} minted",
                        sums.claimed(),
                        sums.minted
                    ));
                }
                self.stores.entry(store_key(name)).or_default().add(*row);
            }
        }

        fn merge(&mut self, other: Self) {
            self.files += other.files;
            self.rejected += other.rejected;
            self.ts_bytes += other.ts_bytes;
            self.css_bytes += other.css_bytes;
            self.svelte_bytes += other.svelte_bytes;
            for (key, sums) in other.stores {
                self.stores.entry(key).or_default().merge(sums);
            }
            for (name, n) in other.wire {
                *self.wire.entry(name).or_default() += n;
            }
            self.panics.extend(other.panics);
            self.over_claims.extend(other.over_claims);
        }
    }

    /// The internal parse, on this thread: `true` when it parsed.
    pub(super) fn parse_once(source: &str, parser: ParserType) -> bool {
        let arena = bumpalo::Bump::new();
        match parser {
            ParserType::TypeScript => tsv_ts::__private::parse(source, &arena).is_ok(),
            ParserType::Svelte => tsv_svelte::__private::parse(source, &arena).is_ok(),
            ParserType::Css => tsv_css::__private::parse(source, &arena).is_ok(),
        }
    }

    /// Count every object carrying a `"type"` string, by that string.
    pub(super) fn walk_wire(value: &Value, counts: &mut HashMap<String, u64>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(kind)) = map.get("type") {
                    *counts.entry(kind.clone()).or_default() += 1;
                }
                for child in map.values() {
                    walk_wire(child, counts);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk_wire(item, counts);
                }
            }
            _ => {}
        }
    }

    fn census_file(path: &Path, census: &mut Census) {
        let Ok(source) = std::fs::read_to_string(path) else {
            census.rejected += 1;
            return;
        };
        let parser = ParserType::from_extension(&path.to_string_lossy());
        let result = catch_unwind(AssertUnwindSafe(|| {
            ast_census_probe::arm();
            let parsed = parse_once(&source, parser);
            (parsed, ast_census_probe::disarm())
        }));
        match result {
            Ok((true, tally)) => {
                census.files += 1;
                let bytes = source.len() as u64;
                match parser {
                    ParserType::TypeScript => census.ts_bytes += bytes,
                    ParserType::Css => census.css_bytes += bytes,
                    ParserType::Svelte => census.svelte_bytes += bytes,
                }
                census.add_tally(&path.to_string_lossy(), &tally);
                // the survivor side, on a fresh parse outside the armed bracket
                if let Some(value) = tsv_parse_to_value(&source, parser) {
                    walk_wire(&value, &mut census.wire);
                }
            }
            Ok((false, _)) => census.rejected += 1,
            Err(payload) => {
                ast_census_probe::disarm();
                census
                    .panics
                    .push(format!("{}: {}", path.display(), panic_message(&*payload)));
            }
        }
    }

    pub(super) fn run(paths: &[String], jobs: Option<usize>, json: bool) -> Result<(), CliError> {
        let (files, skipped) = resolve_profile_files(paths, |_| true)?;
        let mut census = run_pool(
            &files,
            jobs,
            |path, census: &mut Census| census_file(path, census),
            Census::merge,
        )?;
        if !census.panics.is_empty() {
            census.panics.sort_unstable();
            for line in &census.panics {
                eprintln!("panic: {line}");
            }
            eprintln!(
                "Error: {} file(s) panicked in parse; a census missing them is not a total.",
                census.panics.len()
            );
            return Err(CliError::Failed);
        }
        if !census.over_claims.is_empty() {
            census.over_claims.sort_unstable();
            for line in &census.over_claims {
                eprintln!("over-claim: {line}");
            }
            eprintln!(
                "Error: {} row(s) claim more waste than they minted; a waste claim site is wrong.",
                census.over_claims.len()
            );
            return Err(CliError::Failed);
        }
        let minted: u64 = census.stores.values().map(|s| s.minted).sum();
        if census.files == 0 || minted == 0 {
            eprintln!(
                "Error: {} file(s) parsed and {minted} row(s) minted: there is nothing to report.",
                census.files
            );
            return Err(CliError::Failed);
        }
        if json {
            print_json(&census, skipped);
        } else {
            print_table(&census, skipped);
        }
        Ok(())
    }

    #[expect(clippy::cast_precision_loss)]
    fn per_byte(count: u64, bytes: u64) -> f64 {
        count as f64 / bytes as f64
    }

    fn wire_of<'a>(census: &'a Census, key: &StoreKey) -> Option<&'a u64> {
        census.wire.get(&key.label)
    }

    /// Stores with the most minted first, ties by name, so a re-run reads the same.
    fn ranked(census: &Census) -> Vec<(&StoreKey, Sums)> {
        let mut rows: Vec<_> = census.stores.iter().map(|(k, s)| (k, *s)).collect();
        rows.sort_by(|a, b| b.1.minted.cmp(&a.1.minted).then_with(|| a.0.cmp(b.0)));
        rows
    }

    fn print_table(census: &Census, skipped: usize) {
        let label_w = census
            .stores
            .keys()
            .map(|k| k.label.len())
            .max()
            .unwrap_or(0)
            .max("store".len());
        println!(
            "{:<10}  {:<label_w$}  {:>5}  {:>10}  {:>8}  {:>10}  {:>9}  {:>9}  {:>9}  {:>9}  \
             {:>9}  {:>9}  {:>8}  {:>8}  {:>10}  {:>9}",
            "crate",
            "store",
            "width",
            "minted",
            "minted/B",
            "live",
            "abandoned",
            "dup_dead",
            "supersede",
            "moved_in",
            "built_in",
            "dup_kept",
            "peak_max",
            "peak/KB",
            "wire",
            "delta"
        );
        for (key, sums) in ranked(census) {
            let bytes = census.denominator(&key.krate).max(1);
            let wire = wire_of(census, key);
            let delta = wire.map(|&wire| sums.live().cast_signed() - wire.cast_signed());
            println!(
                "{:<10}  {:<label_w$}  {:>5}  {:>10}  {:>8.4}  {:>10}  {:>9}  {:>9}  {:>9}  \
                 {:>9}  {:>9}  {:>9}  {:>8}  {:>8.3}  {:>10}  {:>9}",
                key.krate,
                key.label,
                sums.width,
                sums.minted,
                per_byte(sums.minted, bytes),
                sums.live(),
                sums.abandoned,
                sums.duplicate_dead,
                sums.superseded,
                sums.moved_inline,
                sums.built_inline,
                sums.duplicate_kept,
                sums.scratch_peak_max,
                per_byte(sums.scratch_peak_sum, bytes) * 1024.0,
                wire.map_or_else(|| "-".to_string(), ToString::to_string),
                delta.map_or_else(|| "-".to_string(), |d| d.to_string()),
            );
        }
        let (minted, live) = census
            .stores
            .values()
            .fold((0, 0), |(m, l), s| (m + s.minted, l + s.live()));
        println!();
        println!(
            "{} files, {} source bytes ({} TS/JS, {} CSS, {} Svelte), {} rejected; {minted} rows \
             minted, {live} live ({:.1}%), {skipped} invalid-fixture skips",
            census.files,
            census.source_bytes(),
            census.ts_bytes,
            census.css_bytes,
            census.svelte_bytes,
            census.rejected,
            pct(live as usize, minted as usize)
        );
        println!(
            "Per-byte columns: tsv_ts over TS/JS + Svelte documents, tsv_css over CSS + Svelte \
             documents, tsv_svelte over Svelte documents (embedded script and style are inside \
             a Svelte document's bytes). `live` is minted minus claims, not a tree walk; `wire` \
             / `delta` are the survivor side where the wire names the row. Capacity and \
             growth-copy bytes are not reported: no store exists to measure."
        );
    }

    fn print_json(census: &Census, skipped: usize) {
        let rows: Vec<_> = ranked(census)
            .iter()
            .map(|(key, sums)| {
                let bytes = census.denominator(&key.krate).max(1);
                serde_json::json!({
                    "crate": key.krate,
                    "store": key.label,
                    "width": sums.width,
                    "minted": sums.minted,
                    "denominator_bytes": bytes,
                    "minted_per_byte": per_byte(sums.minted, bytes),
                    "live": sums.live(),
                    "abandoned": sums.abandoned,
                    "duplicate_dead": sums.duplicate_dead,
                    "superseded": sums.superseded,
                    "duplicate_kept": sums.duplicate_kept,
                    "moved_inline": sums.moved_inline,
                    "built_inline": sums.built_inline,
                    "scratch_peak_max": sums.scratch_peak_max,
                    "scratch_peak_per_byte": per_byte(sums.scratch_peak_sum, bytes),
                    "wire": wire_of(census, key),
                })
            })
            .collect();
        super::super::print_json_pretty(&serde_json::json!({
            "files": census.files,
            "source_bytes": census.source_bytes(),
            "ts_bytes": census.ts_bytes,
            "css_bytes": census.css_bytes,
            "svelte_bytes": census.svelte_bytes,
            "rejected": census.rejected,
            "invalid_fixture_skips": skipped,
            "stores": rows,
        }));
    }
}

#[cfg(all(test, feature = "_internal_ast_census"))]
mod tests {
    use super::census::{Census, StoreKey, parse_once, store_key};
    use tsv_cli::cli::input::ParserType;
    use tsv_lang::ast_census_probe;

    fn census_of(source: &str, parser: ParserType) -> Census {
        ast_census_probe::arm();
        assert!(parse_once(source, parser), "the fixture must parse");
        let mut census = Census::default();
        census.add_tally("test", &ast_census_probe::disarm());
        census
    }

    #[expect(clippy::panic)]
    fn row(census: &Census, krate: &str, label: &str) -> super::census::Sums {
        census
            .stores
            .get(&StoreKey {
                krate: krate.to_string(),
                label: label.to_string(),
            })
            .copied()
            .unwrap_or_else(|| panic!("no row {krate} {label}: {:?}", census.stores.keys()))
    }

    /// `[a] = x;`: the array parses as a literal (`a` and the array itself are expressions,
    /// the element list a one-row id list), then the `=` converts it: the pattern's new row
    /// replaces the array, a new list replaces the id list, and `a` moves into that by-value
    /// `Option<Expression>` list, stranding its row. The right side and the assignment bring
    /// the minted total to five, three of them live.
    #[test]
    fn a_cover_grammar_orphan_is_minted_and_not_live() {
        let census = census_of("[a] = x;", ParserType::TypeScript);
        let expr = row(&census, "tsv_ts", "Expression");
        assert_eq!(
            expr.minted, 5,
            "a, the array, x, the pattern's row, the assignment"
        );
        assert_eq!(expr.duplicate_dead, 0);
        assert_eq!(expr.moved_inline, 1, "`a`");
        assert_eq!(expr.superseded, 1, "the array");
        assert_eq!(expr.live(), 3, "x, the pattern's row, the assignment");
        let ids = row(&census, "tsv_ts", "Option<&Expression> (ids)");
        assert_eq!(
            (ids.minted, ids.superseded),
            (1, 1),
            "the literal's element list"
        );
        let values = row(&census, "tsv_ts", "Option<Expression>");
        assert_eq!(values.minted, 1, "`a` in the pattern's by-value elements");
        assert_eq!(row(&census, "tsv_ts", "Statement").minted, 1);
    }

    /// `x = y;`, and `({ a: b, ...c } = z);`: a target that does not convert keeps the row it
    /// was parsed into, and so does a destructuring leaf. Only the object literal is replaced.
    #[test]
    fn an_unconverted_target_keeps_its_row() {
        let census = census_of("x = y;", ParserType::TypeScript);
        let expr = row(&census, "tsv_ts", "Expression");
        assert_eq!(expr.minted, 3, "x, y, the assignment");
        assert_eq!(
            (expr.duplicate_dead, expr.superseded),
            (0, 0),
            "x is the target as parsed"
        );

        let census = census_of("({ a: b, ...c } = z);", ParserType::TypeScript);
        let expr = row(&census, "tsv_ts", "Expression");
        assert_eq!(expr.duplicate_dead, 0, "b and c are the pattern's leaves");
        assert_eq!(expr.superseded, 1, "the object literal");
        let properties = row(&census, "tsv_ts", "ObjectProperty");
        assert_eq!(properties.superseded, 2, "the literal's property list");
    }

    /// `a ? (b): c => d;`: the consequent `(b): c => d` is tried as an annotated arrow and
    /// rewound, since no `:` follows its body (a consequent's arrow must be followed by the
    /// conditional's `:`), so everything it built is abandoned and the head re-reads as a
    /// parenthesized `b`. Live: `a`, `b`, the alternate's arrow (its row and its by-value
    /// parameter `c`), its body `d`, the conditional — six rows of `Expression`.
    #[test]
    fn a_rewound_speculation_is_abandoned_and_not_live() {
        // the speculated parameter `b`, body `d` and the arrow's own `Expression` row
        const ABANDONED_EXPRS: u64 = 3;
        let census = census_of("a ? (b): c => d;", ParserType::TypeScript);
        let expr = row(&census, "tsv_ts", "Expression");
        assert_eq!(expr.live(), 6, "minted rows: {:?}", expr.minted);
        assert_eq!(
            expr.abandoned, ABANDONED_EXPRS,
            "the speculated arrow's rows"
        );
        assert_eq!(expr.minted, expr.live() + expr.abandoned);
        assert_eq!(
            row(&census, "tsv_ts", "ArrowFunctionExpression").abandoned,
            1
        );
        assert_eq!(row(&census, "tsv_ts", "ArrowFunctionExpression").live(), 1);
    }

    #[test]
    fn a_reference_list_row_names_its_element_as_ids() {
        let key = store_key("&tsv_ts::ast::expressions::Expression");
        assert_eq!(key.label, "&Expression (ids)");
        assert_eq!(key.krate, "tsv_ts");
        let opt = store_key("core::option::Option<&tsv_ts::ast::expressions::Expression>");
        assert_eq!(opt.label, "Option<&Expression> (ids)");
        let value = store_key("core::option::Option<tsv_ts::ast::expressions::Expression>");
        assert_eq!(value.label, "Option<Expression>");
        let plain = store_key("tsv_ts::ast::statements::Statement");
        assert_eq!(
            (plain.krate.as_str(), plain.label.as_str()),
            ("tsv_ts", "Statement")
        );
    }

    /// Scratch counts the longest total of open lists, not the longest list: a nested
    /// statement list's peak sits on top of the outer list's live prefix.
    #[test]
    fn scratch_counts_the_outer_prefix_under_a_nested_list() {
        let census = census_of("a; b; { c; d; }", ParserType::TypeScript);
        let stmts = row(&census, "tsv_ts", "Statement");
        assert_eq!(stmts.minted, 5, "two, the block, its two");
        // two statements pushed, then the inner block's two on top of them: the block's own
        // `Statement` is pushed only after its inner list flushes
        assert_eq!(stmts.scratch_peak_max_for_test(), 4);
    }

    #[test]
    fn a_run_over_no_files_fails_loudly() {
        let run = super::census::run(&[], None, false);
        assert!(run.is_err());
    }

    /// A corpus of a file that doesn't parse has nothing to report.
    #[test]
    fn a_run_over_only_rejected_files_fails_loudly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.ts");
        std::fs::write(&path, "class {").unwrap();
        let run = super::census::run(&[path.to_string_lossy().into_owned()], Some(1), false);
        assert!(run.is_err());
    }

    /// CSS and Svelte reach their rows the same way: one rule's selector and declaration,
    /// one element holding one expression.
    #[test]
    fn css_and_svelte_lists_count_their_rows() {
        let census = census_of("a { color: red; }", ParserType::Css);
        assert_eq!(
            row(&census, "tsv_css", "CssBlockChild").minted,
            1,
            "the declaration"
        );
        assert_eq!(row(&census, "tsv_css", "CssNode").minted, 1, "the rule");
        let census = census_of("<p>{a}</p>", ParserType::Svelte);
        assert_eq!(row(&census, "tsv_ts", "Expression").live(), 1, "`a`");
        assert_eq!(
            row(&census, "tsv_svelte", "FragmentNode").minted,
            2,
            "<p> and {{a}}"
        );
    }

    /// `class A { x = 1 }`: the initializer `1` is minted once, then moved by value into its
    /// field, so its row is stranded but its node is alive; the key `x` is built by value and
    /// never placed. Neither is waste.
    #[test]
    fn a_by_value_holder_is_moved_inline_not_waste() {
        let census = census_of("class A { x = 1 }", ParserType::TypeScript);
        let expr = row(&census, "tsv_ts", "Expression");
        assert_eq!(expr.minted, 1, "the initializer `1`");
        assert_eq!(expr.moved_inline, 1, "the initializer, into its field");
        assert_eq!(expr.built_inline, 1, "the key `x`");
        assert_eq!(expr.live(), 0, "the tree holds it by value");
        assert_eq!(expr.duplicate_dead, 0);
    }

    /// A computed object key clones out of the arena and is allocated again: a duplicate
    /// root, not a by-value holder.
    #[test]
    fn a_computed_object_key_is_a_duplicate_root() {
        let census = census_of("({ [k]: 1 });", ParserType::TypeScript);
        let expr = row(&census, "tsv_ts", "Expression");
        assert_eq!(expr.moved_inline, 0);
        assert_eq!(expr.duplicate_dead, 1, "the key's original");
    }

    /// A prelude that is no condition is re-read raw: everything the condition reader
    /// listed is abandoned.
    #[test]
    fn a_reconsumed_prelude_abandons_the_condition_read() {
        let census = census_of("@supports (a: b) foo {}", ParserType::Css);
        assert_eq!(
            census.stores.values().map(|s| s.abandoned).sum::<u64>(),
            census
                .stores
                .iter()
                .filter(|(key, _)| key.label.starts_with("Condition"))
                .map(|(_, s)| s.minted)
                .sum::<u64>(),
            "{:?}",
            census.stores.keys().collect::<Vec<_>>(),
        );
        assert!(census.stores.values().map(|s| s.minted).sum::<u64>() > 0);
    }

    /// `{#each xs as x}`: the type-extent probe on the first `as` parses `x` as a type and
    /// discards it, so the `TSType` row it built is abandoned.
    #[test]
    fn the_each_head_type_extent_is_abandoned() {
        let census = census_of("{#each xs as x}{/each}", ParserType::Svelte);
        let types = row(&census, "tsv_ts", "TSType");
        assert_eq!((types.minted, types.abandoned), (1, 1));
    }

    /// The `{#snippet}` head's `function` shell is claimed; its parameter stays live.
    #[test]
    fn a_snippet_wrapper_shell_is_superseded() {
        let census = census_of("{#snippet s(a)}{/snippet}", ParserType::Svelte);
        assert_eq!(row(&census, "tsv_ts", "Statement").superseded, 1);
        assert_eq!(row(&census, "tsv_ts", "Statement").live(), 0);
        assert_eq!(row(&census, "tsv_ts", "FunctionDeclaration").live(), 0);
    }

    /// A per-byte denominator names the documents that can hold the row.
    #[test]
    fn a_row_divides_by_its_languages_documents() {
        let mut census = Census::default();
        census.ts_bytes = 10;
        census.css_bytes = 20;
        census.svelte_bytes = 40;
        assert_eq!(census.denominator("tsv_ts"), 50);
        assert_eq!(census.denominator("tsv_css"), 60);
        assert_eq!(census.denominator("tsv_svelte"), 40);
        assert_eq!(census.denominator("tsv_lang"), 70);
    }

    /// A waste claim past what a row minted fails the run.
    #[test]
    fn an_over_claim_fails_loudly() {
        ast_census_probe::arm();
        ast_census_probe::mint::<u8>(1);
        ast_census_probe::dead::<u8>(ast_census_probe::Dead::DuplicateRoot, 2);
        let mut census = Census::default();
        census.add_tally("test", &ast_census_probe::disarm());
        assert!(census.has_over_claims());
    }
}
