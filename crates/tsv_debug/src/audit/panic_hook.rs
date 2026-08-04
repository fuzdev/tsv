//! The **suppressed-panic-hook** bracket every audit that formats under
//! `catch_unwind` installs for the duration of its corpus walk.
//!
//! An audit that catches a formatter panic and buckets it has already recorded
//! the finding; the default hook then prints a full `thread '…' panicked at …`
//! block *per panicking file* on top of it. On one seed that is noise, on a
//! corpus of them it buries the report the audit exists to print — so the hook
//! comes off for the walk and goes back on after it.
//!
//! ⚠️ Suppression is only sound for a caller that records the panic itself.
//! [`sweep`](crate::audit::sweep) counts the panic AND keeps a bounded sample of
//! the inputs that produced it, and the injection audits key a PANIC finding to
//! its site; a caller that dropped the hook without recording would turn a
//! crash into silence, which is strictly worse than the noise.
//!
//! One definition, two consumers (`ArmedRun` in `audit::parallel` holds one —
//! no intra-doc link, that module is feature-gated and this one is not — and
//! `sweep_pristine_armed` installs one), because a hook left installed on an
//! error path is invisible until the run that needed it.

/// The boxed hook `std::panic::take_hook` hands back — held for the restore on drop.
type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// RAII: the default panic hook is replaced with a no-op on construction and
/// restored on drop — including on an early-return error path, which a
/// hand-rolled take/set pair leaks.
///
/// Nesting is safe: each guard restores exactly the hook it displaced, and
/// `Drop` runs in reverse construction order.
pub(crate) struct SuppressedPanicHook {
    prev: Option<PanicHook>,
}

impl SuppressedPanicHook {
    pub(crate) fn install() -> Self {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self { prev: Some(prev) }
    }
}

impl Drop for SuppressedPanicHook {
    fn drop(&mut self) {
        if let Some(hook) = self.prev.take() {
            std::panic::set_hook(hook);
        }
    }
}

/// The panic payload's message, for a caller recording a caught panic.
///
/// `catch_unwind` hands back a `Box<dyn Any>` whose payload is a `&str` for a
/// literal `panic!`/`assert!` and a `String` for a formatted one; anything else
/// (a non-string payload) has no text to report. Suppressing the hook removes
/// the only other place that text would have appeared, so a recorder that
/// skipped this would drop it entirely.
///
/// Deliberately NOT shared with `tsc_conformance`'s `panic_payload_message`,
/// which asks the same question: that module is declared under BOTH crate roots
/// (`lib.rs` and `main.rs`) while `audit` is bin-only, so a call into here would
/// not compile in the lib build. Unifying them means promoting this module to
/// the crate root first. The sentinel string is kept identical to that one so
/// the two at least read as one answer.
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return s;
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.as_str();
    }
    "(non-string panic payload)"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Both properties in ONE test, deliberately: the panic hook is
    /// process-global, so a second test installing its own would race this one
    /// under `cargo test`'s thread pool. For the same reason the sentinel
    /// counts only panics carrying this module's marker — an unrelated
    /// concurrent panic elsewhere in the suite must not be mistaken for ours.
    #[test]
    fn the_hook_is_suppressed_then_restored_and_payloads_still_read() {
        const MARKER: &str = "panic_hook.rs sentinel";
        static SEEN: AtomicUsize = AtomicUsize::new(0);

        let displaced = std::panic::take_hook();
        std::panic::set_hook(Box::new(|info| {
            if panic_message(info.payload()).contains(MARKER) {
                SEEN.fetch_add(1, Ordering::SeqCst);
            }
        }));

        {
            let _guard = SuppressedPanicHook::install();

            let literal = std::panic::catch_unwind(|| panic!("{MARKER} literal"));
            assert_eq!(
                panic_message(literal.expect_err("panicked").as_ref()),
                format!("{MARKER} literal")
            );
            let formatted = std::panic::catch_unwind(|| panic!("{MARKER} formatted {}", 1_u8));
            assert_eq!(
                panic_message(formatted.expect_err("panicked").as_ref()),
                format!("{MARKER} formatted 1")
            );
            let other = std::panic::catch_unwind(|| std::panic::panic_any(7_u8));
            assert_eq!(
                panic_message(other.expect_err("panicked").as_ref()),
                "(non-string panic payload)"
            );

            assert_eq!(
                SEEN.load(Ordering::SeqCst),
                0,
                "no hook output while suppressed — the whole point"
            );
        }

        let after = std::panic::catch_unwind(|| panic!("{MARKER} after"));
        assert!(after.is_err());
        assert_eq!(
            SEEN.load(Ordering::SeqCst),
            1,
            "the displaced hook must be back after the guard drops"
        );

        std::panic::set_hook(displaced);
    }
}
