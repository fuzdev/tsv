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
    /// First line column offset for width calculations (default: 0)
    /// Used when formatting expressions that start mid-line (e.g., `{#each expr as item}`)
    /// The expression starts at column first_line_offset, not column 0.
    pub first_line_offset: usize,
    /// Expected suffix width after the expression (default: 0)
    /// Used when formatting expressions followed by known suffix text (e.g., ` as item}...`)
    /// This reduces the effective line width for wrapping decisions.
    pub suffix_width: usize,
    /// Whether to add trailing comma for arrow type params disambiguation (default: true)
    /// When true, single type params in arrow functions get a trailing comma: `<T,>` instead of `<T>`
    /// This matches prettier's behavior in Svelte files where `<T>` could be confused with template syntax.
    /// Set to false for pure TypeScript (.ts) files.
    pub arrow_type_param_trailing_comma: bool,
    /// Whether to force binary expressions to allow line breaks even with 2 operands (default: false)
    /// When true, simple binary expressions like `a || b` can break when they exceed print width.
    /// Used in attribute string contexts where Prettier allows internal expression breaks.
    pub force_binary_breaks: bool,
    /// Whether this expression is embedded in a template language (default: false)
    /// When true, binary expressions use ContinuationIndent style (group([head, indent(rest)]))
    /// instead of Grouped style (group(parts)). This matches Prettier's behavior where
    /// JsExpressionRoot parent triggers `shouldNotIndent = true` (no indent) at the top level,
    /// but nested binaries within the expression still get normal indentation.
    pub is_embedded_expression: bool,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            // TODO: Replace with `use_tabs: bool` to match Prettier's config model.
            // Would require deriving indent string (Cow<str> or pre-computed) since
            // spaces need `" ".repeat(tab_width)`. See CLAUDE.md § Configuration.
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            base_indent_offset: 0,
            first_line_offset: 0,
            suffix_width: 0,
            arrow_type_param_trailing_comma: true, // Default to true for Svelte compatibility
            force_binary_breaks: false,
            is_embedded_expression: false,
        }
    }
}
