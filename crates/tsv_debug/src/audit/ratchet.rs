//! A snapshot **ratchet** — a reusable library for the "every line is a known
//! bug, the file shrinking is the goal" gate shape.
//!
//! An audit that produces a large, churny set of finding shapes can't gate on an
//! exact count (a count pinned per shape fails on every ordinary fixture PR, and
//! a gate that fails per added fixture gets turned off). The ratchet gates on the
//! *shape set* instead: a machine-generated snapshot file lists every known-bug
//! key, and the gate fails on
//!
//! - a key that is **not** in the snapshot (a new kind of bug), or
//! - a snapshot key that **no longer fires** (a fixed bug, or a rotting list) — so
//!   fixing a bug must remove its line and the list can't rot, or
//! - any **unpinnable** key (see [`SnapshotKey::is_pinnable`]) — an invariant so
//!   absolute it is never allowed into the list whose shrinking is the goal.
//!
//! `gap_audit` (`gap_audit_known.txt`), `blank_audit` (`blank_audit_known.txt`),
//! `ignore_audit` (`ignore_audit_known.txt`), `fabrication_audit`
//! (`fabrication_audit_known.txt`), `census_audit` and `width_audit`
//! (`width_audit_known.txt`) are the consumers. It is written
//! generic — parameterized on the snapshot **path**, the **key type**
//! ([`SnapshotKey`], which owns its own line render/parse and its pinnability
//! rule), and (via that trait) the **pinnable predicate** — so each later
//! injection audit adopted it without copying the read/render/grade/refuse-narrow
//! logic. It is deliberately *minimal*: no generality beyond what a second
//! consumer would actually reuse. The consumer-side orchestration those audits
//! were still copying — the snapshot's colocated path, the narrowed-`--update`
//! refusal, the write confirmation, the grade→verdict→exit-status tail, the
//! "ratchet SKIPPED" notice, the unpinned-PANIC epilogue — lives here too
//! ([`Ratchet::colocated`] / [`refuse_narrowed_update`] /
//! [`Ratchet::write_pinned`] / [`Ratchet::grade_and_report`] /
//! [`print_ratchet_skipped`] / [`report_unpinned_panics`]). A consumer that
//! needs its own tail (gap's yield line, blank's PANIC class) calls
//! [`Ratchet::grade`] and prints its own; the rest hand the whole thing over.
//!
//! What the snapshot buys and does not: a shape not on the list fails the gate,
//! so no *new* kind of bug lands silently; but a new instance at an **existing**
//! shape is invisible (the key is the shape, not a count). That tradeoff is the
//! whole reason a count can't be the key.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::cli::CliError;

/// Where every audit snapshot lives, relative to this crate's manifest. One constant so the
/// convention ("colocated with the command that owns it") is stated once rather than spelled
/// into each consumer's own path helper.
const SNAPSHOT_DIR: &str = "src/cli/commands";

/// One snapshot line: a key that renders to / parses from a single record, and
/// knows whether it may be **pinned**.
///
/// The key IS the record — [`Self::to_line`] and [`Self::from_line`] are inverse
/// over the snapshot's line format (a TAB-delimited row, by convention), and the
/// key's [`Ord`] is what fixes the on-disk line order, so a consumer whose key
/// sorts in its report order gets a stable, minimal-diff snapshot for free.
pub(crate) trait SnapshotKey: Ord + Clone + Sized {
    /// Render this key to one snapshot line, **without** a trailing newline (the
    /// ratchet adds it). By convention a TAB-delimited row.
    fn to_line(&self) -> String;

    /// Parse one snapshot line back into a key, or `None` if it is malformed. The
    /// ratchet has already stripped the trailing whitespace and filtered out
    /// blank / `#`-comment lines, so this sees only candidate records.
    fn from_line(line: &str) -> Option<Self>;

    /// Whether this key may be written into the snapshot. A key that is **not**
    /// pinnable (gap's `PANIC`: a comment in a gap must never crash the formatter)
    /// breaks an *absolute* invariant, so the ratchet never writes it and never
    /// diffs it — it is counted separately ([`GateDiff::unpinnable`]) and always
    /// fails the gate. Without this an `--update` would quietly launder such a
    /// key into the list whose shrinking is the goal.
    fn is_pinnable(&self) -> bool;
}

/// A ratchet over one snapshot file of [`SnapshotKey`] records.
///
/// The file is read at **runtime** ([`Self::parse_known`]), not embedded with
/// `include_str!`: only the path is compile-time. Embedding would recompile the
/// crate on every re-pin — a per-slice tax — for a data file the binary is
/// otherwise indifferent to.
pub(crate) struct Ratchet {
    /// The snapshot file. Colocated with the code that owns it, by convention.
    path: PathBuf,
    /// The `#`-comment header the render prepends verbatim. Owns the file's
    /// self-documentation, so it travels with the consumer, not the ratchet.
    header: &'static str,
    /// The command that regenerates the snapshot (e.g. `deno task gaps:audit:update`)
    /// — quoted in the read-failure message so it stays actionable.
    repin_hint: &'static str,
}

impl Ratchet {
    pub(crate) fn new(path: PathBuf, header: &'static str, repin_hint: &'static str) -> Self {
        Self {
            path,
            header,
            repin_hint,
        }
    }

    /// A ratchet over `file_name` in [`SNAPSHOT_DIR`] — the form every real consumer wants,
    /// since every snapshot is colocated with the command that owns it.
    ///
    /// Anchored on `CARGO_MANIFEST_DIR` rather than the cwd, so an audit finds its snapshot from
    /// any working directory. The path is the only compile-time piece; the file itself is read at
    /// runtime (see this module's docs for why not `include_str!`). `env!` expands where it is
    /// written, and that is here — the same crate as every consumer — so the anchor is theirs.
    pub(crate) fn colocated(
        file_name: &str,
        header: &'static str,
        repin_hint: &'static str,
    ) -> Self {
        Self::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(SNAPSHOT_DIR)
                .join(file_name),
            header,
            repin_hint,
        )
    }

    /// The snapshot file, for the consumers that report on it directly (an existence probe
    /// before a re-pin diff, a path in a header line) rather than through [`Self::grade`].
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Read the snapshot from disk and parse it into its key set.
    ///
    /// A read failure is a **hard error**, not an empty set: the gate cannot grade
    /// without the snapshot, and grading against an empty one would read every
    /// pinned bug as stale and every found shape as new — a wall of noise, or worse
    /// a silent pass if the diff were inverted. So a missing/unreadable file fails
    /// loudly.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] (after a user-facing message) when the snapshot
    /// file cannot be read.
    pub(crate) fn parse_known<K: SnapshotKey>(&self) -> Result<BTreeSet<K>, CliError> {
        let contents = std::fs::read_to_string(&self.path).map_err(|e| {
            eprintln!(
                "Error: cannot read the ratchet snapshot {}: {e}. The gate cannot grade \
                 without it — restore the file or re-pin with `{}`.",
                self.path.display(),
                self.repin_hint
            );
            CliError::Failed
        })?;
        Ok(contents
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(K::from_line)
            .collect())
    }

    /// Render the snapshot file for `found`: the header, an honest `# shapes: N` count
    /// stamp, then one line per **pinnable** key in `found`'s [`Ord`] order (the
    /// non-pinnable ones are dropped here and accounted for by [`Self::grade`]).
    ///
    /// The stamp exists because the file also carries blank / `#`-comment lines, so a
    /// casual `head`/`wc -l` over-counts the shapes. It is another `#` line, so
    /// [`Self::parse_known`] ignores it and the gate is indifferent to it — `--update`
    /// keeps it honest.
    pub(crate) fn render<K: SnapshotKey>(&self, found: &BTreeSet<K>) -> String {
        let pinnable: Vec<&K> = found.iter().filter(|k| k.is_pinnable()).collect();
        let mut out = String::new();
        out.push_str(self.header);
        out.push_str("# shapes: ");
        out.push_str(&pinnable.len().to_string());
        out.push('\n');
        for key in pinnable {
            out.push_str(&key.to_line());
            out.push('\n');
        }
        out
    }

    /// Render ([`Self::render`]) and write the snapshot to disk.
    ///
    /// The caller is responsible for only writing from a run whose `found` set is
    /// the *full* one the snapshot describes — a narrowed run reaches only part of
    /// the shape set, and pinning it would silently unpin real bugs. The ratchet
    /// cannot know what "full" means for a given consumer, so that guard lives at
    /// the call site (gap's `narrowing_flags`).
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] (after a user-facing message) when the snapshot
    /// file cannot be written.
    pub(crate) fn write<K: SnapshotKey>(&self, found: &BTreeSet<K>) -> Result<(), CliError> {
        let rendered = self.render(found);
        std::fs::write(&self.path, &rendered).map_err(|e| {
            eprintln!("Error: cannot write {}: {e}", self.path.display());
            CliError::Failed
        })
    }

    /// [`Self::write`], plus the consumer bookkeeping every audit was copying around it:
    /// compute the pinnable subset that was actually written, print the `✓ wrote N` line, and
    /// return that pinned set (gap's yield line diffs it against the pre-write snapshot; the
    /// other consumers drop it). `noun` is the audit's own word for a snapshot key — gap /
    /// blank "shape", ignore "position".
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the snapshot file cannot be written ([`Self::write`]).
    pub(crate) fn write_pinned<K: SnapshotKey>(
        &self,
        found: &BTreeSet<K>,
        noun: &str,
    ) -> Result<BTreeSet<K>, CliError> {
        self.write(found)?;
        let pinned: BTreeSet<K> = found.iter().filter(|k| k.is_pinnable()).cloned().collect();
        println!(
            "✓ wrote {} {noun}(s) to {}",
            pinned.len(),
            self.path.display()
        );
        Ok(pinned)
    }

    /// Diff a run's `found` keys against the committed snapshot.
    ///
    /// `found` is the run's **whole** key set — pinnable and not. The ratchet
    /// splits it: the pinnable keys are diffed against the snapshot (`new` /
    /// `stale`), and the rest are counted into [`GateDiff::unpinnable`], never
    /// diffed. Grading decides nothing about the *exit status* (see [`GateDiff`]);
    /// the one thing it can fail on is reading the snapshot, which is fatal.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the snapshot cannot be read
    /// ([`Self::parse_known`]).
    pub(crate) fn grade<K: SnapshotKey>(
        &self,
        found: &BTreeSet<K>,
    ) -> Result<GateDiff<K>, CliError> {
        let known = self.parse_known::<K>()?;
        let pinnable: BTreeSet<K> = found.iter().filter(|k| k.is_pinnable()).cloned().collect();
        let unpinnable = found.iter().filter(|k| !k.is_pinnable()).count();
        Ok(GateDiff {
            new: pinnable.difference(&known).cloned().collect(),
            stale: known.difference(&pinnable).cloned().collect(),
            known: known.len(),
            unpinnable,
        })
    }

    /// [`Self::grade`], then the verdict and the exit status — the tail every simple ratchet
    /// consumer was copying. `noun` is the audit's own word for a key (gap / blank "shape",
    /// census "key"), `scope` is the trailing parenthetical of the ✓ line ("7426 files"), and
    /// `render` spells one key for a human.
    ///
    /// Total over [`GateDiff`]: the unpinnable class gets its own line rather than falling
    /// through silently. No current consumer of this method has one (their `is_pinnable` is
    /// always true), but a `holds()` that is false with nothing printed is the one way this
    /// helper could exit non-zero without saying why, so it is stated rather than assumed. An
    /// audit that owns a real never-pinnable class wants [`report_unpinned_panics`] and its own
    /// tail instead — that class needs naming, not counting.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the snapshot cannot be read ([`Self::grade`]), or when
    /// the ratchet does not hold.
    pub(crate) fn grade_and_report<K: SnapshotKey>(
        &self,
        found: &BTreeSet<K>,
        noun: &str,
        scope: &str,
        render: impl Fn(&K) -> String,
    ) -> Result<(), CliError> {
        let diff = self.grade(found)?;
        if diff.holds() {
            println!(
                "\n✓ ratchet holds — {} known {noun}(s), no new ones ({scope})",
                diff.known
            );
            return Ok(());
        }
        for key in &diff.new {
            eprintln!("✗ NEW {noun} (not pinned): {}", render(key));
        }
        for key in &diff.stale {
            eprintln!(
                "✗ STALE pin (no longer fires — fix landed or fixture moved, re-pin): {}",
                render(key)
            );
        }
        if diff.unpinnable > 0 {
            eprintln!(
                "✗ {} unpinnable {noun}(s) — never written, always failing",
                diff.unpinnable
            );
        }
        eprintln!(
            "\nRe-pin with `{}` once the change is deliberate.",
            self.repin_hint
        );
        Err(CliError::Failed)
    }
}

/// What a [`Ratchet::grade`] found, computed **before** anything prints.
///
/// Grading is split from reporting for one reason: a ratchet that holds has
/// nothing to act on, so printing its (many) known shapes is noise inside a
/// passing gate — and whether it holds is only knowable after the diff. Deciding
/// first lets a clean gate print a summary instead. The consumer turns this into
/// an exit status.
pub(crate) struct GateDiff<K> {
    /// How many keys the snapshot pins — the denominator in the ✓ line.
    pub(crate) known: usize,
    /// Keys the snapshot has never seen: a new kind of finding.
    pub(crate) new: Vec<K>,
    /// Pinned keys that no longer fire — a fixed bug, or a rotting list.
    pub(crate) stale: Vec<K>,
    /// Found keys that are not pinnable (see [`SnapshotKey::is_pinnable`]). Never
    /// written, never diffed — graded on their own, and always failing.
    pub(crate) unpinnable: usize,
}

impl<K> GateDiff<K> {
    /// Whether the ratchet holds: no new key, no stale key, no unpinnable key.
    pub(crate) fn holds(&self) -> bool {
        self.new.is_empty() && self.stale.is_empty() && self.unpinnable == 0
    }
}

// ---------------------------------------------------------------------------
// Consumer orchestration — the prose every ratchet-consuming audit was copying
// into its `run()`. The ones an always-compiled consumer reaches
// (`fabrication_audit`, `census_audit`, `width_audit`) are unconditional; the
// rest stay behind `comment_check` with the injection audits that are their
// only callers.
// `compile_corpus_compare --ratchet` has its own path-keyed flow with different
// semantics and keeps its own messages.
// ---------------------------------------------------------------------------

/// Refuse `--update` on a narrowed run. The snapshot describes the FULL default run —
/// `scope` names it in the audit's own words (e.g. "the blank payload over tests/fixtures") —
/// so pinning a narrowed one would silently unpin real bugs. `relation` names what the
/// narrowed shape set is relative to the snapshot's ("SUBSET"; gap's
/// "SUBSET (or, for --all-bytes, a superset)"). A no-op unless `update` and the run is
/// narrowed, so it is callable unconditionally at the top of a consumer's `run()`.
///
/// # Errors
///
/// Returns [`CliError::Failed`] (after the user-facing refusal) when `update` is set on a
/// narrowed run.
pub(crate) fn refuse_narrowed_update(
    update: bool,
    narrowed: &[&'static str],
    scope: &str,
    relation: &str,
) -> Result<(), CliError> {
    if !update || narrowed.is_empty() {
        return Ok(());
    }
    let flags = narrowed.join(" / ");
    eprintln!(
        "Error: --update pins the FULL default run ({scope}). This run is narrowed by \
         {flags}, so its shape set is a {relation} of what the snapshot means — writing it \
         would silently unpin real bugs. Re-run without {flags}."
    );
    Err(CliError::Failed)
}

/// The notice a narrowed (non-`--update`) default run prints INSTEAD of grading: it reaches
/// only part of the snapshot's shape set, so grading would report every unreached shape as
/// stale — findings are reported, never graded, and the run must not read as a passing gate.
///
/// Not gated: `width_audit` is an ungated consumer (a narrowed run there is a *triage* run and
/// always exits 0, which is exactly the shape that would otherwise read as a green gate). An
/// audit whose findings are bugs wherever they are found wants [`grade_narrowed_strictly`]
/// instead — it still grades, just not against the snapshot.
pub(crate) fn print_ratchet_skipped(narrowed: &[&'static str]) {
    eprintln!(
        "\n○ ratchet SKIPPED — {} narrows this run, and the snapshot pins the full default \
         one. Findings above are reported, NOT graded: this is not a passing gate.",
        narrowed.join(" / ")
    );
}

/// The verdict a narrowed (non-`--update`) run prints when the audit grades STRICTLY rather
/// than consulting the snapshot — and the exit status that goes with it.
///
/// The sibling of [`print_ratchet_skipped`], and the wording is deliberately not shared with
/// it. Both say the snapshot pins the full default run and was not read; they differ on what
/// happens next, because the audits differ on what a finding MEANS off-corpus:
///
/// - `width_audit` has no zero to grade against — the sanctioned overruns are everywhere, so a
///   narrowed run reports and exits 0. That is [`print_ratchet_skipped`]: a triage run, and its
///   whole job is to say it is not a gate.
/// - `fabrication_audit` / `census_audit` have a zero target wherever they are pointed (an
///   invented blank, a lost comment — a bug in any subtree), so a narrowed run still GRADES.
///   Strictly: against zero rather than against the snapshot, so a finding that is pinned on
///   the default corpus fails here.
///
/// Which makes the notice load-bearing in both directions. A clean narrowed run proves less
/// than the standing gate does (the snapshot went unread), and a failing one is not a ratchet
/// regression — without the line, an exit-0 run that printed findings reads as a green gate,
/// and an exit-1 run reads as a newly-broken pin.
///
/// # Errors
///
/// Returns [`CliError::Failed`] (after the verdict) when `findings` is non-zero.
pub(crate) fn grade_narrowed_strictly(
    narrowed: &[&'static str],
    noun: &str,
    findings: usize,
) -> Result<(), CliError> {
    let flags = narrowed.join(" / ");
    if findings == 0 {
        println!(
            "\n✓ no {noun}s — {flags} narrows this run, so the snapshot was NOT consulted. \
             Graded STRICTLY instead: here every {noun} fails, pinned or not."
        );
        return Ok(());
    }
    eprintln!(
        "\n✗ {findings} file(s) with {noun}s — {flags} narrows this run, so the snapshot was \
         NOT consulted. Graded STRICTLY instead: every {noun} above fails, pinned or not."
    );
    Err(CliError::Failed)
}

/// The `--update` epilogue for the never-pinnable class: a PANIC key is deliberately NOT
/// written (see [`SnapshotKey::is_pinnable`]), so a re-pin with crashes present must still
/// exit non-zero — otherwise `--update` would read as having laundered them. `noun` matches
/// [`Ratchet::write_pinned`]'s; `subject` names the audit's injected thing ("an injected
/// directive", "a blank in a gap", "a comment in a gap").
///
/// # Errors
///
/// Returns [`CliError::Failed`] (after the user-facing message) when `panics > 0`.
#[cfg(feature = "comment_check")]
pub(crate) fn report_unpinned_panics(
    panics: usize,
    noun: &str,
    subject: &str,
) -> Result<(), CliError> {
    if panics == 0 {
        return Ok(());
    }
    eprintln!(
        "\n✗ {panics} PANIC {noun}(s) were NOT pinned — {subject} must never crash the \
         formatter, so the gate will keep failing until they are fixed."
    );
    Err(CliError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy key exercising the generic ratchet in isolation from any real
    /// consumer: `(rank, name)` renders `rank\tname`, and rank 9 is "unpinnable"
    /// (the stand-in for gap's PANIC).
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
    struct ToyKey {
        rank: u8,
        name: String,
    }

    impl SnapshotKey for ToyKey {
        fn to_line(&self) -> String {
            format!("{}\t{}", self.rank, self.name)
        }
        fn from_line(line: &str) -> Option<Self> {
            let mut cols = line.split('\t');
            let rank = cols.next()?.parse().ok()?;
            let name = cols.next()?.to_string();
            Some(Self { rank, name })
        }
        fn is_pinnable(&self) -> bool {
            self.rank != 9
        }
    }

    fn key(rank: u8, name: &str) -> ToyKey {
        ToyKey {
            rank,
            name: name.to_string(),
        }
    }

    /// A ratchet over a fresh, uniquely-named temp file, cleaned up on drop.
    struct TempRatchet {
        ratchet: Ratchet,
    }
    impl TempRatchet {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NONCE: AtomicU32 = AtomicU32::new(0);
            let n = NONCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("tsv_ratchet_test_{}_{n}.txt", std::process::id()));
            Self {
                ratchet: Ratchet::new(path, "# header\n", "re-pin cmd"),
            }
        }
    }
    impl Drop for TempRatchet {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.ratchet.path);
        }
    }

    /// render → parse must round-trip over the pinnable keys, in `Ord` order.
    #[test]
    fn render_and_parse_round_trip_pinnable_keys() {
        let found: BTreeSet<ToyKey> = [key(0, "b"), key(0, "a"), key(1, "c")]
            .into_iter()
            .collect();
        let r = Ratchet::new(PathBuf::from("/unused"), "# header\n", "cmd");
        let rendered = r.render(&found);
        // Header, the count stamp, then pinnable keys in Ord order (rank, then name).
        assert_eq!(rendered, "# header\n# shapes: 3\n0\ta\n0\tb\n1\tc\n");
        let parsed: BTreeSet<ToyKey> = rendered
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .filter_map(ToyKey::from_line)
            .collect();
        assert_eq!(parsed, found);
    }

    /// An unpinnable key is never rendered, never a pinnable key — but it is
    /// counted, so it fails the gate. The corpus cannot grade this (a real
    /// snapshot has no unpinnable lines).
    #[test]
    fn unpinnable_key_is_not_written_and_fails() {
        let t = TempRatchet::new();
        // Pin one pinnable key.
        let pinned: BTreeSet<ToyKey> = std::iter::once(key(0, "a")).collect();
        t.ratchet.write(&pinned).expect("write");

        // The unpinnable key must not reach the file.
        let both: BTreeSet<ToyKey> = [key(0, "a"), key(9, "boom")].into_iter().collect();
        let rendered = t.ratchet.render(&both);
        assert_eq!(
            rendered, "# header\n# shapes: 1\n0\ta\n",
            "only the pinnable key is written, and the stamp counts just it"
        );

        // Grading the found set against the just-written snapshot: no new/stale
        // (the pinnable key matches), but one unpinnable ⇒ the gate does not hold.
        let diff = t.ratchet.grade(&both).expect("grade");
        assert!(diff.new.is_empty(), "no new: {:?}", diff.new);
        assert!(diff.stale.is_empty(), "no stale: {:?}", diff.stale);
        assert_eq!(diff.unpinnable, 1);
        assert_eq!(diff.known, 1);
        assert!(!diff.holds(), "an unpinnable key fails the gate");
    }

    /// A new key and a stale key each break the ratchet; a clean diff holds.
    #[test]
    fn grade_detects_new_and_stale() {
        let t = TempRatchet::new();
        t.ratchet
            .write(&[key(0, "a"), key(1, "b")].into_iter().collect())
            .expect("write");

        // Same set ⇒ holds.
        let same: BTreeSet<ToyKey> = [key(0, "a"), key(1, "b")].into_iter().collect();
        assert!(t.ratchet.grade(&same).expect("grade").holds());

        // Drop `b`, add `c`.
        let shifted: BTreeSet<ToyKey> = [key(0, "a"), key(2, "c")].into_iter().collect();
        let diff = t.ratchet.grade(&shifted).expect("grade");
        assert_eq!(diff.new, vec![key(2, "c")], "c is new");
        assert_eq!(diff.stale, vec![key(1, "b")], "b is stale");
        assert!(!diff.holds());
    }

    /// A missing snapshot is a hard read error, never a silent empty set.
    #[test]
    fn missing_snapshot_is_a_hard_error() {
        let r = Ratchet::new(
            PathBuf::from("/nonexistent/tsv_ratchet_missing.txt"),
            "# header\n",
            "cmd",
        );
        assert_eq!(r.parse_known::<ToyKey>().err(), Some(CliError::Failed));
    }

    /// A narrowed run of a zero-target audit still GRADES — the notice is a verdict, not a
    /// skip. The counterpart to [`print_ratchet_skipped`], which always exits 0.
    #[test]
    fn a_narrowed_strict_grade_fails_on_any_finding() {
        assert_eq!(
            grade_narrowed_strictly(&["explicit paths"], "toy", 0),
            Ok(())
        );
        assert_eq!(
            grade_narrowed_strictly(&["explicit paths"], "toy", 1).err(),
            Some(CliError::Failed),
            "a finding off the default corpus is news, snapshot or no snapshot"
        );
    }
}
