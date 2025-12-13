// Shared print configuration across all formatters

/// Print configuration
#[derive(Debug, Clone, Copy)]
pub struct PrintConfig {
    /// Indent string (default: tabs)
    pub indent: &'static str,
    /// Maximum line width (default: 100, matching prettier default)
    pub print_width: usize,
    /// Tab width for visual width calculations (default: 2)
    pub tab_width: usize,
    /// Base indent offset for width calculations (default: 0)
    /// Used when formatting nested content (e.g., CSS inside Svelte)
    /// where the output will be wrapped with additional indentation
    pub base_indent_offset: usize,
    /// Whether to add trailing comma for arrow type params disambiguation (default: true)
    /// When true, single type params in arrow functions get a trailing comma: `<T,>` instead of `<T>`
    /// This matches prettier's behavior in Svelte files where `<T>` could be confused with template syntax.
    /// Set to false for pure TypeScript (.ts) files.
    pub arrow_type_param_trailing_comma: bool,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            base_indent_offset: 0,
            arrow_type_param_trailing_comma: true, // Default to true for Svelte compatibility
        }
    }
}
