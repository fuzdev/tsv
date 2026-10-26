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

/// RAII core of both brackets: installs `hook` on construction and puts the hook it
/// displaced back on drop — including on an early-return error path, which a hand-rolled
/// take/set pair leaks.
///
/// The drop replaces whatever hook is installed by then. Where that is still the guard's
/// own — guards nested on one thread, whose `Drop` runs in reverse construction order — it
/// restores the displaced hook itself, the same allocation, so an install/restore cycle
/// leaves nothing behind. Where it is not (another guard installed over this one and was
/// not dropped first), the displaced hook may still be held by the hook that replaced it,
/// and the drop installs a forwarder to it instead.
struct HookGuard {
    prev: Option<std::sync::Arc<PanicHook>>,
}

impl HookGuard {
    fn install(hook: PanicHook) -> Self {
        Self::install_over(|_| hook)
    }

    /// Install the hook `make` builds from the displaced one, which it may forward to.
    fn install_over(make: impl FnOnce(std::sync::Arc<PanicHook>) -> PanicHook) -> Self {
        #[cfg(test)]
        assert!(
            HOLDS_HOOK_TEST_LOCK.get(),
            "a test that installs a panic hook holds `hook_test_lock` for the whole test"
        );
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
        let Some(prev) = self.prev.take() else {
            return;
        };
        // the installed hook goes first: a forwarding one holds the other handle on `prev`
        drop(std::panic::take_hook());
        match std::sync::Arc::try_unwrap(prev) {
            Ok(hook) => std::panic::set_hook(hook),
            Err(shared) => std::panic::set_hook(Box::new(move |info| shared(info))),
        }
    }
}

/// The lock every test that installs a panic hook holds for its whole run — directly, or
/// through code that installs one ([`SuppressedPanicHook`], [`CapturingPanicHook`]). The
/// hook is process-global, so two such tests on `cargo test`'s threads would each install
/// over the other and restore the other's hook. `HookGuard` asserts that its thread
/// holds the lock.
#[cfg(test)]
pub(crate) fn hook_test_lock() -> HookTestLock {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // a test that failed holding the lock poisoned it, and its failure is reported already
    let guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    HOLDS_HOOK_TEST_LOCK.set(true);
    HookTestLock { _guard: guard }
}

/// [`hook_test_lock`]'s hold, released on drop.
#[cfg(test)]
pub(crate) struct HookTestLock {
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for HookTestLock {
    fn drop(&mut self) {
        HOLDS_HOOK_TEST_LOCK.set(false);
    }
}

#[cfg(test)]
thread_local! {
    /// Whether this thread holds [`hook_test_lock`].
    static HOLDS_HOOK_TEST_LOCK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// RAII: the installed panic hook is replaced with a no-op on construction and put back on
/// drop, as `HookGuard` puts a displaced hook back.
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
/// of printed — for a caller that reports the panics it triggers with their location. Puts
/// the displaced hook back on drop, like [`SuppressedPanicHook`].
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

    /// The panic hook is process-global: every test here, like every other test that
    /// installs one, holds [`hook_test_lock`]. A test that holds it can still see an
    /// unrelated test's panic, so the sentinel counts only panics carrying this module's
    /// marker.
    #[test]
    fn the_hook_is_suppressed_then_restored_and_payloads_still_read() {
        const MARKER: &str = "panic_hook.rs sentinel";
        static SEEN: AtomicUsize = AtomicUsize::new(0);

        let _lock = hook_test_lock();
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

            // the poll's count is back to zero once `catch_panic` returns, so a later panic
            // on this thread, caught by no `catch_panic`, is not recorded
            let bare = std::panic::catch_unwind(|| panic!("{MARKER} bare"));
            assert!(bare.is_err());
            assert_eq!(
                SEEN.load(Ordering::SeqCst),
                2,
                "a panic on the catching thread after the catch reaches the displaced hook"
            );
            assert_eq!(take_last_panic(), None, "and leaves no record");

            let elsewhere = std::thread::spawn(|| panic!("{MARKER} elsewhere")).join();
            assert!(elsewhere.is_err());
            assert_eq!(
                SEEN.load(Ordering::SeqCst),
                3,
                "a panic outside every catch reaches the displaced hook"
            );
        }

        let restored = std::panic::catch_unwind(|| panic!("{MARKER} restored"));
        assert!(restored.is_err());
        assert_eq!(
            SEEN.load(Ordering::SeqCst),
            4,
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
            4,
            "an unwind-dropped guard leaves its hook installed"
        );

        std::panic::set_hook(displaced);
    }

    /// A guard's drop puts back the hook it displaced itself, not a wrapper around it:
    /// after any number of install/restore cycles, of either bracket, the installed hook is
    /// the original allocation.
    #[test]
    fn install_restore_cycles_put_back_the_displaced_hook_itself() {
        // the address of a hook's closure; a capture keeps it off the zero-size dangling
        // pointer every capture-free closure shares
        fn address(hook: &PanicHook) -> *const () {
            std::ptr::from_ref::<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send>(&**hook)
                .cast::<()>()
        }

        let _lock = hook_test_lock();
        let displaced = std::panic::take_hook();
        let tag = 7_u64;
        let original: PanicHook = Box::new(move |_| {
            std::hint::black_box(tag);
        });
        let at = address(&original);
        std::panic::set_hook(original);

        for _ in 0..3 {
            drop(SuppressedPanicHook::install());
            drop(CapturingPanicHook::install_for_catches());
            drop(CapturingPanicHook::install());
        }
        {
            let _outer = CapturingPanicHook::install_for_catches();
            let _inner = SuppressedPanicHook::install();
        }

        let back = std::panic::take_hook();
        assert_eq!(
            address(&back),
            at,
            "the restored hook is the one the first guard displaced"
        );
        std::panic::set_hook(displaced);
    }
}
