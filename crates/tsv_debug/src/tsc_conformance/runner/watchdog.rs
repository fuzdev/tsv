use super::*;

/// Per-test wall-clock budget for the [`TestWatchdog`]. The full ~12k-test
/// sweep runs in seconds, so a single test at 60 s is pathological with huge
/// margin (~10⁴× the mean) — the limit exists to convert a *hang* into a loud
/// named failure, not to police slow tests.
const WATCHDOG_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// The sweep's hang watchdog — the wall-clock half of the "watchdog
/// independent of ported budgets" requirement. `catch_unwind` converts panics
/// into per-test buckets, but a **hang** (a mis-ported budget at P3, a parser
/// loop) would otherwise freeze the gate silently. The worker heartbeats each
/// test's name + start; a monitor thread checks ~1 Hz and, past
/// [`WATCHDOG_LIMIT`], prints the offending test and exits the process (a hung
/// thread cannot be killed safely — a loud named exit is the correct failure).
/// The instruction-count half (budget-arithmetic cross-check) rides P3 with
/// the budgets themselves.
pub(super) struct TestWatchdog {
    /// `(current test relative_path, its start)`; `None` after `finish`.
    current: std::sync::Arc<std::sync::Mutex<Option<(String, Instant)>>>,
}

impl TestWatchdog {
    pub(super) fn spawn() -> TestWatchdog {
        let current: std::sync::Arc<std::sync::Mutex<Option<(String, Instant)>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let monitor = std::sync::Arc::clone(&current);
        // Detached monitor: exits within a tick of the sweep clearing the slot
        // (or dies with the process — it holds nothing that needs cleanup).
        drop(
            std::thread::Builder::new()
                .name("tsc-watchdog".to_string())
                .spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        let guard = monitor
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let Some((test, start)) = guard.as_ref() else {
                            return; // sweep finished
                        };
                        if start.elapsed() > WATCHDOG_LIMIT {
                            eprintln!(
                                "tsc_conformance watchdog: test {test:?} exceeded {}s — a hang \
                                 (mis-ported budget / parser loop); aborting the run",
                                WATCHDOG_LIMIT.as_secs()
                            );
                            std::process::exit(3);
                        }
                    }
                }),
        );
        TestWatchdog { current }
    }

    /// Heartbeat: the sweep is entering `test` now.
    pub(super) fn enter(&self, test: &str) {
        *self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((test.to_string(), Instant::now()));
    }

    /// The sweep is done — clear the slot so the monitor thread exits.
    pub(super) fn finish(&self) {
        *self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}
