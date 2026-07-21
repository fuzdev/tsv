//! Opt-in printer buffer-population sampling — the data source behind
//! `tsv_debug buffer_sizes`' chain/comment metrics.
//!
//! The printer's `SmallVec` inline capacities (`ChainNodeVec`, `ChainGroupVec`,
//! `ChainGroup.nodes`, the leading-comment `CommentVec`) are sizing claims, and
//! a claim in a doc comment drifts; this module samples the real populations at
//! the buffers' construction chokepoints so the inline-`N` choice is graded
//! against measured lengths on any corpus.
//!
//! Compiled in only under the `buffer_stats` cargo feature (off by default,
//! like `debug_lex`), so production builds — and default `tsv_debug` builds,
//! whose profiles must measure production-shaped code — carry no recording. A
//! feature-enabled binary still records nothing until armed via
//! [`set_buffer_stats`] (the `buffer_sizes` command arms it), so other commands
//! in the same binary stay unperturbed. Recording is output-neutral either way.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::chain::{ChainGroupNodesVec, ChainGroupVec, ChainNodeVec};
use super::comments::CommentVec;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Arm or disarm recording process-wide. Recording is per-thread (the sink is a
/// thread-local); the driving command formats on one thread and drains there.
pub fn set_buffer_stats(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

#[inline]
fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// One sampled population per instrumented buffer. Lengths are recorded at the
/// point the buffer's final size is known, one sample per construction.
#[derive(Debug, Default, Clone)]
pub struct BufferStats {
    /// `ChainNodeVec` length per linearized chain (`finalize_chain_nodes`).
    pub chain_nodes: Vec<usize>,
    /// `ChainGroupVec` length per `group_chain_nodes` call.
    pub chain_groups: Vec<usize>,
    /// `ChainGroup.nodes` length per built group (every group of every
    /// `group_chain_nodes` call).
    pub group_nodes: Vec<usize>,
    /// Leading-comment `CommentVec` length per `collect_leading_comments` call
    /// (the statement-gap collector — the buffer type's dominant allocation
    /// site; other `CommentVec` sites are not sampled).
    pub leading_comments: Vec<usize>,
}

/// The instrumented buffer types' *current* inline capacities, read from the
/// types themselves so the `buffer_sizes` report labels can't drift when an
/// `N` is re-tuned. Fields mirror [`BufferStats`].
#[derive(Debug, Clone, Copy)]
pub struct BufferInlineCapacities {
    pub chain_nodes: usize,
    pub chain_groups: usize,
    pub group_nodes: usize,
    pub leading_comments: usize,
}

/// See [`BufferInlineCapacities`].
pub fn inline_capacities() -> BufferInlineCapacities {
    BufferInlineCapacities {
        chain_nodes: ChainNodeVec::new().inline_size(),
        chain_groups: ChainGroupVec::new().inline_size(),
        group_nodes: ChainGroupNodesVec::new().inline_size(),
        leading_comments: CommentVec::new().inline_size(),
    }
}

thread_local! {
    static SINK: RefCell<BufferStats> = RefCell::new(BufferStats::default());
}

pub(crate) fn record_chain_nodes(len: usize) {
    if enabled() {
        SINK.with(|s| s.borrow_mut().chain_nodes.push(len));
    }
}

pub(crate) fn record_chain_groups(len: usize) {
    if enabled() {
        SINK.with(|s| s.borrow_mut().chain_groups.push(len));
    }
}

pub(crate) fn record_group_nodes(len: usize) {
    if enabled() {
        SINK.with(|s| s.borrow_mut().group_nodes.push(len));
    }
}

pub(crate) fn record_leading_comments(len: usize) {
    if enabled() {
        SINK.with(|s| s.borrow_mut().leading_comments.push(len));
    }
}

/// Drain this thread's recorded samples.
pub fn take_buffer_stats() -> BufferStats {
    SINK.with(|s| std::mem::take(&mut *s.borrow_mut()))
}
