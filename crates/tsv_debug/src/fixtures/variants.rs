//! Single-scan discovery of a fixture directory's variant files.

use crate::fixtures::Fixture;
use crate::fixtures::{
    AUDIT_SIGNATURE_FILENAME, GOAL_FILENAME, PRETTIER_NONCONVERGENT_FILENAME,
    PRETTIER_REJECTS_FILENAME, TSV_REJECTS_FILENAME, audit_signature_variant_suffix,
};
use std::fs;

/// Filename prefix of the variants only OUR formatter normalizes to input.
///
/// Several rules key off the *pairing* between one of these and a same-suffix sibling —
/// the three `prettier_intermediate*_*` kinds (N7/N7b/N7c) and `audit_signature_<suffix>.txt`
/// (N12/S21) — so both directions of the name↔suffix map live here as
/// [`unformatted_ours_filename`] and [`unformatted_ours_suffix`] rather than being spelled
/// out at each site.
const UNFORMATTED_OURS_PREFIX: &str = "unformatted_ours_";

/// The `unformatted_ours_*` filename for a suffix, in a fixture whose input has `input_ext`.
pub fn unformatted_ours_filename(suffix: &str, input_ext: &str) -> String {
    format!("{UNFORMATTED_OURS_PREFIX}{suffix}{input_ext}")
}

/// The suffix of an `unformatted_ours_*` filename, or `None` when the name isn't one.
///
/// The extension must match the fixture's input type; a wrong-extension name is not an
/// `unformatted_ours_*` variant at all (`variant_bucket` sends it to `unknown`), so it gets
/// no suffix here either.
pub fn unformatted_ours_suffix<'a>(filename: &'a str, input_ext: &str) -> Option<&'a str> {
    filename
        .strip_prefix(UNFORMATTED_OURS_PREFIX)?
        .strip_suffix(input_ext)
}

/// Which single-form variant marker can express a prettier-**stable** form `V`, judged by
/// what our formatter does with it.
///
/// The three marker kinds are distinguished purely by `ours(V)`, and two outcomes name no
/// marker at all. Three callers ask this one question — `fixtures_audit` (to suggest a file),
/// `fixtures_update_formatted` (to decide whether a chain pin is the only expressible pin),
/// and N10's report (to name the file to add) — so it has one definition. They had drifted
/// into partial spellings: the updater asked only "does tsv reject it?", which is the last
/// arm alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableFormMarker {
    /// `ours(V) == input` — a `prettier_variant_*`.
    PrettierVariant,
    /// `ours(V) == V` — dual-stable, a `variant_*`.
    Variant,
    /// `ours(V)` is a third fixed point (≠ input, ≠ `V`) — a `divergent_variant_*`.
    DivergentVariant,
    /// `ours` rewrites `V` to something that is not itself a fixed point. A tsv idempotency
    /// bug, not a marker choice — no marker can hold it, and pinning the chain instead would
    /// paper over the bug.
    OursNotIdempotent,
    /// tsv cannot format `V` at all. Every marker above asserts something about `ours(V)`, so
    /// none is reachable and a chain pin is the only expressible claim.
    OursRejects,
}

impl StableFormMarker {
    /// The variant filename prefix this marker names, or `None` for the two non-marker arms.
    pub const fn file_prefix(self) -> Option<&'static str> {
        match self {
            Self::PrettierVariant => Some("prettier_variant_"),
            Self::Variant => Some("variant_"),
            Self::DivergentVariant => Some("divergent_variant_"),
            Self::OursNotIdempotent | Self::OursRejects => None,
        }
    }

    /// Does a prettier-STABLE form with this classification require the chain pin
    /// (`audit_signature_<suffix>.txt`)? Only `OursRejects`: every single-form marker
    /// asserts something about `ours(V)`, so none is reachable there. `OursNotIdempotent`
    /// deliberately answers NO — that is a tsv bug, and pinning the chain would paper
    /// over it. One predicate for the two askers — the updater's `needs_chain_pin`
    /// (writing side) and N10's blocking arm (absence side) — so they cannot drift.
    pub const fn requires_chain_pin(self) -> bool {
        matches!(self, Self::OursRejects)
    }
}

/// Classify a prettier-stable form `V` by what our formatter does with it.
///
/// Pure Rust — the caller has already established that prettier holds `V` stable; every
/// remaining question is about `ours`. `input_file` names the fixture's input so the right
/// language formatter is selected.
pub fn classify_stable_form(v: &str, input: &str, input_file: &str) -> StableFormMarker {
    let Ok(ours) = crate::fixtures::format_with_our_formatter(v, input_file) else {
        return StableFormMarker::OursRejects;
    };
    if ours == input {
        return StableFormMarker::PrettierVariant;
    }
    if ours == v {
        return StableFormMarker::Variant;
    }
    match crate::fixtures::format_with_our_formatter(&ours, input_file) {
        Ok(second) if second == ours => StableFormMarker::DivergentVariant,
        _ => StableFormMarker::OursNotIdempotent,
    }
}

/// A fixture directory's variant files, partitioned by filename prefix in a
/// single directory scan (each list sorted by name).
///
/// Built once per fixture (`FixtureFiles::scan`) and threaded to every
/// consumer — structure rules, validation phases, audits, update commands —
/// so each fixture directory is read exactly once instead of once per
/// variant kind.
#[derive(Debug, Default)]
pub struct FixtureFiles {
    /// `unformatted_*` (excluding `unformatted_ours_*` / `unformatted_prettier_*`):
    /// variants both formatters normalize to the input file.
    pub unformatted: Vec<String>,
    /// `unformatted_ours_*`: variants only OUR formatter normalizes to input
    /// (used in `_prettier_divergence` directories where prettier validation
    /// is skipped).
    pub unformatted_ours: Vec<String>,
    /// `unformatted_prettier_*`: variants prettier normalizes to
    /// `output_prettier.*` (our formatter validation is not applied).
    pub unformatted_prettier: Vec<String>,
    /// `prettier_variant_*`: prettier's stable variants — inputs prettier
    /// preserves as-is while our formatter normalizes them to input.
    pub prettier_variant: Vec<String>,
    /// `variant_*`: dual-stable forms both formatters keep stable (distinct
    /// from input, unlike `prettier_variant_*`).
    pub variant: Vec<String>,
    /// `divergent_variant_*`: prettier-stable forms our formatter rewrites to a *third*
    /// stable form (distinct from both `V` and input). Unlike `variant_*` (both
    /// keep `V`) and `prettier_variant_*` (ours → input), ours settles on a form
    /// that is neither — the divergent-variant category (three distinct stable
    /// forms: input, `V`, and `ours(V)`).
    pub divergent_variant: Vec<String>,
    /// `prettier_intermediate_*` (excluding `prettier_intermediate_to_variant_*`):
    /// prettier's unstable first-pass output from `unformatted_ours_*` files;
    /// the second pass converges to input.
    pub prettier_intermediate: Vec<String>,
    /// `prettier_intermediate_to_variant_*`: like `prettier_intermediate`,
    /// but the second pass converges to a documented `variant_*` /
    /// `prettier_variant_*` form rather than input (N7b).
    pub prettier_intermediate_to_variant: Vec<String>,
    /// `prettier_intermediate_to_divergent_variant_*`: like
    /// `prettier_intermediate_to_variant`, but the second pass converges to a
    /// documented `divergent_variant_*` form (a prettier-stable form our
    /// formatter rewrites to a third form) rather than input or a `variant_*`
    /// (N7c). The convergence target no other intermediate marker accepts —
    /// arises when prettier's unstable first pass on an `unformatted_ours_*`
    /// shell settles on a glued form ours un-glues (the intersection
    /// first-member redundant-paren mixed case).
    pub prettier_intermediate_to_divergent_variant: Vec<String>,
    /// `input_invalid_*`: invalid syntax that must fail BOTH parsers.
    pub input_invalid: Vec<String>,
    /// `audit_signature_<suffix>.txt`: prettier's full chain from the same-suffix
    /// `unformatted_ours_*` source to its fixed point (N12). Carries a `.txt`
    /// extension rather than the input's, so it is bucketed by name in `scan`
    /// rather than by `variant_bucket`'s extension-gated prefix chain.
    pub audit_signature_variant: Vec<String>,
    /// `prettier_nonconvergent.txt` marker present: prettier has no fixed
    /// point on this input, so F2/F3/F4 and the prettier-side N rules are
    /// replaced by the live non-convergence check (F5).
    pub prettier_nonconvergent: bool,
    /// `prettier_rejects.txt` marker present: prettier throws on this input
    /// (parse rejection or printer crash), so F2/F3/F4 and the prettier-side N
    /// rules are replaced by the live rejection check (F6). The file's trimmed
    /// content is the expected-error substring.
    pub prettier_rejects: bool,
    /// `tsv_rejects.txt` marker present: tsv rejects this input while the
    /// canonical parser accepts it. The tsv-side parser/formatter phases and the
    /// prettier-formatter side are replaced by the live rejection check (F7) —
    /// tsv must fail with the marker's trimmed substring, and `expected_svelte.json`
    /// must still match the canonical parser.
    pub tsv_rejects: bool,
    /// Files matching no known fixture pattern — catches typos like
    /// "unformated_*.svelte" (missing 't') or accidental additions.
    /// Variant-prefixed files with the wrong extension land here too.
    pub unknown: Vec<String>,
}

impl FixtureFiles {
    /// The three intermediate lists paired with their filename prefixes — the one table
    /// for callers that walk "every intermediate kind" (`SingleFormPins::collect`, the
    /// updater's orphan sweep), so a fourth kind cannot be added to one walker and
    /// silently missed by another.
    pub fn intermediate_kinds(&self) -> [(&Vec<String>, &'static str); 3] {
        [
            (&self.prettier_intermediate, "prettier_intermediate_"),
            (
                &self.prettier_intermediate_to_variant,
                "prettier_intermediate_to_variant_",
            ),
            (
                &self.prettier_intermediate_to_divergent_variant,
                "prettier_intermediate_to_divergent_variant_",
            ),
        ]
    }

    /// Scan the fixture directory once and partition entries by filename.
    pub fn scan(fixture: &Fixture) -> Self {
        let ext = fixture.input_type().extension();
        let mut files = Self::default();

        let Ok(entries) = fs::read_dir(&fixture.path) else {
            return files;
        };

        for entry in entries.flatten() {
            let os_filename = entry.file_name();
            let Some(filename) = os_filename.to_str() else {
                continue;
            };
            if filename == PRETTIER_NONCONVERGENT_FILENAME {
                files.prettier_nonconvergent = true;
                continue;
            }
            if filename == PRETTIER_REJECTS_FILENAME {
                files.prettier_rejects = true;
                continue;
            }
            if filename == TSV_REJECTS_FILENAME {
                files.tsv_rejects = true;
                continue;
            }
            if is_static_fixture_file(filename) {
                continue;
            }
            // Checked after the exact-name statics so `audit_signature.txt` (no suffix)
            // stays the F4 file rather than becoming a suffix-less N12 one.
            if audit_signature_variant_suffix(filename).is_some() {
                files.audit_signature_variant.push(filename.to_string());
                continue;
            }
            if let Some(bucket) = files.variant_bucket(filename, ext) {
                bucket.push(filename.to_string());
            } else if entry.path().is_file() {
                files.unknown.push(filename.to_string());
            }
        }

        files.unformatted.sort();
        files.unformatted_ours.sort();
        files.unformatted_prettier.sort();
        files.prettier_variant.sort();
        files.variant.sort();
        files.divergent_variant.sort();
        files.prettier_intermediate.sort();
        files.prettier_intermediate_to_variant.sort();
        files.prettier_intermediate_to_divergent_variant.sort();
        files.input_invalid.sort();
        files.audit_signature_variant.sort();
        files.unknown.sort();
        files
    }

    /// The bucket a variant filename belongs to, or `None` for non-variants.
    ///
    /// More specific prefixes are checked before their parents
    /// (`unformatted_ours_` before `unformatted_`, …). Variant files must
    /// carry the input file's extension; wrong-extension matches fall
    /// through to `unknown`.
    fn variant_bucket(&mut self, filename: &str, ext: &str) -> Option<&mut Vec<String>> {
        if !filename.ends_with(ext) {
            return None;
        }
        if filename.starts_with(UNFORMATTED_OURS_PREFIX) {
            Some(&mut self.unformatted_ours)
        } else if filename.starts_with("unformatted_prettier_") {
            Some(&mut self.unformatted_prettier)
        } else if filename.starts_with("unformatted_") {
            Some(&mut self.unformatted)
        } else if filename.starts_with("prettier_intermediate_to_divergent_variant_") {
            Some(&mut self.prettier_intermediate_to_divergent_variant)
        } else if filename.starts_with("prettier_intermediate_to_variant_") {
            Some(&mut self.prettier_intermediate_to_variant)
        } else if filename.starts_with("prettier_intermediate_") {
            Some(&mut self.prettier_intermediate)
        } else if filename.starts_with("prettier_variant_") {
            Some(&mut self.prettier_variant)
        } else if filename.starts_with("variant_") {
            Some(&mut self.variant)
        } else if filename.starts_with("divergent_variant_") {
            Some(&mut self.divergent_variant)
        } else if filename.starts_with("input_invalid_") {
            Some(&mut self.input_invalid)
        } else {
            None
        }
    }
}

/// Check if a filename is a known non-variant fixture file
/// (input, expected JSON, output_prettier, README, audit signature).
fn is_static_fixture_file(filename: &str) -> bool {
    matches!(
        filename,
        "input.svelte"
            | "input.svelte.ts"
            | "input.ts"
            | "input.css"
            | "expected.json"
            | "expected_ours.json"
            | "expected_svelte.json"
            | "output_prettier.svelte"
            | "output_prettier.svelte.ts"
            | "output_prettier.ts"
            | "output_prettier.css"
            | "README.md"
            | AUDIT_SIGNATURE_FILENAME
            | GOAL_FILENAME
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Which bucket a filename lands in, by pushing through `variant_bucket`
    /// and seeing which list became non-empty.
    fn bucket_of(filename: &str, ext: &str) -> Option<&'static str> {
        let mut files = FixtureFiles::default();
        files
            .variant_bucket(filename, ext)?
            .push(filename.to_string());
        let buckets = [
            ("unformatted", &files.unformatted),
            ("unformatted_ours", &files.unformatted_ours),
            ("unformatted_prettier", &files.unformatted_prettier),
            ("prettier_variant", &files.prettier_variant),
            ("variant", &files.variant),
            ("divergent_variant", &files.divergent_variant),
            ("prettier_intermediate", &files.prettier_intermediate),
            (
                "prettier_intermediate_to_variant",
                &files.prettier_intermediate_to_variant,
            ),
            (
                "prettier_intermediate_to_divergent_variant",
                &files.prettier_intermediate_to_divergent_variant,
            ),
            ("input_invalid", &files.input_invalid),
        ];
        buckets.iter().find(|(_, v)| !v.is_empty()).map(|(n, _)| *n)
    }

    /// More specific prefixes must win over their parents — some variant kinds
    /// (e.g. `unformatted_prettier_*`) have no in-tree fixtures, so the e2e
    /// suite doesn't exercise every arm of the chain.
    #[test]
    fn prefix_precedence() {
        let cases = [
            ("unformatted_x.svelte", "unformatted"),
            ("unformatted_ours_x.svelte", "unformatted_ours"),
            ("unformatted_prettier_x.svelte", "unformatted_prettier"),
            ("prettier_variant_x.svelte", "prettier_variant"),
            ("variant_x.svelte", "variant"),
            ("divergent_variant_x.svelte", "divergent_variant"),
            ("prettier_intermediate_x.svelte", "prettier_intermediate"),
            (
                "prettier_intermediate_to_variant_x.svelte",
                "prettier_intermediate_to_variant",
            ),
            (
                "prettier_intermediate_to_divergent_variant_x.svelte",
                "prettier_intermediate_to_divergent_variant",
            ),
            ("input_invalid_x.svelte", "input_invalid"),
        ];
        for (filename, expected) in cases {
            assert_eq!(bucket_of(filename, ".svelte"), Some(expected), "{filename}");
        }
    }

    /// Variant files must carry the input file's extension; everything else
    /// (wrong extension, typo'd prefix) is not a variant and lands in `unknown`.
    #[test]
    fn non_variants_fall_through() {
        assert_eq!(bucket_of("unformatted_x.ts", ".svelte"), None);
        assert_eq!(bucket_of("unformatted_x.svelte", ".svelte.ts"), None);
        assert_eq!(bucket_of("unformated_typo.svelte", ".svelte"), None);
        assert_eq!(bucket_of("notes.svelte", ".svelte"), None);
    }

    /// Only `OursRejects` requires the chain pin: a formattable stable form takes a
    /// single-form marker instead, and `OursNotIdempotent` is a tsv bug a pin would
    /// paper over. The updater's `needs_chain_pin` and N10's blocking arm both ride
    /// this predicate, so its answer set is pinned here.
    #[test]
    fn only_ours_rejects_requires_the_chain_pin() {
        assert!(StableFormMarker::OursRejects.requires_chain_pin());
        assert!(!StableFormMarker::PrettierVariant.requires_chain_pin());
        assert!(!StableFormMarker::Variant.requires_chain_pin());
        assert!(!StableFormMarker::DivergentVariant.requires_chain_pin());
        assert!(!StableFormMarker::OursNotIdempotent.requires_chain_pin());
    }

    #[test]
    fn static_files_are_known() {
        assert!(is_static_fixture_file("input.svelte"));
        assert!(is_static_fixture_file("expected.json"));
        assert!(is_static_fixture_file("README.md"));
        assert!(is_static_fixture_file(AUDIT_SIGNATURE_FILENAME));
        assert!(!is_static_fixture_file("notes.md"));
        // The per-variant signature is NOT static — it's bucketed, and the bare
        // signature must not absorb it.
        assert!(!is_static_fixture_file("audit_signature_own_line.txt"));
    }
}
