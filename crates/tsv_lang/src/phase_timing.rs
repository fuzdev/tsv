//! Opt-in render timing for `tsv_debug profile` (`_internal_phase_timing` feature, off by
//! default).
//!
//! `profile`'s `format` column is the doc build and the render together, and no caller can
//! time them apart: a Svelte document renders once per root node, and the CSS printer
//! renders piece by piece as it walks. All of those renders go through the four output
//! entry points in `doc`, so the time inside them is the render, and the rest of a format
//! is the doc build. A render that happens while a doc is being built (an embedded
//! `<style>`) counts as render.
//!
//! The call sites are `#[cfg]`-gated, so with the feature off no shipped code changes and
//! [`take_render_time`] reads zero. With it on, every render pays two clock reads, charged
//! to the `format` column: small beside one render of a TypeScript document, and a visible
//! share of a CSS format, which renders per declaration. Read the CSS split as a rough one.
//!
//! Measurement renders (`arena_measure_doc_flat_resolved`) are not timed: their output is
//! discarded, and they are part of deciding a layout.
//!
//! Per thread, like the other diagnostic seams: the profiler formats and reads on one
//! thread.

use std::time::Duration;

/// Whether render timing is compiled in. Without it [`take_render_time`] reads zero, so a
/// report must not print a split.
pub const ENABLED: bool = cfg!(feature = "_internal_phase_timing");

#[cfg(feature = "_internal_phase_timing")]
mod enabled {
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    thread_local! {
        static RENDER: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static DEPTH: Cell<u32> = const { Cell::new(0) };
    }

    /// Times one render until dropped. Only the outermost span of a nest is charged, so a
    /// render inside a render counts once.
    pub struct RenderSpan {
        start: Option<Instant>,
    }

    /// Open a render span.
    #[must_use]
    pub fn render_span() -> RenderSpan {
        let depth = DEPTH.get();
        DEPTH.set(depth + 1);
        RenderSpan {
            start: (depth == 0).then(Instant::now),
        }
    }

    impl Drop for RenderSpan {
        fn drop(&mut self) {
            DEPTH.set(DEPTH.get() - 1);
            if let Some(start) = self.start {
                RENDER.set(RENDER.get() + start.elapsed());
            }
        }
    }

    pub(super) fn take() -> Duration {
        RENDER.replace(Duration::ZERO)
    }
}

#[cfg(feature = "_internal_phase_timing")]
pub use enabled::{RenderSpan, render_span};

/// The render time this thread has accumulated since the last call, and reset it.
#[must_use]
pub fn take_render_time() -> Duration {
    #[cfg(feature = "_internal_phase_timing")]
    {
        enabled::take()
    }
    #[cfg(not(feature = "_internal_phase_timing"))]
    {
        Duration::ZERO
    }
}
