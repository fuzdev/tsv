//! Opt-in perf census counters (`census` feature, off by default).
//!
//! A perf session that wants to know how often a loop runs, how long its runs are,
//! or how often a predicate returns true adds `census::add` calls at the sites it is
//! pricing, builds `tsv_debug` with `--features census`, and reads the report
//! `tsv_debug profile` / `json_profile` print after their tables. The counters are
//! the *session's*, not the tree's — the call sites are throwaway and come back out
//! before anything is measured; only this module and the report hook are durable.
//!
//! **With the feature off every entry point here is an empty `#[inline]`
//! function and [`LABELS`] is empty**, so an instrumented tree left un-reverted is
//! provably inert in a default build (`objcopy -O binary --only-section=.text` +
//! `cmp` against the base binary, which is the neutrality check a perf session owes
//! anyway). That is the point of the gate: the census and the measurement can share
//! one working tree instead of a revert.
//!
//! ⚠️ **A census sizes the INSTRUCTION channel.** It says how much work a site does,
//! never what that work costs in cycles — an out-of-order machine hides a great deal
//! of it. Size a lever with the layout group, not with this.
//!
//! ⚠️ **Census the axis the transformation's cost actually runs on.** For a scan that
//! means the run-length distribution *with the byte mass per bucket* and the region's
//! trailing run, not a mean; for a predicate it means the SUCCESS rate, not just the
//! call count. Both rules were learned by getting them wrong.
//!
//! ```ignore
//! // in the crate being priced:
//! tsv_lang::census::add(0, 1);                     // one call
//! tsv_lang::census::add(1, run_len as u64);        // its bytes
//! tsv_lang::census::hit(2, matched.is_some());     // and whether it found anything
//! ```
//!
//! Then name them in [`LABELS`] and run
//! `cargo run --release --features census -p tsv_debug -- profile <corpus>`.
//! Unlabelled non-zero counters still report, as `c<index>`.

/// Number of counters. Raise it if a session needs more; the array is static and
/// zero-initialized, so unused slots cost nothing but the `.bss` bytes.
pub const N: usize = 64;

/// Names for the counters the current session is using, in report order.
///
/// Left **empty in the committed tree** — a session fills it in beside its call
/// sites and empties it again with them. Counters missing here still report under
/// `c<index>` when non-zero, so a forgotten label loses a name, never a number.
pub static LABELS: &[(usize, &str)] = &[];

#[cfg(feature = "census")]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    pub(super) static C: [AtomicU64; super::N] = [const { AtomicU64::new(0) }; super::N];

    #[inline]
    pub fn add(index: usize, n: u64) {
        C[index].fetch_add(n, Relaxed);
    }

    pub fn report() {
        let mut named = [false; super::N];
        for (index, name) in super::LABELS {
            named[*index] = true;
            eprintln!("census,{name},{}", C[*index].load(Relaxed));
        }
        for (index, slot) in C.iter().enumerate() {
            let value = slot.load(Relaxed);
            if value != 0 && !named[index] {
                eprintln!("census,c{index},{value}");
            }
        }
    }
}

#[cfg(not(feature = "census"))]
mod imp {
    #[inline]
    pub fn add(_index: usize, _n: u64) {}

    pub fn report() {}
}

/// Add `n` to counter `index`. A no-op without the `census` feature.
#[inline]
pub fn add(index: usize, n: u64) {
    imp::add(index, n);
}

/// Bump counter `index` by one. A no-op without the `census` feature.
#[inline]
pub fn hit(index: usize) {
    imp::add(index, 1);
}

/// Bump counter `index` by one when `condition` holds — the success-rate shape.
/// A no-op without the `census` feature.
#[inline]
pub fn hit_if(index: usize, condition: bool) {
    if condition {
        imp::add(index, 1);
    }
}

/// Print every labelled counter, then every unlabelled non-zero one, as
/// `census,<name>,<value>` lines on stderr. A no-op without the `census` feature.
pub fn report() {
    imp::report();
}
