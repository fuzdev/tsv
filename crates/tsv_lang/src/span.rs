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

    /// Whether `inner` lies wholly within this span (inclusive on both ends, so a
    /// span contains itself).
    ///
    /// The question every "does this comment belong to that node rather than to the
    /// gap around it?" test asks — the list-expansion gates' inside-an-item filters,
    /// the compiler's dropped-region and module-script checks, and the ignore-range
    /// hoist cut. Spelled once here because the inclusive/exclusive choice at each end
    /// is the whole content of the predicate, and five hand-written copies is five
    /// chances to pick differently.
    #[inline]
    pub fn contains(&self, inner: Span) -> bool {
        inner.start >= self.start && inner.end <= self.end
    }

    /// Convert to `std::ops::Range<usize>` for indexing
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
