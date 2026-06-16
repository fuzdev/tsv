//! Hardcoded formatter settings shared across all language printers.
//!
//! tsv is **non-configurable**: formatting options are fixed at Prettier's
//! defaults and cannot be changed — there are no config files, CLI flags, or
//! runtime options (like `gofmt` or Black). The constants below are the single
//! source of truth for those fixed options; the renderer reads them directly,
//! so nothing is threaded through call signatures.
//!
//! Runtime *state* that genuinely varies per input is not configuration and
//! lives on dedicated context types: embedding offsets / layout mode on
//! [`EmbedContext`] here.

/// Maximum line width used by the formatter (matches Prettier default).
///
/// The renderer reads this constant directly; the doc-builder unit tests
/// exercise the layout at smaller widths via the internal `RenderConfig` seam
/// (`doc::render_config`), never at runtime.
pub const PRINT_WIDTH: usize = 100;

/// Visual width of a single tab character, used for column calculations.
pub const TAB_WIDTH: usize = 2;

/// Indent string emitted at each indentation level.
///
/// `"\t"` matches the project's tabs-only indentation policy.
pub const INDENT: &str = "\t";

/// How the renderer should treat the doc tree at its outer boundary.
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

/// Embedding state for a render, threaded into the renderer alongside the doc
/// tree. This is per-input *state*, not configuration: the host language (e.g.
/// tsv_svelte) constructs it when invoking an embedded language's printer.
/// Defaults to a standalone, column-0, no-suffix, no-base-offset layout.
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
