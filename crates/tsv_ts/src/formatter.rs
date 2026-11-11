//! [`Formatter`]: [`format()`](crate::format) and [`format_with_goal`](crate::format_with_goal)
//! over arenas kept warm across calls.

use crate::ast::AstStores;
use crate::{Goal, Result, format_in};
use tsv_lang::doc::arena::DocArena;

/// Formats many TypeScript sources on one thread, reusing its allocations from one
/// source to the next.
///
/// [`format()`](crate::format) allocates a fresh doc arena and AST stores for every call and
/// drops them before returning. A `Formatter` owns both and rewinds them instead, keeping
/// the capacity they have grown, so after the first few sources the work allocates almost
/// nothing new — the shape of a driver that formats a directory, or an editor that formats
/// on every save. The output is exactly
/// [`format()`](crate::format)'s (and [`format_with_goal`](crate::format_with_goal)'s) for
/// the same source.
///
/// **The doc arena rewinds at the start of every call**, so nothing one call leaves behind
/// reaches the next — including a call that never returned because it panicked and the
/// panic was caught: the next call starts from a clean doc arena all the same, and from new
/// stores, the old ones having been out of the `Formatter` while the call ran. The stores
/// are parked rewound as each call returns, keeping their capacity.
///
/// Catching that panic takes [`std::panic::AssertUnwindSafe`] around the call — a closure
/// borrowing the `Formatter` mutably is not [`UnwindSafe`](std::panic::UnwindSafe) — and the
/// rewind is what makes the assertion sound.
///
/// A `Formatter` is [`Send`], so it can move to (or be created on) a worker thread, but
/// not [`Sync`]: give each thread its own.
///
/// ```
/// # fn main() -> tsv_ts::Result<()> {
/// use tsv_ts::{Formatter, Goal};
///
/// let mut formatter = Formatter::new();
/// for source in ["const x=42;", "let y = [1,2]"] {
///     let formatted = formatter.format(source)?;
///     assert_eq!(formatted, tsv_ts::format(source)?);
/// }
/// assert!(formatter.format_with_goal("with (a) b;", Goal::Module).is_err());
/// assert_eq!(formatter.format("with (a) b;")?, "with (a) b;\n");
/// # Ok(())
/// # }
/// ```
pub struct Formatter {
    doc: DocArena,
    /// The AST stores between calls: out of the slot while one runs, so a caught panic leaves
    /// nothing half-used to carry into the next.
    stores: Option<AstStores<'static>>,
}

impl Formatter {
    /// A `Formatter` with an empty doc arena and no stores; they grow to the sources it is
    /// given.
    #[must_use]
    pub fn new() -> Self {
        Self {
            doc: DocArena::new(),
            stores: None,
        }
    }

    /// [`format()`](crate::format) on this formatter's arenas: no source type named, so
    /// `Module`, retried at `Script` only if that fails ([Goals](crate#goals)).
    ///
    /// # Errors
    ///
    /// A parse error under both goals (the one [Goals](crate#goals) picks).
    pub fn format(&mut self, source: &str) -> Result<String> {
        self.format_with_optional_goal(source, None)
    }

    /// [`format_with_goal`](crate::format_with_goal) on this formatter's arenas: exactly
    /// `goal`, with no fallback.
    ///
    /// # Errors
    ///
    /// A parse error at `goal`.
    pub fn format_with_goal(&mut self, source: &str, goal: Goal) -> Result<String> {
        self.format_with_optional_goal(source, Some(goal))
    }

    fn format_with_optional_goal(&mut self, source: &str, goal: Option<Goal>) -> Result<String> {
        // The rewind is the contract, and it sits ahead of the work: a caller that caught
        // a panic out of the previous call left this arena mid-document. Its rewind frees
        // nothing (its stores keep their capacity), so it waits for the next call rather
        // than following this one.
        self.doc.reset();
        let mut stores: AstStores<'_> = self.stores.take().unwrap_or_default();
        let formatted = format_in(source, goal, &mut stores, |_| &self.doc);
        self.stores = Some(stores.park());
        formatted
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Formatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Formatter").finish_non_exhaustive()
    }
}

// A driver hands a formatter to each worker thread; pin that it can move to one, so a field
// added later cannot take that away unnoticed.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<Formatter>();
};

#[cfg(test)]
mod tests {
    use super::Formatter;
    use crate::{Goal, format, format_with_goal};

    /// A reused formatter answers every call as the one-shot functions do, across a failed
    /// parse, a goal change and a document much larger than the one before it.
    #[test]
    fn a_reused_formatter_matches_the_one_shot_functions() {
        let large = "const a = [1, 2, 3];\n".repeat(2_000);
        let sources = [
            "const x=42;",
            "const = ;",
            large.as_str(),
            "with (a) b;",
            "let y\r\n= 1",
            "",
        ];
        let mut formatter = Formatter::default();
        for source in sources {
            let reused = formatter.format(source).map_err(|e| format!("{e:#}"));
            let fresh = format(source).map_err(|e| format!("{e:#}"));
            assert_eq!(reused, fresh, "{source:?}");
            for goal in [Goal::Module, Goal::Script] {
                let reused = formatter
                    .format_with_goal(source, goal)
                    .map_err(|e| format!("{e:#}"));
                let fresh = format_with_goal(source, goal).map_err(|e| format!("{e:#}"));
                assert_eq!(reused, fresh, "{source:?} at {goal:?}");
            }
        }
    }
}
