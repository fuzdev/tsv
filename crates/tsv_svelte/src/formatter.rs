//! [`Formatter`]: [`format()`](crate::format) over a doc arena and stores kept warm across
//! calls.

use crate::ast::WarmStores;
use crate::{Result, format_in};
use tsv_lang::doc::arena::DocArena;

/// Formats many Svelte sources on one thread, reusing its allocations from one source to the
/// next.
///
/// [`format()`](crate::format) borrows the thread's parked doc arena and stores (the
/// document's own, TypeScript and CSS halves), keeping each for the next call only while it
/// holds at most 1 MiB ([Memory](crate#memory)). A `Formatter` owns both and rewinds them
/// instead, keeping all the capacity they have grown, so after the first few sources the work
/// allocates almost nothing new — the shape of a driver that formats a directory, or an
/// editor that formats on every save. The output is exactly [`format()`](crate::format)'s
/// for the same source.
///
/// **The doc arena rewinds at the start of every call**, so nothing one call leaves behind
/// reaches the next — including a call that never returned because it panicked and the
/// panic was caught: the next call starts from a clean arena all the same, and from new
/// stores, the old ones having been out of the `Formatter` while the call ran.
///
/// Catching that panic takes [`std::panic::AssertUnwindSafe`] around the call — a closure
/// borrowing the `Formatter` mutably is not [`UnwindSafe`](std::panic::UnwindSafe) — and the
/// rewind is what makes the assertion sound.
///
/// **Nothing is trimmed**: the capacity the largest source so far grew stays with the
/// `Formatter` for as long as it lives — that reuse is the point. Drop it to give the memory
/// back (after an outsized source, say) and start a new one; the output is the same either
/// way.
///
/// A `Formatter` is [`Send`], so it can move to (or be created on) a worker thread, but
/// not [`Sync`]: give each thread its own.
///
/// ```
/// # fn main() -> tsv_svelte::Result<()> {
/// use tsv_svelte::Formatter;
///
/// let mut formatter = Formatter::new();
/// for source in ["<p>x</p>", "<div  class=\"a\">y</div>"] {
///     let formatted = formatter.format(source)?;
///     assert_eq!(formatted, tsv_svelte::format(source)?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct Formatter {
    doc: DocArena,
    /// The document stores between calls, out of the slot while one runs.
    stores: WarmStores,
}

impl Formatter {
    /// A `Formatter` with an empty arena and stores; they grow to the sources it is given.
    #[must_use]
    pub fn new() -> Self {
        Self {
            doc: DocArena::new(),
            stores: WarmStores::default(),
        }
    }

    /// [`format()`](crate::format) on this formatter's doc arena and stores.
    ///
    /// # Errors
    ///
    /// The errors of [`format()`](crate::format).
    pub fn format(&mut self, source: &str) -> Result<String> {
        // The rewind is the contract, and it sits ahead of the work: a caller that caught
        // a panic out of the previous call left this arena mid-document.
        self.doc.reset();
        let doc = &self.doc;
        self.stores
            .with(|stores| format_in(source, stores, |_| doc))
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
    use crate::format;

    /// A reused formatter answers every call as the one-shot function does, across a failed
    /// parse, a refused lone `<CR>`, embedded TypeScript and CSS, and a document much larger
    /// than the one before it.
    #[test]
    fn a_reused_formatter_matches_the_one_shot_function() {
        let large = "<p>x</p>\n".repeat(2_000);
        let sources = [
            "<p>x</p>",
            "<p>{</p>",
            large.as_str(),
            "<div // a\rb\n></div>\n",
            "<script>let x=1</script>\r\n<style>a { color:red }</style>",
            "",
        ];
        let mut formatter = Formatter::default();
        for source in sources {
            let reused = formatter.format(source).map_err(|e| format!("{e:#}"));
            let fresh = format(source).map_err(|e| format!("{e:#}"));
            assert_eq!(reused, fresh, "{source:?}");
        }
    }
}
