//! TypeScript formatting context.
//!
//! tsv is non-configurable — there are no user-facing TypeScript options. This
//! type is not configuration: it records whether the TypeScript being formatted
//! is standalone or embedded in a Svelte file, a distinction derived from the
//! file kind that selects context-dependent formatting required for Prettier
//! parity. It is the only such knob today (arrow-type-param disambiguation), so
//! it stays separate from the language-agnostic [`tsv_lang::EmbedContext`].

/// Whether TypeScript is being formatted on its own or embedded in a Svelte
/// file.
///
/// Not user configuration — the host picks the variant from the file kind. The
/// only behavior it currently gates is arrow-function type-param trailing-comma
/// disambiguation (`<T>` vs `<T,>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TsContext {
    /// A standalone `.ts` / `.svelte.ts` file. Single arrow type params stay
    /// bare (`<T>`), matching Prettier's pure-TypeScript output.
    #[default]
    Standalone,
    /// TypeScript embedded in a Svelte file (`<script>` blocks, template
    /// `{expr}` slots). Single arrow type params get a trailing comma (`<T,>`)
    /// so `<T>` isn't ambiguous with Svelte's template/element syntax. Used by
    /// `tsv_svelte` when formatting embedded TypeScript.
    Svelte,
}

impl TsContext {
    /// Whether a single arrow-function type param needs the `<T,>` trailing
    /// comma for Svelte disambiguation. True only for [`TsContext::Svelte`].
    #[inline]
    pub(crate) fn arrow_type_param_trailing_comma(self) -> bool {
        matches!(self, TsContext::Svelte)
    }
}
