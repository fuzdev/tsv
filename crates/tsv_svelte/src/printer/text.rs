// Text analysis helpers for Svelte template whitespace
//
// Newline/blank-line predicates over `str`, used by the printer's blank-line decisions.
// (Whitespace *collapsing* itself lives in the doc-based content path —
// `build_text_fill_doc_trimmed` and friends.)

// Helper trait for text analysis
pub(crate) trait TextAnalysis {
    fn count_newlines(&self) -> usize;
    fn has_blank_line(&self) -> bool;
}

impl TextAnalysis for str {
    /// Count newlines in the string
    fn count_newlines(&self) -> usize {
        self.chars().filter(|&c| c == '\n').count()
    }

    /// Check if string contains a blank line (2+ newlines)
    fn has_blank_line(&self) -> bool {
        self.count_newlines() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::TextAnalysis;

    #[test]
    fn newline_counting_and_blank_line() {
        assert_eq!("a\nb".count_newlines(), 1);
        assert_eq!("a\n\nb".count_newlines(), 2);
        assert!("a\n\nb".has_blank_line());
        assert!(!"a\nb".has_blank_line());
    }
}
