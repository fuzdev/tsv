//! The tape exports' sink: the thread's parked `tsv_lang::tape::TapeSink`, which the language
//! crates' wire writers write the tape into directly (their `_tape` tape copies,
//! `docs/tape_format.md`) and [`parse_tape!`](crate::parse_tape) lends a binding the finished
//! words of. Behind this crate's `tape` feature.

use std::cell::Cell;

/// The sink and why it refused a tape — what a binding's `parse_tape!` maps through
/// `$map_tape_err` — re-exported, as the bindings take no `tsv_lang` edge for them.
pub use tsv_lang::tape::{SinkError, TapeSink};

thread_local! {
    /// This thread's parked sink, between [`with_tape_sink`] calls.
    static TAPE_SINK: Cell<Option<Box<TapeSink>>> = const { Cell::new(None) };
}

/// Runs `f` with this thread's tape sink, whose buffers keep their capacity from call to call
/// — the bindings' tape exports run once per file in a loop, as their parse exports do over
/// the stores.
///
/// Take and park, never a held borrow: the sink leaves the slot for the call and goes back
/// after, so a WASM trap inside `f` (which runs no `Drop`) leaves the slot empty — the next
/// call starts a fresh sink — rather than locked, and a call nested inside `f` or one after a
/// caught panic gets a fresh one too. Parked uncapped, as the bindings park their stores and
/// doc arena: a thread keeps what its largest input grew. A call from a thread-local
/// destructor, once the slot is gone, runs on a fresh sink and parks nothing.
pub fn with_tape_sink<R>(f: impl FnOnce(&mut TapeSink) -> R) -> R {
    let mut sink = TAPE_SINK
        .try_with(Cell::take)
        .ok()
        .flatten()
        .unwrap_or_default();
    let result = f(&mut sink);
    let _ = TAPE_SINK.try_with(|slot| slot.set(Some(sink)));
    result
}
