//! [`Formatter`]: [`format()`](crate::format) over arenas kept warm across calls.

use crate::{Result, format_in};
use tsv_lang::doc::arena::DocArena;

/// Formats many Svelte sources on one thread, reusing its allocations from one source to the
/// next.
///
/// [`format()`](crate::format) allocates a fresh AST arena and a fresh doc arena for every
/// call and drops both before returning. A `Formatter` owns the pair and rewinds them
/// instead: each rewind keeps the largest block it has grown, so after the first few
/// sources the work allocates almost nothing new — the shape of a driver that formats a
/// directory, or an editor that formats on every save. The output is exactly
/// [`format()`](crate::format)'s for the same source.
///
/// **The arenas rewind at the start of every call**, so nothing one call leaves behind
/// reaches the next — including a call that never returned because it panicked and the
/// panic was caught: the next call starts from a clean arena all the same. The AST arena
/// is rewound once more as each call returns, which releases every block but the largest
/// while the `Formatter` sits idle.
///
/// A `Formatter` is [`Send`], so it can move to (or be created on) a worker thread, but
/// not [`Sync`]: give each thread its own.
///
/// ```
/// use tsv_svelte::Formatter;
///
/// let mut formatter = Formatter::new();
/// for source in ["<p>x</p>", "<div  class=\"a\">y</div>"] {
///     let formatted = formatter.format(source).unwrap();
///     assert_eq!(formatted, tsv_svelte::format(source).unwrap());
/// }
/// ```
pub struct Formatter {
    ast: bumpalo::Bump,
    doc: DocArena,
}

impl Formatter {
    /// A `Formatter` with empty arenas; they grow to the sources it is given.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ast: bumpalo::Bump::new(),
            doc: DocArena::new(),
        }
    }

    /// [`format()`](crate::format) on this formatter's arenas.
    ///
    /// # Errors
    ///
    /// The errors of [`format()`](crate::format).
    pub fn format(&mut self, source: &str) -> Result<String> {
        // The rewind is the contract, and it sits ahead of the work: a caller that caught
        // a panic out of the previous call left these arenas mid-document.
        self.ast.reset();
        self.doc.reset();
        let formatted = format_in(source, &self.ast, |_| &self.doc);
        // Nothing borrowed from the AST arena outlives `format_in`, so it can rewind now,
        // keeping one block rather than every block the document grew. The doc arena's
        // rewind frees nothing (its stores keep their capacity), so it waits for the next
        // call.
        self.ast.reset();
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
