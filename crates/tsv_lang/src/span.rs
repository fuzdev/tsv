// Span type for tracking source positions
// Using u32 for 50% memory savings (8 bytes vs 16 bytes on 64-bit)
// Maximum file size: 4GB (u32::MAX), which is more than sufficient for source code

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn combine(start: Span, end: Span) -> Self {
        Self {
            start: start.start,
            end: end.end,
        }
    }

    /// Extract the source text for this span
    #[inline]
    pub fn extract<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }

    /// Convert to std::ops::Range<usize> for indexing
    #[inline]
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }

    /// Get start position as usize (for indexing)
    #[inline]
    pub fn start_usize(&self) -> usize {
        self.start as usize
    }

    /// Get end position as usize (for indexing)
    #[inline]
    pub fn end_usize(&self) -> usize {
        self.end as usize
    }
}
