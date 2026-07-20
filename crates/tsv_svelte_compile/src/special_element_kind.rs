//! The special-element handled-or-refused table: one macro-generated source for
//! the per-kind refusal labels, their list, and the enum→label mapping.
//!
//! A **shared classification table**, not an emitter — it is consumed by
//! [`crate::fragment`]'s dispatch (which refuses), by [`census`](mod@crate::census)
//! (which detects the same shapes as co-blockers), and by [`crate::refusal`] (whose
//! `is_deliberate_fence` reads [`SPECIAL_ELEMENT_FENCED_KINDS`]). It sits apart
//! from the fragment walk because none of those three consumers is walking: they
//! ask a question *about* a [`SpecialElementKind`], and keeping the table here
//! stops the walk's size from hiding what is really a lookup.
//!
//! **Single source of truth**, and the reason this is a macro rather than three
//! hand-written items: a label constant, the
//! [`SPECIAL_ELEMENT_REFUSAL_KINDS`] list, and
//! [`special_element_refusal_kind`] must agree, and only the mapping is checked
//! by exhaustiveness. A hand-written list beside the mapping keeps compiling when
//! a sixth kind appears, silently dropping that kind's key from the census's
//! declared buckets and quietly skewing its exposure accounting — so the list is
//! generated from the same table rows the mapping is.
//!
//! See [`crate::transform_server`] for the orchestration these gate.

use tsv_svelte::ast::internal::SpecialElementKind;

/// Declares the special-element handled-or-refused table ONCE, expanding it into
/// the three things that must agree: a label constant per refused kind, the
/// [`SPECIAL_ELEMENT_REFUSAL_KINDS`] list of every such label, and the
/// [`special_element_refusal_kind`] mapping itself.
///
/// One source, so completeness is STATIC rather than reviewed. A new
/// [`SpecialElementKind`] variant fails compilation in the generated match (which
/// is exhaustive), and the only way to satisfy it is a new table row — which then
/// mints the constant and extends the list on its own. A hand-written list beside
/// the mapping could not do this: a `[&str; 5]` keeps compiling when a sixth kind
/// appears, silently dropping that kind's key from the census's declared buckets
/// and quietly skewing its exposure accounting.
///
/// Each row pairs its pattern with its label directly, so there is no index into a
/// separate array that a reorder could silently re-point.
macro_rules! special_element_kind_table {
    (
        $( handled $handled:pat, )+
        $( $(#[$label_doc:meta])* refused $refused:pat => $label_const:ident = $label:literal, )+
    ) => {
        $(
            $(#[$label_doc])*
            pub(crate) const $label_const: &str = $label;
        )+

        /// Every label [`special_element_refusal_kind`] can return.
        ///
        /// The labels live here so the census's detected-bucket declaration
        /// ([`census_detected_buckets`](crate::census_detected_buckets)) can
        /// enumerate them without hand-copying a string — and, because both this
        /// list and the mapping expand from one table, without the two drifting.
        pub(crate) const SPECIAL_ELEMENT_REFUSAL_KINDS: &[&str] = &[$($label_const),+];

        /// The single enum→key mapping for special elements: `Some(kind)` is the
        /// [`Refusal::TemplateNode`](crate::Refusal::TemplateNode) label of a kind
        /// the SSR transform does not emit, `None` a kind it handles.
        ///
        /// Both consumers read it, so the refusal set and its detection can never
        /// drift: `clean_nodes` (which refuses) and the census's
        /// `collect_template_nodes` (which detects the same shape as a co-blocker)
        /// used to hand-mirror the allow-list. The key is per variant rather than a
        /// flat `"special element"`, so the corpus census can break the bucket down
        /// without a hand-run per-tag cross-reference.
        ///
        /// Exhaustive by design — a new [`SpecialElementKind`] variant fails
        /// compilation here, forcing a conscious emitted-or-refused choice in one
        /// place.
        pub(crate) fn special_element_refusal_kind(
            kind: &SpecialElementKind<'_>,
        ) -> Option<&'static str> {
            match kind {
                $( $handled => None, )+
                $( $refused => Some($label_const), )+
            }
        }
    };
}

special_element_kind_table! {
    // Handled: emitted, hoisted, or SSR-inert-but-validated.
    handled SpecialElementKind::SvelteHead,
    handled SpecialElementKind::TitleElement,
    handled SpecialElementKind::SvelteWindow,
    handled SpecialElementKind::SvelteBody,
    handled SpecialElementKind::SvelteDocument,
    handled SpecialElementKind::SvelteElement { .. },
    handled SpecialElementKind::SvelteBoundary,

    // Not emitted — one bucket each.
    /// `<svelte:component>` — a **fenced** legacy tag ([`SPECIAL_ELEMENT_FENCED_KINDS`]).
    refused SpecialElementKind::SvelteComponent { .. }
        => SPECIAL_ELEMENT_SVELTE_COMPONENT = "special element <svelte:component>",
    /// `<svelte:self>` — a **fenced** legacy tag ([`SPECIAL_ELEMENT_FENCED_KINDS`]).
    refused SpecialElementKind::SvelteSelf
        => SPECIAL_ELEMENT_SVELTE_SELF = "special element <svelte:self>",
    /// `<slot>` — a **fenced** legacy tag ([`SPECIAL_ELEMENT_FENCED_KINDS`]).
    refused SpecialElementKind::SlotElement
        => SPECIAL_ELEMENT_SLOT = "special element <slot>",
    /// `<svelte:fragment>` — a **fenced** legacy tag ([`SPECIAL_ELEMENT_FENCED_KINDS`]).
    refused SpecialElementKind::SvelteFragment
        => SPECIAL_ELEMENT_SVELTE_FRAGMENT = "special element <svelte:fragment>",
}

/// The subset of [`SPECIAL_ELEMENT_REFUSAL_KINDS`] that are **deliberate runes-only
/// fences** rather than unimplemented features — the set
/// [`Refusal::is_deliberate_fence`](crate::Refusal::is_deliberate_fence) reads.
///
/// Each is deprecation-warned or superseded by the oracle in Svelte 5: `<slot>` and
/// `<svelte:fragment>` by snippets (which this compiler already emits),
/// `<svelte:component>` by a plain dynamic component reference, `<svelte:self>` by
/// importing the module itself. A runes-only compiler will not implement them, so
/// booking the files that use them as future work books work that will never be done.
///
/// `<svelte:boundary>` is deliberately absent — a first-class Svelte 5 feature and a
/// real gap.
pub(crate) const SPECIAL_ELEMENT_FENCED_KINDS: [&str; 4] = [
    SPECIAL_ELEMENT_SVELTE_COMPONENT,
    SPECIAL_ELEMENT_SVELTE_SELF,
    SPECIAL_ELEMENT_SLOT,
    SPECIAL_ELEMENT_SVELTE_FRAGMENT,
];
