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
    /// (e.g., a TS expression inside a Svelte `{...}` template tag). A binary
    /// at the expression ROOT uses ContinuationIndent style here (the `{…}`
    /// value indents its continuation lines) — root only, decided at the
    /// embedding entry (`build_root_expression_doc`); every nested binary
    /// formats identically to a `<script>`, keyed by its parent position the
    /// way Prettier's `shouldNotIndent` parent chain does.
    Embedded,
}

/// Embedding state for a render, threaded into the renderer alongside the doc
/// tree. This is per-input *state*, not configuration: the host language (e.g.
/// tsv_svelte) constructs it when invoking an embedded language's printer.
/// Defaults to a standalone, column-0, no-suffix, no-base-offset layout.
///
/// ⚠️ **The width fields are read at render, not at doc-build.** The three width fields are
/// consumed by the renderer, so they act only on the context handed to `arena_print_doc_*`.
/// The context handed to a `build_*_doc` call reaches nothing but [`LayoutMode`] and
/// [`Self::jsdoc_cast_cannot_hang`] (the two build-time fields): that doc is rendered
/// later, under whatever context its host passes — so a width set there is inert, and a
/// layout choice that looks keyed to one is really keyed to `mode`. (Same reason a
/// build-time `current_column()` is always 0: build-time width state is disconnected from
/// the render column, and only `fits()` knows it.)
///
/// Deliberately lifetime-free and `Copy`: borrowed per-document inputs
/// (source, comments, the `line_breaks` table) travel as separate parameters
/// on the embedding entry points — folding one in here would force a lifetime
/// onto every `EmbedContext` holder.
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
    ///
    /// Its live effect is as a **gate**, not an offset: it is the column past which
    /// `suffix_width` is reserved, and (in `write_indentation`) the flag that a
    /// `base_indent_offset` applies at all. So with `suffix_width: 0` it does nothing on its
    /// own, and the two retire together.
    pub first_line_offset: usize,
    /// Expected suffix width after the expression (default: 0).
    /// Used when formatting expressions followed by known suffix text
    /// (e.g., ` as item}...`). Reduces the effective line width for wrapping
    /// decisions.
    // TODO: delete once the doc-tree embedding migration makes lookahead native. Nonzero at
    // four producers, all handing a local context straight to a render call: tsv_svelte's
    // empty-`<script></script>` closing tag, and tsv_css's selector (x2) + at-rule-prelude
    // writers (which also carry it as a fill's `trailing_reserve`, since fills don't read it).
    pub suffix_width: usize,
    /// How the renderer should treat the outer doc — see [`LayoutMode`].
    pub mode: LayoutMode,
    /// Whether a JSDoc cast **leading** this expression must REFLOW its comment→`(` break —
    /// the value-gap category "answers the break by rule and CANNOT hang". The second
    /// build-time field (with `mode`), read once at the TS expression entry
    /// (`build_expression_doc_with_comments`), which resolves the expression's left-spine
    /// cast and marks it; casts nested deeper keep their width-decided layout.
    ///
    /// Set by `tsv_svelte` for every braced head whose value **hugs** its delimiters (a
    /// block head, a prefixed tag, a braced attribute head, the `{expr}` tag): such a
    /// value starts right after the head's prefix, so there is no operator line to end and
    /// the cast's own-line hardline has no matching hang — the stranded `(` lands at the
    /// head's own column, an authoring with no fixed point. The heads that CAN place the
    /// comment on a real line of its own — `{@const}`'s `=` hang, a directive value's
    /// block form — leave this `false` and keep the authoring. See
    /// `docs/conformance_prettier_svelte.md` §Svelte: Own-line JSDoc cast at a braced head.
    pub jsdoc_cast_cannot_hang: bool,
}

impl Default for EmbedContext {
    fn default() -> Self {
        Self {
            base_indent_offset: 0,
            first_line_offset: 0,
            suffix_width: 0,
            mode: LayoutMode::Standalone,
            jsdoc_cast_cannot_hang: false,
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
