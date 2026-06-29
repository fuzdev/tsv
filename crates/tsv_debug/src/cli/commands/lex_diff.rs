use crate::cli::CliError;
use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Differential lexer harness — snapshot the raw token stream over a corpus and
/// diff it against a golden. Capture a golden from the current lexer (`--write`),
/// then after a lexer change re-run (`--check`) to prove token-stream identity:
/// same `(kind, start, end,
/// decoded)` for every token of every file. Stronger than format byte-identity — it
/// catches token-level divergence the formatter might absorb. Pure Rust, no Deno.
///
/// Covers the **context-free** `next_token` dispatch only (a raw `next_token` loop
/// doesn't reach the parser-driven regex / template-resume paths); those stay gated
/// by the fixture + corpus format byte-identity suites.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "lex_diff")]
pub struct LexDiffCommand {
    /// files/dirs to lex (recursive; `.ts` / `.mts` / `.cts` / `.svelte.ts`)
    #[argh(positional)]
    paths: Vec<PathBuf>,

    /// golden snapshot file: with `--write` it is written, otherwise it is the
    /// reference to check the current lexer against
    #[argh(option)]
    golden: Option<PathBuf>,

    /// write the golden instead of checking against it
    #[argh(switch)]
    write: bool,

    /// on a check mismatch, print the first divergent line per file
    #[argh(switch)]
    verbose: bool,
}

const SECTION: &str = "@@@ ";

impl LexDiffCommand {
    pub fn run(&self) -> Result<(), CliError> {
        if self.paths.is_empty() {
            eprintln!("lex_diff: no paths given");
            return Err(CliError::Failed);
        }
        let mut files = Vec::new();
        for p in &self.paths {
            collect_ts_files(p, &mut files);
        }
        files.sort();
        files.dedup();
        if files.is_empty() {
            eprintln!("lex_diff: no .ts files found under {:?}", self.paths);
            return Err(CliError::Failed);
        }

        // Build the current token-stream map (path → stream).
        let mut streams: BTreeMap<String, String> = BTreeMap::new();
        for f in &files {
            let key = f.to_string_lossy().into_owned();
            match std::fs::read_to_string(f) {
                Ok(src) => {
                    streams.insert(key, tsv_ts::debug_token_stream(&src));
                }
                Err(e) => {
                    eprintln!("lex_diff: skip {key}: {e}");
                }
            }
        }

        match (&self.golden, self.write) {
            (Some(path), true) => {
                let serialized = serialize_golden(&streams);
                if let Err(e) = std::fs::write(path, serialized) {
                    eprintln!("lex_diff: write {path:?}: {e}");
                    return Err(CliError::Failed);
                }
                let tokens: usize = streams.values().map(|s| s.lines().count()).sum();
                println!(
                    "lex_diff: wrote golden {path:?} — {} files, {tokens} token lines",
                    streams.len()
                );
                Ok(())
            }
            (Some(path), false) => {
                let golden_text = match std::fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("lex_diff: read golden {path:?}: {e}");
                        return Err(CliError::Failed);
                    }
                };
                let golden = parse_golden(&golden_text);
                self.check(&streams, &golden)
            }
            (None, _) => {
                // No golden: print a summary + a combined stable digest.
                let tokens: usize = streams.values().map(|s| s.lines().count()).sum();
                let digest = combined_digest(&streams);
                println!(
                    "lex_diff: {} files, {tokens} token lines, digest {digest:016x}",
                    streams.len()
                );
                Ok(())
            }
        }
    }

    fn check(
        &self,
        current: &BTreeMap<String, String>,
        golden: &BTreeMap<String, String>,
    ) -> Result<(), CliError> {
        let mut mismatched = 0usize;
        let mut missing = 0usize;
        for (path, cur) in current {
            match golden.get(path) {
                None => {
                    missing += 1;
                    if self.verbose {
                        println!("NEW (not in golden): {path}");
                    }
                }
                Some(want) if want == cur => {}
                Some(want) => {
                    mismatched += 1;
                    println!("MISMATCH: {path}");
                    if self.verbose
                        && let Some((n, w, g)) = first_diff_line(want, cur)
                    {
                        println!("  line {n}:\n    golden: {w}\n    now:    {g}");
                    }
                }
            }
        }
        let golden_only = golden.keys().filter(|k| !current.contains_key(*k)).count();
        println!(
            "lex_diff: {} files checked — {mismatched} mismatched, {missing} new, {golden_only} golden-only",
            current.len()
        );
        if mismatched > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

/// First line (1-indexed) where two streams differ, with both versions.
fn first_diff_line<'a>(golden: &'a str, current: &'a str) -> Option<(usize, &'a str, &'a str)> {
    for (i, (g, c)) in golden.lines().zip(current.lines()).enumerate() {
        if g != c {
            return Some((i + 1, g, c));
        }
    }
    // Same prefix but different length.
    let (gc, cc) = (golden.lines().count(), current.lines().count());
    if gc != cc {
        return Some((gc.min(cc) + 1, "<eof>", "<eof>"));
    }
    None
}

fn serialize_golden(streams: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (path, stream) in streams {
        out.push_str(SECTION);
        out.push_str(path);
        out.push('\n');
        out.push_str(stream);
    }
    out
}

fn parse_golden(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut cur_path: Option<String> = None;
    let mut buf = String::new();
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix(SECTION) {
            if let Some(p) = cur_path.take() {
                map.insert(p, std::mem::take(&mut buf));
            }
            cur_path = Some(rest.trim_end_matches('\n').to_string());
        } else {
            buf.push_str(line);
        }
    }
    if let Some(p) = cur_path {
        map.insert(p, buf);
    }
    map
}

/// Stable FNV-1a-64 over the path+stream pairs (order-independent via BTreeMap).
fn combined_digest(streams: &BTreeMap<String, String>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (path, stream) in streams {
        for b in path.bytes().chain(std::iter::once(0)).chain(stream.bytes()) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Collect `.ts` / `.mts` / `.cts` / `.svelte.ts` files, skipping the usual nests.
fn collect_ts_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if is_ts_file(path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "node_modules" | ".git" | "target" | "dist" | "build") {
                continue;
            }
            collect_ts_files(&p, out);
        } else if is_ts_file(&p) {
            out.push(p);
        }
    }
}

fn is_ts_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    // Any TypeScript source, including `.d.ts` declaration files — they lex like
    // any other `.ts` and are worth diffing.
    name.ends_with(".ts") || name.ends_with(".mts") || name.ends_with(".cts")
}
