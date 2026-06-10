//! Audit signature: records prettier's multi-pass chain from `output_prettier.*` to its fixed point
//!
//! Some `_prettier_divergence` fixtures involve prettier non-idempotence: `prettier(output_prettier)`
//! produces a different output (and may take several more passes to stabilize). The audit signature
//! file pins the exact chain content so:
//!
//! - The `fixtures_audit` tool can recognize these documented multi-pass cases and stop flagging
//!   them as `[novel]`, while still flagging drift if the chain changes byte-for-byte.
//! - The validator (rule F4) byte-equality-checks each chain step on every run, so prettier-version
//!   drift in any intermediate pass surfaces as a fixture failure (stronger than the existing F2
//!   pass-1 check alone).
//!
//! The file is only generated when prettier is non-idempotent on `output_prettier.*`; idempotent
//! fixtures don't need it. Validation against this file therefore costs a few extra prettier runs
//! per affected fixture (fewer than a dozen today, scoped to TypeScript comment-relocation cases),
//! nothing for the rest.
//!
//! ## Intentional overlap with `variant_*` / `prettier_variant_*` files
//!
//! When a fixture has a `variant_X` (or `prettier_variant_X`) file whose content equals
//! `prettier^k(output_prettier)` for some k, the signature's pass-k entry will duplicate
//! that file's bytes. This is intentional, not a bug:
//!
//! - `N9a` (prettier idempotent on `variant_*`) and the audit's classification both confirm
//!   that the variant file is *self-stable*. Neither check verifies that *output_prettier's
//!   prettier-chain actually reaches that variant*. Prettier could in principle drift to a
//!   different stable form that's still variant-shaped, and both N9a and the audit would
//!   stay green.
//! - F4's byte-exact chain check directly enforces the chain-step transition itself, which
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

use crate::deno::{PrettierParser, run_prettier};

/// Filename for the audit signature file, located alongside `output_prettier.*`
pub const AUDIT_SIGNATURE_FILENAME: &str = "audit_signature.txt";

/// Maximum number of prettier passes to walk before declaring non-convergence.
/// Real cases observed in this repo bottom out at 3 (switch case_block_comment).
pub const MAX_CHAIN_DEPTH: usize = 8;

/// Header lines prepended to every signature file
const SIGNATURE_HEADER: &str = concat!(
    "# Auto-generated prettier chain signature. Do not edit manually.\n",
    "# Regenerate: deno task fixtures:update:formatted <pattern>\n",
    "#\n",
    "# Each %%PASS=N%% section is exactly prettier^N(output_prettier.<ext>).\n",
    "# This file exists when prettier is non-idempotent on output_prettier.* — it pins\n",
    "# the full chain so the audit recognizes the case and the validator catches drift.\n",
    "# See docs/fixture_overview.md (rule F4) and crates/tsv_debug/src/fixtures/audit_signature.rs.\n",
    "\n",
);

/// A captured prettier-chain anchored at `output_prettier.*`.
///
/// `passes` contains each unique output starting from `prettier^2(input)` (= `prettier(output_prettier)`),
/// up to and including the fixed point. `passes` is non-empty by construction — if prettier were
/// idempotent on `output_prettier`, no signature file would exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditSignature {
    /// Each entry is `prettier^(N+2)(input)`, indexed from 0. The last entry is the fixed point.
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

    /// Serialize to the canonical file format.
    pub fn serialize(&self) -> String {
        let mut out = String::from(SIGNATURE_HEADER);
        let last = self.passes.len();
        for (i, content) in self.passes.iter().enumerate() {
            let pass_num = i + 2; // pass=1 is output_prettier itself
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
            return Err("audit_signature.txt has no %%PASS=N%% sections".to_string());
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
        let serialized = sig.serialize();
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
        let serialized = sig.serialize();
        assert!(serialized.contains("%%PASS=2%%\n"));
        assert!(serialized.contains("%%PASS=3%%\n"));
        assert!(serialized.contains("%%PASS=4 (fixed point)%%\n"));
        let parsed = AuditSignature::parse(&serialized).unwrap();
        assert_eq!(parsed, sig);
    }

    #[test]
    fn roundtrip_preserves_trailing_blank_lines_in_content() {
        // Catches a subtle bug: serialize adds a separator `\n` after non-final sections,
        // and parse strips a trailing `\n\n`. If content legitimately ends in `\n\n`, the
        // round-trip must preserve it (don't strip more than the inserted separator).
        let sig = AuditSignature {
            passes: vec!["abc\n\n".to_string(), "def\n".to_string()],
        };
        let parsed = AuditSignature::parse(&sig.serialize()).unwrap();
        assert_eq!(parsed, sig);
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(AuditSignature::parse("# only header\n").is_err());
    }
}
