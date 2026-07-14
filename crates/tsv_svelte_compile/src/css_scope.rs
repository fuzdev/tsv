//! Minimal `<style>` scoping analysis and CSS splicing.
//!
//! Supports top-level rules whose selectors are single simple class selectors:
//! which class names are scoped, and the host-source positions where the
//! `.svelte-tsvhash` hash class splices into the style text — a source splice,
//! not a reprint, matching the oracle's output byte-for-byte.

use std::collections::BTreeSet;

use tsv_css::ast::internal::{CssBlockChild, CssNode, SimpleSelector};
use tsv_svelte::ast::internal::Style;

use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// The deterministic scoping class — the fixed `cssHash` the oracle sidecar
/// compiles with, so outputs are byte-comparable across runs.
pub(crate) const SCOPE_HASH_CLASS: &str = "svelte-tsvhash";

/// The scoping analysis product: which class names the style scopes, and the
/// host-source positions where the hash class splices into the style text.
pub(crate) struct ScopeInfo {
    pub(crate) class_names: BTreeSet<String>,
    /// Host-source byte offsets (each just past a `.class` selector token)
    /// where `.svelte-tsvhash` is inserted, ascending.
    insertions: Vec<u32>,
}

/// Analyze a `<style>` for the minimal supported shape: top-level rules whose
/// selectors are single simple class selectors. Anything else is refused — the
/// real matcher/pruner machinery is a later milestone.
pub(crate) fn analyze_style(style: &Style<'_>, source: &str) -> Result<ScopeInfo, CompileError> {
    let mut info = ScopeInfo {
        class_names: BTreeSet::new(),
        insertions: Vec::new(),
    };
    for node in style.css_stylesheet.nodes {
        let CssNode::Rule(rule) = node else {
            return Err(unsupported(Refusal::CssAtRule));
        };
        for child in rule.declarations {
            if matches!(child, CssBlockChild::Rule(_) | CssBlockChild::Atrule(_)) {
                return Err(unsupported(Refusal::CssNestedRule));
            }
        }
        for complex in rule.selector.selectors {
            let [relative] = complex.children else {
                return Err(unsupported(Refusal::CssCombinatorSelector));
            };
            let [SimpleSelector::Class { span }] = relative.selectors else {
                return Err(unsupported(Refusal::CssNonClassSelector));
            };
            // Span text includes the leading `.`.
            let name = &span.extract(source)[1..];
            info.class_names.insert(name.to_string());
            info.insertions.push(span.end);
        }
    }
    info.insertions.sort_unstable();
    Ok(info)
}

/// The scoped CSS: the author's style text verbatim (whitespace preserved) with
/// `.svelte-tsvhash` spliced in after each scoped selector — a source splice,
/// not a reprint, matching the oracle's output byte-for-byte.
pub(crate) fn splice_scoped_css(style: &Style<'_>, source: &str, scope: &ScopeInfo) -> String {
    let content_start = style.content_span.start;
    let content = style.content_span.extract(source);
    let mut out = String::with_capacity(content.len() + 16 * scope.insertions.len());
    let mut prev = 0usize;
    for &pos in &scope.insertions {
        let rel = (pos - content_start) as usize;
        out.push_str(&content[prev..rel]);
        out.push('.');
        out.push_str(SCOPE_HASH_CLASS);
        prev = rel;
    }
    out.push_str(&content[prev..]);
    out
}
