//! Audit signature: records a prettier chain, pass by pass, up to its fixed point
//!
//! Some `_prettier_divergence` fixtures involve prettier non-idempotence: formatting again
//! produces a different output (and may take several more passes to stabilize). A signature
//! file pins the exact chain content so:
//!
//! - The `fixtures_audit` tool can recognize these documented multi-pass cases and stop flagging
//!   them as `[novel]`, while still flagging drift if the chain changes byte-for-byte.
//! - The validator byte-equality-checks each chain step on every run, so prettier-version drift
//!   in any intermediate pass surfaces as a fixture failure (stronger than the single-pass checks
//!   alone).
//!
//! ## Two anchors
//!
//! The same file format serves two anchors, distinguished by [`ChainAnchor`]:
//!
//! - [`ChainAnchor::OutputPrettier`] — `audit_signature.txt`, the chain from `output_prettier.*`
//!   (rule F4). Written only when prettier is non-idempotent there; pass 1 *is* `output_prettier`
//!   (already pinned by F2), so the file's own numbering starts at pass 2.
//! - [`ChainAnchor::UnformattedOurs`] — `audit_signature_<suffix>.txt`, the chain from
//!   `unformatted_ours_<suffix>.*` (rule N12), and the **marker of last resort**. Written only
//!   when prettier's first pass from that source matches no documented form ([`SingleFormPins`])
//!   *and* no single-form marker could express it anyway — a chain with two or more distinct
//!   intermediates, or a fixed point tsv cannot ingest. Nothing else pins `prettier(source)`,
//!   so the numbering starts at pass 1.
//!
//! Idempotent fixtures need neither. Validation therefore costs a few extra prettier runs per
//! affected fixture (a couple dozen today), nothing for the rest.
//!
//! ## Intentional overlap with `variant_*` / `prettier_variant_*` files
//!
//! When a fixture has a `variant_X` (or `prettier_variant_X`) file whose content equals
//! `prettier^k(anchor)` for some k, the signature's pass-k entry will duplicate that file's
//! bytes. This is intentional, not a bug:
//!
//! - `N9a` (prettier idempotent on `variant_*`) and the audit's classification both confirm
//!   that the variant file is *self-stable*. Neither check verifies that *the anchor's
//!   prettier-chain actually reaches that variant*. Prettier could in principle drift to a
//!   different stable form that's still variant-shaped, and both N9a and the audit would
//!   stay green.
//! - The byte-exact chain check directly enforces the chain-step transition itself, which
//!   is the property we actually care about for documenting "prettier produces X after k
//!   passes." Drift in that transition surfaces as a fixture failure.
//!
//! The few duplicated bytes (≤1 KB per affected fixture) buy a real check that wasn't
//! otherwise covered.
//!
//! ## File-format invariants
//!
//! - Pass content must end with `\n`. Prettier output always does, so this holds in practice;
//!   round-trip preservation of trailing blank lines relies on it (`strip_trailing_blank`
//!   undoes exactly one separator newline added by `serialize`).
//! - Pass content must not contain `%%PASS=` at column 0. The parser uses that prefix as a
//!   section delimiter. Implausible for valid Svelte/TS/CSS/JS content but worth stating;
//!   a future hostile fixture would trip parsing.

use std::collections::HashSet;

use crate::deno::{PrettierParser, run_prettier};
use crate::fixtures::{Fixture, FixtureFiles, read_file};

/// Filename for the audit signature file, located alongside `output_prettier.*`
pub const AUDIT_SIGNATURE_FILENAME: &str = "audit_signature.txt";

/// Filename prefix for a per-`unformatted_ours_*` signature: `audit_signature_<suffix>.txt`.
///
/// Deliberately not a prefix of [`AUDIT_SIGNATURE_FILENAME`] — the trailing `_` means
/// `"audit_signature.txt".strip_prefix(…)` is `None`, so the two never collide.
const AUDIT_SIGNATURE_VARIANT_PREFIX: &str = "audit_signature_";

/// Extension shared by both signature filenames.
const AUDIT_SIGNATURE_EXTENSION: &str = ".txt";

/// Maximum number of prettier passes to walk before declaring non-convergence.
/// Real cases observed in this repo bottom out at 3 (switch case_block_comment).
pub const MAX_CHAIN_DEPTH: usize = 8;

/// The signature filename pinning `unformatted_ours_<suffix>`'s chain.
pub fn audit_signature_variant_filename(suffix: &str) -> String {
    format!("{AUDIT_SIGNATURE_VARIANT_PREFIX}{suffix}{AUDIT_SIGNATURE_EXTENSION}")
}

/// The `unformatted_ours_*` suffix a signature filename is anchored to, or `None` when the
/// name isn't a per-variant signature. An empty suffix (`audit_signature_.txt`) is rejected —
/// it names no source file, so it belongs in the `unknown` bucket rather than silently
/// matching every source.
pub fn audit_signature_variant_suffix(filename: &str) -> Option<&str> {
    let suffix = filename
        .strip_prefix(AUDIT_SIGNATURE_VARIANT_PREFIX)?
        .strip_suffix(AUDIT_SIGNATURE_EXTENSION)?;
    (!suffix.is_empty()).then_some(suffix)
}

/// What a signature's `%%PASS=N%%` numbering is anchored to.
///
/// The anchor is *not* recorded in the file — it is implied by the filename, and the parser
/// ignores pass numbers entirely (drift detection is byte-exact on content). It exists so
/// `serialize` emits numbering and a header that match how the file will be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainAnchor {
    /// `audit_signature.txt` — the chain from `output_prettier.*` (rule F4). Pass 1 *is*
    /// `output_prettier` itself, already pinned by F2, so the file starts at pass 2.
    OutputPrettier,
    /// `audit_signature_<suffix>.txt` — the chain from `unformatted_ours_<suffix>.*` (rule
    /// N12). Nothing else pins `prettier(source)`, so the file starts at pass 1.
    UnformattedOurs,
}

impl ChainAnchor {
    /// The `%%PASS=N%%` number of the signature's first recorded section.
    pub const fn first_pass_number(self) -> usize {
        match self {
            Self::OutputPrettier => 2,
            Self::UnformattedOurs => 1,
        }
    }

    /// Header lines prepended to a signature file with this anchor.
    const fn header(self) -> &'static str {
        match self {
            Self::OutputPrettier => concat!(
                "# Auto-generated prettier chain signature. Do not edit manually.\n",
                "# Regenerate: deno task fixtures:update:formatted <pattern>\n",
                "#\n",
                "# Each %%PASS=N%% section is exactly prettier^N(output_prettier.<ext>).\n",
                "# This file exists when prettier is non-idempotent on output_prettier.* — it pins\n",
                "# the full chain so the audit recognizes the case and the validator catches drift.\n",
                "# See docs/fixture_overview.md (rule F4) and crates/tsv_debug/src/fixtures/audit_signature.rs.\n",
                "\n",
            ),
            Self::UnformattedOurs => concat!(
                "# Auto-generated prettier chain signature. Do not edit manually.\n",
                "# Regenerate: deno task fixtures:update:formatted <pattern>\n",
                "#\n",
                "# Each %%PASS=N%% section is exactly prettier^N(unformatted_ours_<suffix>.<ext>),\n",
                "# the same-suffix source beside this file. It exists when prettier's output from\n",
                "# that source is claimed by no single-form marker — a chain with two or more\n",
                "# distinct intermediates, or a fixed point no documented stable form holds.\n",
                "# See docs/fixture_overview.md (rule N12) and crates/tsv_debug/src/fixtures/audit_signature.rs.\n",
                "\n",
            ),
        }
    }
}

/// The single-form pins a fixture carries for its `unformatted_ours_*` prettier outputs.
///
/// Two ways an output can already be expressed without a chain: a
/// `prettier_intermediate*_<suffix>` claims the suffix outright (N7/N7b/N7c own the
/// convergence check), or the output equals `input` or one of the fixture's documented
/// stable forms. Collected once and read by three callers — `fixtures_update_formatted`
/// (which writes a chain pin only where no single-form marker can express the output), N12
/// (which refuses a signature a single-form pin already covers), and N10 (which reports what
/// neither covers). One definition means they cannot drift into disagreeing about what
/// "claimed" means.
pub struct SingleFormPins {
    /// Suffixes an intermediate marker claims.
    claimed_suffixes: HashSet<String>,
    /// Contents of `output_prettier.*` / `prettier_variant_*` / `variant_*` /
    /// `divergent_variant_*` — the stable forms an output may land on.
    known_contents: Vec<String>,
}

impl SingleFormPins {
    /// Collect the fixture's single-form pins in one directory read.
    pub fn collect(fixture: &Fixture, input_ext: &str, files: &FixtureFiles) -> Self {
        let fixture_dir = &fixture.path;

        let mut claimed_suffixes: HashSet<String> = HashSet::new();
        let intermediates: [(&Vec<String>, &str); 3] = [
            (&files.prettier_intermediate, "prettier_intermediate_"),
            (
                &files.prettier_intermediate_to_variant,
                "prettier_intermediate_to_variant_",
            ),
            (
                &files.prettier_intermediate_to_divergent_variant,
                "prettier_intermediate_to_divergent_variant_",
            ),
        ];
        for (names, prefix) in intermediates {
            for name in names {
                let suffix = name
                    .strip_prefix(prefix)
                    .and_then(|s| s.strip_suffix(input_ext))
                    .unwrap_or("")
                    .to_string();
                claimed_suffixes.insert(suffix);
            }
        }

        // Read failures are tolerated here without an error: F2/N1/N9a/N11a own these
        // files and report unreadable ones loudly, so a silent skip here can't hide a gap.
        let mut known_contents: Vec<String> = Vec::new();
        let output_prettier_path = fixture.output_prettier_path();
        if output_prettier_path.exists()
            && let Ok(content) = read_file(&output_prettier_path)
        {
            known_contents.push(content);
        }
        // `divergent_variant_*` belongs here too: a prettier-stable form our formatter
        // rewrites is still a documented target for an `unformatted_ours_*` whose prettier
        // output lands on it (e.g. the heritage own-line form).
        for name in files
            .prettier_variant
            .iter()
            .chain(&files.variant)
            .chain(&files.divergent_variant)
        {
            if let Ok(content) = read_file(&fixture_dir.join(name)) {
                known_contents.push(content);
            }
        }

        Self {
            claimed_suffixes,
            known_contents,
        }
    }

    /// Build directly from parts, for tests that exercise the coverage predicate without
    /// a fixture directory on disk.
    #[cfg(test)]
    fn from_parts(claimed_suffixes: &[&str], known_contents: &[&str]) -> Self {
        Self {
            claimed_suffixes: claimed_suffixes.iter().map(|s| (*s).to_string()).collect(),
            known_contents: known_contents.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Does an intermediate marker claim this `unformatted_ours_*` suffix?
    pub fn claims_suffix(&self, suffix: &str) -> bool {
        self.claimed_suffixes.contains(suffix)
    }

    /// Does the fixture document any prettier-stable form at all? When it documents none,
    /// it makes its case by README alone, which is what keeps N10's report informational.
    pub fn has_documented_forms(&self) -> bool {
        !self.known_contents.is_empty()
    }

    /// Does a documented stable form hold these exact bytes?
    pub fn matches_stable_form(&self, prettier_output: &str) -> bool {
        self.known_contents.iter().any(|c| c == prettier_output)
    }

    /// Does a documented stable form — or `input` itself — already hold these bytes?
    ///
    /// Content only — the caller that is itself deciding which intermediate file to write
    /// asks this rather than [`Self::covers`], since the intermediates on disk at scan time
    /// may be the stale ones it is about to remove.
    pub fn matches_documented_form(&self, prettier_output: &str, input: &str) -> bool {
        prettier_output == input || self.matches_stable_form(prettier_output)
    }

    /// Is prettier's output from this source already expressed without a chain pin?
    pub fn covers(&self, suffix: &str, prettier_output: &str, input: &str) -> bool {
        self.claims_suffix(suffix) || self.matches_documented_form(prettier_output, input)
    }
}

/// A captured prettier chain, anchored per [`ChainAnchor`].
///
/// `passes` contains each unique output from `prettier(anchor)` up to and including the fixed
/// point. It is non-empty by construction — if prettier were idempotent on the anchor, no
/// signature file would exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditSignature {
    /// Each entry is one prettier pass past the anchor, indexed from 0. The last is the fixed point.
    pub passes: Vec<String>,
}

impl AuditSignature {
    /// Walk prettier on `start` until reaching a fixed point or `MAX_CHAIN_DEPTH`.
    ///
    /// Returns `None` when there's no useful chain to record:
    /// - Prettier is idempotent on `start` (no signature needed).
    /// - Prettier errors on `start` or some intermediate step (e.g., a fixture documents
    ///   prettier producing invalid syntax — re-running prettier on that captured output
    ///   reasonably fails; we don't pin those chains).
    ///
    /// Returns `Err` only when the chain hits `MAX_CHAIN_DEPTH` without converging — that's
    /// a real escape hatch worth surfacing.
    pub async fn walk(
        start: &str,
        parser: PrettierParser<'_>,
    ) -> Result<Option<AuditSignature>, String> {
        let mut passes = Vec::new();
        let mut current = start.to_string();
        for _ in 0..MAX_CHAIN_DEPTH {
            let Ok(next) = run_prettier(&current, parser).await else {
                return Ok(None);
            };
            if next == current {
                // Reached fixed point.
                if passes.is_empty() {
                    // Prettier is idempotent on `start` — no signature needed.
                    return Ok(None);
                }
                return Ok(Some(AuditSignature { passes }));
            }
            passes.push(next.clone());
            current = next;
        }
        Err(format!(
            "prettier chain did not converge within {MAX_CHAIN_DEPTH} passes"
        ))
    }

    /// Serialize to the canonical file format under `anchor`'s numbering and header.
    pub fn serialize(&self, anchor: ChainAnchor) -> String {
        let mut out = String::from(anchor.header());
        let last = self.passes.len();
        for (i, content) in self.passes.iter().enumerate() {
            let pass_num = i + anchor.first_pass_number();
            let is_fixed = i + 1 == last;
            use std::fmt::Write as _;
            if is_fixed {
                let _ = writeln!(out, "%%PASS={pass_num} (fixed point)%%");
            } else {
                let _ = writeln!(out, "%%PASS={pass_num}%%");
            }
            out.push_str(content);
            if !content.ends_with('\n') {
                out.push('\n');
            }
            if !is_fixed {
                out.push('\n');
            }
        }
        out
    }

    /// Parse a signature file. Tolerates the header and any number of pass sections.
    ///
    /// The parser is intentionally lenient about whitespace between sections but strict
    /// about section content — drift detection depends on byte-exact comparison.
    pub fn parse(content: &str) -> Result<AuditSignature, String> {
        let mut passes: Vec<String> = Vec::new();
        let mut current: Option<String> = None;

        for line in content.split_inclusive('\n') {
            let trimmed_end = line.trim_end_matches('\n');
            if let Some(rest) = trimmed_end.strip_prefix("%%PASS=")
                && (rest.ends_with("%%") || rest.ends_with("(fixed point)%%"))
            {
                // Start of a new section. Flush any in-progress one.
                if let Some(prev) = current.take() {
                    passes.push(strip_trailing_blank(&prev));
                }
                current = Some(String::new());
                continue;
            }
            if line.starts_with('#') && current.is_none() {
                // Header comment line before any section — skip.
                continue;
            }
            if let Some(buf) = current.as_mut() {
                buf.push_str(line);
            }
            // Lines outside any section that aren't header comments are dropped (whitespace).
        }
        if let Some(last) = current.take() {
            passes.push(strip_trailing_blank(&last));
        }

        if passes.is_empty() {
            return Err("signature file has no %%PASS=N%% sections".to_string());
        }
        Ok(AuditSignature { passes })
    }
}

/// Strip a single trailing blank line added by `serialize` between non-final sections.
fn strip_trailing_blank(s: &str) -> String {
    if let Some(stripped) = s.strip_suffix("\n\n") {
        format!("{stripped}\n")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_single_pass() {
        let sig = AuditSignature {
            passes: vec!["hello\nworld\n".to_string()],
        };
        let serialized = sig.serialize(ChainAnchor::OutputPrettier);
        assert!(serialized.contains("%%PASS=2 (fixed point)%%"));
        let parsed = AuditSignature::parse(&serialized).unwrap();
        assert_eq!(parsed, sig);
    }

    #[test]
    fn roundtrip_multi_pass() {
        let sig = AuditSignature {
            passes: vec![
                "one\n".to_string(),
                "two\n".to_string(),
                "three\n".to_string(),
            ],
        };
        let serialized = sig.serialize(ChainAnchor::OutputPrettier);
        assert!(serialized.contains("%%PASS=2%%\n"));
        assert!(serialized.contains("%%PASS=3%%\n"));
        assert!(serialized.contains("%%PASS=4 (fixed point)%%\n"));
        let parsed = AuditSignature::parse(&serialized).unwrap();
        assert_eq!(parsed, sig);
    }

    /// The `unformatted_ours_*` anchor numbers from 1 — nothing else pins `prettier(source)`,
    /// unlike `output_prettier`, which F2 already pins as pass 1.
    #[test]
    fn unformatted_ours_anchor_numbers_from_one() {
        let sig = AuditSignature {
            passes: vec!["one\n".to_string(), "two\n".to_string()],
        };
        let serialized = sig.serialize(ChainAnchor::UnformattedOurs);
        assert!(serialized.contains("%%PASS=1%%\n"));
        assert!(serialized.contains("%%PASS=2 (fixed point)%%\n"));
        assert!(serialized.contains("prettier^N(unformatted_ours_<suffix>.<ext>)"));
        assert_eq!(AuditSignature::parse(&serialized).unwrap(), sig);
    }

    #[test]
    fn roundtrip_preserves_trailing_blank_lines_in_content() {
        // Catches a subtle bug: serialize adds a separator `\n` after non-final sections,
        // and parse strips a trailing `\n\n`. If content legitimately ends in `\n\n`, the
        // round-trip must preserve it (don't strip more than the inserted separator).
        let sig = AuditSignature {
            passes: vec!["abc\n\n".to_string(), "def\n".to_string()],
        };
        let parsed = AuditSignature::parse(&sig.serialize(ChainAnchor::UnformattedOurs)).unwrap();
        assert_eq!(parsed, sig);
    }

    /// The three ways an output is already expressed without a chain pin. The updater
    /// (which writes the pin) and N12/N10 (which read it) all ask through this one
    /// predicate, so a disagreement would leave a chain pinned twice or not at all.
    #[test]
    fn coverage_predicate_spans_suffix_input_and_stable_forms() {
        let pins = SingleFormPins::from_parts(&["claimed"], &["stable\n"]);
        let input = "input\n";

        // An intermediate marker owns the suffix outright, whatever the bytes.
        assert!(pins.covers("claimed", "anything\n", input));
        // The output IS input — N6 owns that report.
        assert!(pins.covers("free", input, input));
        // The output equals a documented stable form.
        assert!(pins.covers("free", "stable\n", input));
        // Nothing holds it: this is the chain pin's case.
        assert!(!pins.covers("free", "novel\n", input));

        // The content-only reading, which the updater uses because the intermediates on
        // disk at scan time may be the stale ones it is about to remove.
        assert!(!pins.matches_documented_form("anything\n", input));
        assert!(pins.matches_documented_form("stable\n", input));

        // A fixture documenting no stable form makes its case by README alone — what keeps
        // N10's report informational rather than blocking.
        assert!(pins.has_documented_forms());
        assert!(!SingleFormPins::from_parts(&[], &[]).has_documented_forms());
    }

    /// The per-variant filename must never be mistaken for the `output_prettier`-anchored one:
    /// the two live side by side in the same directory.
    #[test]
    fn variant_filename_never_collides_with_the_bare_signature() {
        assert_eq!(
            audit_signature_variant_filename("own_line"),
            "audit_signature_own_line.txt"
        );
        assert_eq!(
            audit_signature_variant_suffix("audit_signature_own_line.txt"),
            Some("own_line")
        );
        assert_eq!(
            audit_signature_variant_suffix(AUDIT_SIGNATURE_FILENAME),
            None
        );
        // No suffix names no source file.
        assert_eq!(audit_signature_variant_suffix("audit_signature_.txt"), None);
        assert_eq!(audit_signature_variant_suffix("audit_signature_x.md"), None);
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(AuditSignature::parse("# only header\n").is_err());
    }
}
