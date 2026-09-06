//! The **repro directory** a mutation audit writes for one finding.
//!
//! The two authoring-independence audits share a shape: take a formatted seed `F` (a fixed
//! point), mutate it in a way the language says preserves the document, format the mutant, and
//! require the result to be `F` again. When that fails, the useful hand-off is not a line
//! number — it is the four texts, byte for byte:
//!
//! | file | what it is | what it answers |
//! | --- | --- | --- |
//! | `base` | `F`, the formatted seed | what the twin must format back to |
//! | `variant` | `F` with the mutation applied | the authoring that exposed it |
//! | `ftry` | `format(variant)` | where it actually landed |
//! | `ftry2` | `format(ftry)` | whether that is a second FIXED POINT or a non-idempotency |
//!
//! plus a `note.txt` naming the source, the bucket, and those two answers as booleans.
//!
//! One writer rather than one per audit so the shape is the same **by construction**: each
//! audit's docs tell the reader "the same four files the other one writes", and that had been
//! true only by having been copied. `fuzz --dump-dir` is deliberately not a consumer — a fuzz
//! finding IS an input, so it dumps one file per finding with no base to return to.

use std::path::Path;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::cli::commands::profile::lang_token;

/// One finding's repro material. `seq` disambiguates two findings in one file; `tag` is the
/// audit's own bucket name, and both go into the case directory's name.
#[derive(Clone, Copy)]
pub(crate) struct ReproCase<'a> {
    pub(crate) seq: usize,
    /// The audit's bucket for this finding (`bug_a`, `diverge`, …) — part of the directory name.
    pub(crate) tag: &'a str,
    /// What the mutation did, as one phrase for `note.txt` ("flip one boundary").
    pub(crate) mutation: &'a str,
    pub(crate) src_path: &'a Path,
    /// The formatted seed: a fixed point, and what the mutant must format back to.
    pub(crate) base: &'a str,
    /// `base` with the mutation applied.
    pub(crate) variant: &'a str,
    /// `format(variant)`. Empty when the variant failed to parse — itself a finding, and the
    /// note's two booleans then read `false` / `false`, which is the honest answer.
    pub(crate) ftry: &'a str,
    pub(crate) parser: ParserType,
}

/// Write `case` into `dir` as its own subdirectory. Best-effort: a directory that cannot be
/// created, or a file that cannot be written, is skipped silently — a dump is a convenience
/// beside the report, never the finding itself, so it must not fail a run or mask one.
pub(crate) fn write_repro_case(dir: &str, case: &ReproCase<'_>) {
    let slug: String = case
        .src_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("case")
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let case_dir = Path::new(dir).join(format!("{:03}_{}_{slug}", case.seq, case.tag));
    if std::fs::create_dir_all(&case_dir).is_err() {
        return;
    }
    let ftry2 = format_source(case.ftry, case.parser).unwrap_or_default();
    let note = format!(
        "source:  {}\nbucket:  {}\nbase F (a fixed point) -> {} -> variant -> format = ftry\nftry == F?       {}\nftry idempotent? {}\n",
        case.src_path.display(),
        case.tag,
        case.mutation,
        case.ftry == case.base,
        case.ftry == ftry2,
    );
    let ext = lang_token(case.parser);
    let _ = std::fs::write(case_dir.join(format!("base.{ext}")), case.base);
    let _ = std::fs::write(case_dir.join(format!("variant.{ext}")), case.variant);
    let _ = std::fs::write(case_dir.join(format!("ftry.{ext}")), case.ftry);
    let _ = std::fs::write(case_dir.join(format!("ftry2.{ext}")), ftry2);
    let _ = std::fs::write(case_dir.join("note.txt"), note);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The five files, the directory name, and the note's two booleans — the shape both audits
    /// document. Written as a test because the writer is best-effort by design (it swallows IO
    /// errors), so nothing else would notice it silently producing four files instead of five.
    #[test]
    fn writes_the_case_directory() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let dir = tmp.path().to_str().expect("utf-8 temp path");
        write_repro_case(
            dir,
            &ReproCase {
                seq: 7,
                tag: "diverge",
                mutation: "do the thing",
                src_path: Path::new("a/b/seed.svelte"),
                base: "const a = 1;\n",
                variant: "const a = (1);\n",
                ftry: "const a = 2;\n",
                parser: ParserType::TypeScript,
            },
        );
        let case = tmp.path().join("007_diverge_seed");
        for name in ["base.ts", "variant.ts", "ftry.ts", "ftry2.ts", "note.txt"] {
            assert!(case.join(name).is_file(), "missing {name}");
        }
        let note = std::fs::read_to_string(case.join("note.txt")).expect("note");
        assert!(note.contains("source:  a/b/seed.svelte"), "{note}");
        assert!(note.contains("-> do the thing ->"), "{note}");
        // `ftry` is neither the base nor a fixed point of its own here.
        assert!(note.contains("ftry == F?       false"), "{note}");
        assert!(note.contains("ftry idempotent? true"), "{note}");
    }

    /// A path whose stem carries characters a directory name should not: the slug keeps only
    /// alphanumerics, so the case directory is always nameable.
    #[test]
    fn slugs_the_seed_name() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let dir = tmp.path().to_str().expect("utf-8 temp path");
        write_repro_case(
            dir,
            &ReproCase {
                seq: 1,
                tag: "nonidem",
                mutation: "m",
                src_path: Path::new("unformatted_ours.head-weld.svelte"),
                base: "",
                variant: "",
                ftry: "",
                parser: ParserType::Svelte,
            },
        );
        assert!(
            tmp.path()
                .join("001_nonidem_unformatted_ours_head_weld")
                .is_dir()
        );
    }
}
