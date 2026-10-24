//! The **panic-hook brackets**: [`SuppressedPanicHook`], which every audit that formats
//! under `catch_unwind` installs for the duration of its corpus walk, and
//! [`CapturingPanicHook`], which the fuzzers (`fuzz`, `compile_fuzz`) and the fixture
//! validators (`fixtures_validate`, `compile_fixtures_validate`, `fixtures_tests`) install to
//! report each panic's text and location through [`take_last_panic`] — the validators by way
//! of [`catch_panic`], their per-fixture isolation, recording only the panics it catches
//! ([`CapturingPanicHook::install_for_catches`]).
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
    prev: Option<std::sync::Arc<PanicHook>>,
}

impl HookGuard {
    fn install(hook: PanicHook) -> Self {
        Self::install_over(|_| hook)
    }

    /// Install the hook `make` builds from the displaced one, which it may forward to.
    fn install_over(make: impl FnOnce(std::sync::Arc<PanicHook>) -> PanicHook) -> Self {
        let prev = std::sync::Arc::new(std::panic::take_hook());
        std::panic::set_hook(make(std::sync::Arc::clone(&prev)));
        Self { prev: Some(prev) }
    }
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        // `set_hook` panics on a panicking thread, and a panic in a drop during an unwind
        // aborts the process: a guard dropped by an unwind leaves its hook in place, and
        // the panic it has already seen goes on to wherever it was going.
        if std::thread::panicking() {
            return;
        }
        if let Some(hook) = self.prev.take() {
            std::panic::set_hook(Box::new(move |info| hook(info)));
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

    /// How many [`catch_panic`] polls are running on this thread: a panic raised while one
    /// is will be caught, and only such a panic is recorded by
    /// [`CapturingPanicHook::install_for_catches`].
    static CATCHING: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// RAII: while alive, a panic's `Display` text is recorded for [`take_last_panic`] instead
/// of printed — for a caller that reports the panics it triggers with their location. Restores
/// the displaced hook on drop, like [`SuppressedPanicHook`].
pub(crate) struct CapturingPanicHook {
    _guard: HookGuard,
}

impl CapturingPanicHook {
    /// Record every panic, on every thread — for a caller that catches each one it can
    /// raise (the fuzzers, whose every case runs under `catch_unwind`).
    pub(crate) fn install() -> Self {
        Self {
            _guard: HookGuard::install(Box::new(record_panic)),
        }
    }

    /// Record only a panic raised inside a [`catch_panic`] on its own thread, and hand
    /// every other panic to the displaced hook — so a panic no catch will report (a
    /// sidecar actor's, a runtime worker's outside any fixture) is still printed.
    pub(crate) fn install_for_catches() -> Self {
        Self {
            _guard: HookGuard::install_over(|prev| {
                Box::new(move |info| {
                    if CATCHING.get() > 0 {
                        record_panic(info);
                    } else {
                        prev(info);
                    }
                })
            }),
        }
    }
}

/// [`CapturingPanicHook`]'s record: this thread's last panic, as its `Display` text.
fn record_panic(info: &std::panic::PanicHookInfo<'_>) {
    LAST_PANIC.with(|c| *c.borrow_mut() = Some(info.to_string()));
}

/// RAII: one [`catch_panic`] poll running on this thread, counted in `CATCHING` — the
/// count drops on return and on unwind alike, and the hook runs before the unwind starts.
struct CatchingPoll;

impl CatchingPoll {
    fn enter() -> Self {
        CATCHING.set(CATCHING.get() + 1);
        Self
    }
}

impl Drop for CatchingPoll {
    fn drop(&mut self) {
        CATCHING.set(CATCHING.get() - 1);
    }
}

/// Take this thread's last panic text [`CapturingPanicHook`] recorded, clearing the slot.
pub(crate) fn take_last_panic() -> Option<String> {
    LAST_PANIC.with(|c| c.borrow_mut().take())
}

/// The report for a panic just caught on this thread: [`CapturingPanicHook`]'s record of it
/// (`panicked at <file>:<line>:<col>:` and the message) when one is installed, else the
/// payload's message alone ([`panic_message`]).
///
/// Call it straight after the catch, on the thread that caught. The record is the thread's
/// LAST panic, which a caught one always is at that point, unless the catch was handed a
/// payload no panic recorded (`resume_unwind`); a record that does not end in this payload's
/// message is therefore not taken as its location.
pub(crate) fn caught_panic_report(payload: &(dyn std::any::Any + Send)) -> String {
    let message = panic_message(payload);
    take_last_panic()
        .filter(|record| record.ends_with(message))
        .unwrap_or_else(|| message.to_string())
}

/// Await `future` with its panics caught: `Err` carries [`caught_panic_report`]'s text —
/// the location too while a [`CapturingPanicHook`] is installed. The per-item isolation the
/// fixture validators share, so one panicking fixture is that fixture's failure.
///
/// `catch_unwind` on a future wraps each poll, so the panic is caught on the thread that
/// polled — the one whose record [`caught_panic_report`] reads.
pub(crate) async fn catch_panic<F: Future>(future: F) -> Result<F::Output, String> {
    use futures_util::FutureExt;
    let mut future = std::pin::pin!(future);
    // `AssertUnwindSafe`: a caller reads nothing the future touched after a panic, only the
    // report — the replacement result is built from the caller's own data.
    std::panic::AssertUnwindSafe(std::future::poll_fn(move |cx| {
        let _catching = CatchingPoll::enter();
        future.as_mut().poll(cx)
    }))
    .catch_unwind()
    .await
    .map_err(|payload| caught_panic_report(payload.as_ref()))
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

    /// Every property in ONE test, deliberately: the panic hook is
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

        {
            let _guard = CapturingPanicHook::install_for_catches();
            let caught = futures_util::FutureExt::now_or_never(catch_panic(async {
                panic!("{MARKER} caught");
            }))
            .expect("the future is ready");
            let report = caught.expect_err("panicked");
            assert!(report.starts_with("panicked at "), "{report}");
            assert!(report.ends_with(&format!("{MARKER} caught")), "{report}");
            assert_eq!(
                SEEN.load(Ordering::SeqCst),
                1,
                "a caught panic is recorded, not printed"
            );

            let elsewhere = std::thread::spawn(|| panic!("{MARKER} elsewhere")).join();
            assert!(elsewhere.is_err());
            assert_eq!(
                SEEN.load(Ordering::SeqCst),
                2,
                "a panic outside every catch reaches the displaced hook"
            );
        }

        let restored = std::panic::catch_unwind(|| panic!("{MARKER} restored"));
        assert!(restored.is_err());
        assert_eq!(
            SEEN.load(Ordering::SeqCst),
            3,
            "the hook a forwarding guard displaced must be back after it drops"
        );

        // A guard dropped by an unwind leaves its hook in place rather than restoring
        // (`set_hook` panics on a panicking thread, and that panic would abort): the
        // process survives, and the suppressed hook is still the installed one.
        let unwound = std::panic::catch_unwind(|| {
            let _guard = SuppressedPanicHook::install();
            panic!("{MARKER} unwound");
        });
        assert!(unwound.is_err());
        let later = std::panic::catch_unwind(|| panic!("{MARKER} later"));
        assert!(later.is_err());
        assert_eq!(
            SEEN.load(Ordering::SeqCst),
            3,
            "an unwind-dropped guard leaves its hook installed"
        );

        std::panic::set_hook(displaced);
    }
}
