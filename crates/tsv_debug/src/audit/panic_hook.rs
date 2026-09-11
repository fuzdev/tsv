//! The **panic-hook brackets**: [`SuppressedPanicHook`], which every audit that formats
//! under `catch_unwind` installs for the duration of its corpus walk, and
//! [`CapturingPanicHook`], which the fuzzers (`fuzz`, `compile_fuzz`) install to report each
//! panic's text and location through [`take_last_panic`].
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
//! One definition for every caller — `ArmedRun` in `audit::parallel` holds one
//! (no intra-doc link: that module is feature-gated and this one is not), the
//! pristine sweep installs one, and `authoring_audit`, `paren_audit` and
//! `tsc_conformance`'s crash probe take one directly — because a hook left
//! installed on an error path is invisible until the run that needed it.

/// The boxed hook `std::panic::take_hook` hands back — held for the restore on drop.
type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// RAII core of both brackets: installs `hook` on construction and restores the hook it
/// displaced on drop — including on an early-return error path, which a hand-rolled take/set
/// pair leaks.
///
/// Nesting is safe: each guard restores exactly the hook it displaced, and `Drop` runs in
/// reverse construction order.
struct HookGuard {
    prev: Option<PanicHook>,
}

impl HookGuard {
    fn install(hook: PanicHook) -> Self {
        let prev = std::panic::take_hook();
        std::panic::set_hook(hook);
        Self { prev: Some(prev) }
    }
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        if let Some(hook) = self.prev.take() {
            std::panic::set_hook(hook);
        }
    }
}

/// RAII: the default panic hook is replaced with a no-op on construction and restored on
/// drop.
pub(crate) struct SuppressedPanicHook {
    _guard: HookGuard,
}

impl SuppressedPanicHook {
    pub(crate) fn install() -> Self {
        Self {
            _guard: HookGuard::install(Box::new(|_| {})),
        }
    }
}

thread_local! {
    /// This thread's most recent panic, as its `Display` text (message and location), recorded
    /// by [`CapturingPanicHook`]. Per-thread, and `catch_unwind` returns on the thread that
    /// panicked, so a concurrent caller reads its own panic.
    static LAST_PANIC: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// RAII: while alive, every panic's `Display` text is recorded for [`take_last_panic`] instead
/// of printed — for a caller that reports the panics it triggers with their location. Restores
/// the displaced hook on drop, like [`SuppressedPanicHook`].
pub(crate) struct CapturingPanicHook {
    _guard: HookGuard,
}

impl CapturingPanicHook {
    pub(crate) fn install() -> Self {
        Self {
            _guard: HookGuard::install(Box::new(|info| {
                LAST_PANIC.with(|c| *c.borrow_mut() = Some(info.to_string()));
            })),
        }
    }
}

/// Take this thread's last panic text [`CapturingPanicHook`] recorded, clearing the slot.
pub(crate) fn take_last_panic() -> Option<String> {
    LAST_PANIC.with(|c| c.borrow_mut().take())
}

/// The panic payload's message, for a caller recording a caught panic.
///
/// `catch_unwind` hands back a `Box<dyn Any>` whose payload is a `&str` for a
/// literal `panic!`/`assert!` and a `String` for a formatted one; anything else
/// (a non-string payload) has no text to report. Suppressing the hook removes
/// the only other place that text would have appeared, so a recorder that
/// skipped this would drop it entirely.
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
