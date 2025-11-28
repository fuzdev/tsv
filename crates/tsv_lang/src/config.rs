// Shared print configuration across all formatters

/// Print configuration
#[derive(Debug, Clone, Copy)]
pub struct PrintConfig {
    /// Indent string (default: tabs)
    pub indent: &'static str,
    /// Maximum line width (default: 100)
    pub print_width: usize,
    /// Tab width for visual width calculations (default: 2)
    pub tab_width: usize,
    /// Base indent offset for width calculations (default: 0)
    /// Used when formatting nested content (e.g., CSS inside Svelte)
    /// where the output will be wrapped with additional indentation
    pub base_indent_offset: usize,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            base_indent_offset: 0,
        }
    }
}
