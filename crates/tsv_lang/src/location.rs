pub struct LocationTracker {
    line_starts: Vec<usize>,
}

impl LocationTracker {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    pub fn get_line_column(&self, offset: usize) -> (usize, usize) {
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx, // Exact match - this offset is at the start of a line
            Err(idx) => idx.saturating_sub(1),
        };

        let column = offset - self.line_starts[line_idx];
        (line_idx + 1, column) // Lines are 1-indexed
    }
}
