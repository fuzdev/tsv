// Shared print configuration across all formatters

/// Maximum line width used by the formatter (matches Prettier default).
///
/// Hardcoded — see [`crate::config`] module docs and the project README.
/// The renderer reads this constant directly; tests override widths via
/// the `*_with_widths` rendering helpers.
pub const PRINT_WIDTH: usize = 100;

/// Visual width of a single tab character, used for column calculations.
pub const TAB_WIDTH: usize = 2;

/// Indent string emitted at each indentation level.
///
/// `"\t"` matches the project's tabs-only indentation policy.
pub const INDENT: &str = "\t";

/// Workspace-wide doc-builder configuration.
///
/// Currently empty: the renderer reads compile-time globals ([`PRINT_WIDTH`] /
/// [`TAB_WIDTH`] / [`INDENT`]) directly, embedding state lives on
/// [`EmbedContext`], and language-specific knobs live on the language's own
/// config (e.g., `tsv_ts::TsConfig`). This type is the reserved slot for
/// cross-language doc-builder toggles that will appear when the hardcoded
/// pre-v0.1 posture relaxes — future Prettier-style options that apply to
/// every language go here.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrintConfig {}

/// How the renderer should treat the doc tree at its outer boundary.
///
/// Replaces the old `PrintConfig::is_embedded_expression: bool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// The doc tree is the entire document (e.g., a standalone TS/CSS file,
    /// or a Svelte `<script>` block).
    #[default]
    Standalone,
    /// The doc tree is a fragment embedded inside another language's output
    /// (e.g., a TS expression inside a Svelte `{...}` template tag). Binary
    /// expressions use ContinuationIndent style here, matching Prettier's
    /// `JsExpressionRoot` parent → `shouldNotIndent = true` semantics.
    Embedded,
}

/// Embedding state for a render. Threaded into the renderer alongside the
/// doc tree; replaces the `base_indent_offset` / `first_line_offset` /
/// `suffix_width` / `is_embedded_expression` fields previously on
/// [`PrintConfig`].
///
/// Constructed by the host language (e.g., tsv_svelte) when invoking an
/// embedded language's printer; defaults to a standalone, column-0,
/// no-suffix, no-base-offset layout.
#[derive(Debug, Clone, Copy)]
pub struct EmbedContext {
    /// Base indent offset for width calculations.
    /// Used when formatting nested content (e.g., CSS inside Svelte) where
    /// the output will be wrapped with additional indentation.
    pub base_indent_offset: usize,
    /// First line column offset for width calculations.
    /// Used when formatting expressions that start mid-line (e.g.,
    /// `{#each expr as item}`). The expression starts at column
    /// `first_line_offset`, not column 0.
    pub first_line_offset: usize,
    /// Expected suffix width after the expression (default: 0).
    /// Used when formatting expressions followed by known suffix text
    /// (e.g., ` as item}...`). Reduces the effective line width for wrapping
    /// decisions.
    // TODO: delete once the doc-tree embedding migration makes lookahead native.
    pub suffix_width: usize,
    /// How the renderer should treat the outer doc — see [`LayoutMode`].
    pub mode: LayoutMode,
}

impl Default for EmbedContext {
    fn default() -> Self {
        Self {
            base_indent_offset: 0,
            first_line_offset: 0,
            suffix_width: 0,
            mode: LayoutMode::Standalone,
        }
    }
}

impl EmbedContext {
    /// Convenience: is this an embedded fragment?
    #[inline]
    pub fn is_embedded(&self) -> bool {
        matches!(self.mode, LayoutMode::Embedded)
    }
}
