//! TypeScript-specific printer configuration.
//!
//! Concerns that only apply when formatting TypeScript live here, separate
//! from the language-agnostic [`tsv_lang::PrintConfig`].

/// TypeScript-only formatter configuration.
///
/// Holds knobs whose only meaning is in a TS context, kept out of the
/// foundation crate so it doesn't carry language-specific concerns.
#[derive(Debug, Clone, Copy, Default)]
pub struct TsConfig {
    /// Whether to add a trailing comma for arrow function type params for
    /// disambiguation. Default: false (pure-TS behavior).
    ///
    /// When true, single type params in arrow functions get a trailing comma:
    /// `<T,>` instead of `<T>`. Set this when formatting TS embedded in a
    /// Svelte template, where `<T>` could be parsed as an HTML-style tag.
    /// Pure `.ts` and `.svelte.ts` files use the default `false`.
    pub arrow_type_param_trailing_comma: bool,
}

impl TsConfig {
    /// Config for TypeScript embedded in a Svelte file. Enables
    /// arrow-type-param trailing commas so `<T>` isn't ambiguous with template
    /// syntax. Used by `tsv_svelte` when formatting `<script>` blocks and
    /// template expressions.
    pub fn svelte() -> Self {
        Self {
            arrow_type_param_trailing_comma: true,
        }
    }
}
