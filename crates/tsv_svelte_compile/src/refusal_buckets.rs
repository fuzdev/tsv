//! The refusal catalog's **accounting projections**: the stable corpus bucket
//! key, the enumerable variant catalog, and the deliberate-fence classifier.
//!
//! Split from [`crate::refusal`] because these answer a different question than
//! the catalog does. The enum and its `thiserror` messages say *what shape was
//! declined and how that reads to a human* — the surface
//! `docs/checklist_svelte_compiler.md` quotes. Everything here says *how refusals
//! are COUNTED*: which ones share a bucket ([`Refusal::bucket_key`]), what the
//! full bucket universe is ([`Refusal::every_variant`] /
//! [`Refusal::all_bucket_keys`]), and which buckets sit outside the
//! achievable-parity denominator ([`Refusal::is_deliberate_fence`]). The audiences
//! are the corpus runner and [`refusal_census`](mod@crate::refusal_census), not a person reading an
//! error.
//!
//! **The decoupling is deliberate and load-bearing**, which is the whole reason
//! these are worth separating rather than merely moving: a bucket key is
//! intentionally independent of [`Display`](std::fmt::Display) *so a message can be
//! reworded without shifting corpus buckets*. Keeping the two in one file invites
//! the shortcut of deriving one from the other, which would silently re-partition
//! every historical corpus comparison the next time a message is reworded.
//!
//! Inherent impls may live in any module of the defining crate, so these stay
//! `impl Refusal` — the split is physical, not a change to the API surface.

use std::borrow::Cow;
use std::collections::BTreeSet;

use crate::refusal::{
    INVALID_ASSIGNMENT_CONSTANT, INVALID_ASSIGNMENT_EACH_ITEM,
    INVALID_ASSIGNMENT_SNIPPET_PARAMETER, Refusal,
};
use crate::special_element_kind::SPECIAL_ELEMENT_FENCED_KINDS;

impl Refusal {
    /// The stable corpus bucket key for this refusal.
    ///
    /// User-chosen identifiers collapse to a `{placeholder}` so many concrete
    /// refusals share one bucket; closed-set feature discriminants
    /// ([`TemplateNode`](Refusal::TemplateNode),
    /// [`BindingPatternShape`](Refusal::BindingPatternShape),
    /// [`RunesOnlyFence`](Refusal::RunesOnlyFence)) keep their full
    /// message. The key is intentionally decoupled from
    /// [`Display`](std::fmt::Display) so a message can be reworded without
    /// shifting corpus buckets.
    ///
    /// Exhaustive by design: a new variant must make a conscious bucket-key
    /// choice here rather than silently splitting a bucket per parameter value.
    #[must_use]
    pub fn bucket_key(&self) -> Cow<'static, str> {
        match self {
            // Closed-set feature discriminants — the key is the full message.
            Self::TemplateNode { .. }
            | Self::BindingPatternShape { .. }
            | Self::RunesOnlyFence { .. }
            | Self::InvalidAssignmentTarget { .. } => Cow::Owned(self.to_string()),
            // Parameterized reasons — the user-chosen value collapses away.
            Self::LangInstanceScript { .. } => Cow::Borrowed("lang=\"{lang}\" script"),
            Self::GeneratedNameCollision { .. } => {
                Cow::Borrowed("generated name {name} collides with a user binding")
            }
            Self::Rune { .. } => Cow::Borrowed("rune {name}"),
            Self::DollarPrefixedIdentifier { .. } => Cow::Borrowed("$-prefixed identifier {name}"),
            Self::DollarPrefixedBinding { .. } => Cow::Borrowed("$-prefixed binding {name}"),
            Self::DerivedBindingRead { .. } => Cow::Borrowed("read of derived binding {name}"),
            Self::DerivedReadShadowed { .. } => {
                Cow::Borrowed("read of derived binding {name} shadowed in a nested scope")
            }
            Self::MemberCallAmbiguousRoot { .. } => Cow::Borrowed(
                "member/call rooted at prop/import {name} also bound in a nested scope",
            ),
            Self::BlockScopeShadowsDerived { .. } => {
                Cow::Borrowed("block-scope binding {name} shadows a $derived binding")
            }
            Self::StaticEvalNotPortable(_) => Cow::Borrowed("static evaluation not portable"),
            Self::StaticFoldNotPortable(_) => Cow::Borrowed("static fold not portable"),
            Self::EventAttribute { .. } => Cow::Borrowed("event attribute {name}"),
            Self::EventCaptureAttribute { .. } => {
                Cow::Borrowed("event capture attribute on a load-error element")
            }
            Self::InterpolatedAttrOnStyled { .. } => {
                Cow::Borrowed("interpolated {name} attribute on a styled component")
            }
            Self::ValueAttribute { .. } => Cow::Borrowed("value attribute on <{name}>"),
            Self::DynamicComponent { .. } => {
                Cow::Borrowed("dynamic <{name}> component (member or reactive binding)")
            }
            Self::ComponentNamedSlot { .. } => Cow::Borrowed("named slot on <{name}> component"),
            Self::ComponentChildrenPropConflict { .. } => {
                Cow::Borrowed("<{name}> component with both a children prop and default children")
            }
            Self::ComponentCustomProperty { .. } => {
                Cow::Borrowed("--custom-property attribute on <{name}> component")
            }
            Self::ComponentBindDirective { .. } => {
                Cow::Borrowed("bind: directive on <{name}> component")
            }
            Self::ComponentDirective { .. } => Cow::Borrowed("directive on <{name}> component"),
            Self::ElementWithChildren { .. } => Cow::Borrowed("<{name}> with children"),
            Self::TemplateLevelElement { .. } => Cow::Borrowed("template-level <{name}>"),
            Self::VoidElementChildren { .. } => Cow::Borrowed("children on void element <{name}>"),
            Self::CssSelectorNoMatch { .. } => {
                Cow::Borrowed("css selector {selector} matches no element")
            }
            // Static reasons — the message is already the bucket.
            Self::ClientGeneration => Cow::Borrowed("client generation"),
            Self::DevMode => Cow::Borrowed("dev mode output"),
            Self::ModuleDefaultExport => {
                Cow::Borrowed("default export in <script module> (the oracle rejects it)")
            }
            Self::ModuleInstanceNameCollision { .. } => {
                Cow::Borrowed("binding {name} declared in both the module and instance scripts")
            }
            Self::SvelteOptions => Cow::Borrowed("<svelte:options>"),
            Self::InstanceScriptExport => Cow::Borrowed(
                "instance-script export (component exports / $.bind_props not implemented)",
            ),
            Self::LegacyReactiveStatement => {
                Cow::Borrowed("legacy reactive statement `$:` (invalid in runes mode)")
            }
            Self::SvelteInternalImport => Cow::Borrowed("import from svelte/internal (forbidden)"),
            Self::RunesInvalidImport { .. } => {
                Cow::Borrowed("runes-invalid import of {name} from svelte")
            }
            Self::GenericsAttribute => {
                Cow::Borrowed("generics attribute on <script> (implies TypeScript)")
            }
            // TypeScript — closed-set discriminants, the message is the bucket.
            Self::TypeScriptWithoutLangTs
            | Self::CommentInErasedTypeRegion
            | Self::TsEnum
            | Self::TsNamespaceWithValue
            | Self::TsDottedNamespace
            | Self::TsParameterProperty
            | Self::Decorator
            | Self::TsAccessorField
            | Self::TsAbstractProperty
            | Self::TsOverloadSignature
            | Self::TsIndexSignature
            | Self::TsImportEquals
            | Self::TsExportAssignment
            | Self::TsNamespaceExport
            // Closed sets: no attribute name at all, and `failed`/`pending`.
            | Self::BoundaryInvalidAttribute
            | Self::BoundaryAttributeSnippet { .. } => Cow::Owned(self.to_string()),
            Self::PropsBindingPattern => Cow::Borrowed(
                "$props() binding pattern (not an identifier or object pattern — the oracle rejects it)",
            ),
            Self::DestructuringState => Cow::Borrowed("destructuring a $state declarator"),
            Self::DestructuringStateSnapshot => {
                Cow::Borrowed("destructuring a $state.snapshot declarator")
            }
            Self::DestructuringDerived => Cow::Borrowed("destructuring a $derived declarator"),
            Self::DestructuringDerivedBy => Cow::Borrowed("destructuring a $derived.by declarator"),
            Self::PropsIdBindingPattern => {
                Cow::Borrowed("$props.id() outside a plain top-level variable declaration")
            }
            Self::DuplicatePropsId => Cow::Borrowed("$props.id() used more than once"),
            Self::DuplicateProps => Cow::Borrowed("$props() used more than once"),
            Self::ClassFieldStateReactiveArg => Cow::Borrowed(
                "class-field $state with a lone store/$derived argument (the oracle keeps it bare)",
            ),
            Self::RuneNameBoundAsStore { .. } => {
                Cow::Borrowed("rune {name} whose base is also an instance binding")
            }
            Self::TopLevelAwait => {
                Cow::Borrowed("top-level await (async component output not implemented)")
            }
            Self::StoreScopedSubscription => {
                Cow::Borrowed("store subscription whose base is not a top-level component binding")
            }
            Self::StoreMemberWrite => Cow::Borrowed("store member write ($.store_mutate)"),
            Self::StoreDestructuringWrite => Cow::Borrowed("store destructuring write"),
            Self::MemberCallEscapedRoot => Cow::Borrowed(
                "member/call rooted at an escaped identifier (classification not ported)",
            ),
            Self::CommentsAlongsideMultiDeclarator => Cow::Borrowed(
                "comments in a script alongside a multi-declarator declaration (the oracle re-anchors comments inside the split)",
            ),
            Self::CommentsWithStore => Cow::Borrowed(
                "comments in a script that references a store ($$store_subs injection)",
            ),
            Self::CommentInRewrittenRuneRegion => {
                Cow::Borrowed("comment inside a rewritten rune region (dropped by the transform)")
            }
            Self::CommentAfterLastStatementWithBlock => Cow::Borrowed(
                "comment after the last script statement in a template that emits a nested block (the oracle drops it)",
            ),
            Self::ModuleCommentAfterInstanceScript => Cow::Borrowed(
                "comment in a module script placed after the instance script (the oracle re-attaches it into the template)",
            ),
            Self::LeadingCommentGluedToScript => {
                Cow::Borrowed("leading comment glued to the <script> line (no newline before it)")
            }
            Self::CommentsWithTemplateBeforeScript => {
                Cow::Borrowed("comments with template markup before the script (window ordering)")
            }
            Self::CommentsWithArglessState => {
                Cow::Borrowed("comments in a script with an argument-less $state()")
            }
            Self::CommentsWithRestProps => Cow::Borrowed(
                "comments in a script with a rest-element $props() (injected $$slots/$$events)",
            ),
            Self::CommentsWithNonDestructuredProps => Cow::Borrowed(
                "comments in a script with a non-destructured $props() (injected $$slots/$$events)",
            ),
            Self::CommentsWithPropsId => {
                Cow::Borrowed("comments in a script with a $props.id() declarator")
            }
            Self::CommentsWithBindable => {
                Cow::Borrowed("comments in a script with a $bindable() prop default")
            }
            Self::CommentsWithSlots => Cow::Borrowed(
                "comments in a script with a $$slots reference (injected sanitize_slots)",
            ),
            Self::MultilineBlockComment => Cow::Borrowed(
                "multi-line block comment in script (interior-line re-indentation not carried through)",
            ),
            Self::FormatIgnoreComment => Cow::Borrowed("format-ignore directive comment in script"),
            Self::TemplateComments => Cow::Borrowed(
                "template comments (only instance-script comments are carried through)",
            ),
            Self::ConstTagAtRoot => {
                Cow::Borrowed("{@const} at the component root (only valid inside a block)")
            }
            Self::DestructuredConstTag => {
                Cow::Borrowed("destructured {@const} (only `{@const name = …}`)")
            }
            Self::ConstTagNonPlainName => Cow::Borrowed("{@const} with a non-plain binding name"),
            Self::ConstTagOutsideBlock => Cow::Borrowed("{@const} outside a block scope"),
            Self::NestedEach => {
                Cow::Borrowed("nested {#each} (the nested emission path is not yet validated)")
            }
            Self::SnippetSignatureUnparsed => {
                Cow::Borrowed("{#snippet} signature the parser fell back to raw text for")
            }
            Self::SnippetEscapedName => Cow::Borrowed("{#snippet} with an escaped name"),
            Self::SnippetRestParameter => {
                Cow::Borrowed("{#snippet} rest parameter (the oracle rejects it)")
            }
            Self::SnippetHoistAmbiguous { .. } => {
                Cow::Borrowed("{#snippet} {name} hoist classification ambiguous")
            }
            Self::SnippetHoistOrder => Cow::Borrowed(
                "{#snippet} alongside a {@const}/<svelte:head> in the same fragment (hoist order)",
            ),
            Self::DuplicateSnippetName { .. } => {
                Cow::Borrowed("duplicate {#snippet} {name} (the oracle rejects it)")
            }
            Self::SnippetDeclarationDuplicate { .. } => Cow::Borrowed(
                "{#snippet} {name} is already declared by the instance script (the oracle rejects it)",
            ),
            Self::SnippetShadowingProp { .. } => Cow::Borrowed(
                "{#snippet} {name} shadows the component prop of the same name (the oracle rejects it)",
            ),
            Self::SnippetChildrenConflict => Cow::Borrowed(
                "{#snippet children()} alongside other default content (the oracle rejects it)",
            ),
            Self::SnippetInvalidExport { .. } => Cow::Borrowed(
                "exported {#snippet} {name} is not module-hoistable (the oracle rejects it)",
            ),
            Self::ExportUndefined { .. } => Cow::Borrowed(
                "module script exports {name}, which it does not declare (the oracle rejects it)",
            ),
            Self::RenderTagUnsupportedCallee => {
                Cow::Borrowed("{@render} callee is not a resolvable local snippet or snippet prop")
            }
            Self::HtmlTagStaticValue => Cow::Borrowed("{@html} with a statically-known value"),
            Self::MutationInTemplateExpr => Cow::Borrowed("mutation inside a template expression"),
            Self::UseDirectiveOnLoadErrorElement => Cow::Borrowed(
                "use: directive on a load-error element (event-capture markup not implemented)",
            ),
            Self::TransitionDirectiveConflict => Cow::Borrowed(
                "conflicting transition directives (an element may have at most one intro and one outro — the oracle rejects it)",
            ),
            Self::AnimateDirectiveInvalid => Cow::Borrowed(
                "invalid animate: directive (one per element, only on the sole child of a keyed {#each} — the oracle rejects it)",
            ),
            Self::SpreadOnSelect => {
                Cow::Borrowed("{...spread} on <select> (the oracle routes to $$renderer.select)")
            }
            Self::SpreadOnLoadErrorElement => Cow::Borrowed(
                "{...spread} on a load-error element (event-capture markup not implemented)",
            ),
            Self::BindDirective { .. } => Cow::Borrowed("bind: directive {name}"),
            Self::ClassDirectiveWithMixedClass => {
                Cow::Borrowed("class: directive alongside a mixed-value class attribute")
            }
            Self::StyleDirectiveWithMixedStyle => {
                Cow::Borrowed("style: directive alongside a mixed-value style attribute")
            }
            Self::StyleDirectiveWithMixedValue => {
                Cow::Borrowed("style: directive with a mixed-value (text + expression) value")
            }
            Self::StyleDirectiveInvalidModifier => Cow::Borrowed(
                "style: directive with an invalid modifier (only |important, once, is allowed)",
            ),
            Self::StringLiteralExprAttribute => {
                Cow::Borrowed("string-literal expression attribute value (inline-literal path)")
            }
            Self::DynamicClassOnStyled => {
                Cow::Borrowed("dynamic class attribute on a styled component")
            }
            Self::DynamicStyleOnStyled => {
                Cow::Borrowed("dynamic style attribute on a styled component")
            }
            Self::OptionElement => {
                Cow::Borrowed("<option> (oracle emits $$renderer.option closures)")
            }
            Self::SvelteHeadAttributes => Cow::Borrowed("attributes on <svelte:head>"),
            Self::BoundaryInvalidAttributeValue { .. } => Cow::Borrowed(
                "non-expression value for <svelte:boundary> attribute {name} (the oracle rejects it)",
            ),
            Self::TitleAttributes => Cow::Borrowed("attribute on <title> (the oracle rejects it)"),
            Self::TitleInvalidContent => Cow::Borrowed(
                "invalid <title> content (only text and {expression} — the oracle rejects it)",
            ),
            Self::SvelteHeadWithConstTag => Cow::Borrowed(
                "<svelte:head> alongside a {@const} in the same fragment (hoist order)",
            ),
            Self::RuneInvalidSpread { .. } => {
                Cow::Borrowed("{rune} cannot be called with a spread argument (the oracle rejects it)")
            }
            Self::SvelteMetaInvalidTag { .. } => {
                Cow::Borrowed("<{name}> is not a valid <svelte:...> meta tag (the oracle rejects it)")
            }
            Self::SpecialElementInvalidPlacement { .. } => {
                Cow::Borrowed("<{name}> must be a top-level element (the oracle rejects it)")
            }
            Self::DuplicateSpecialElement { .. } => {
                Cow::Borrowed("duplicate <{name}> element (the oracle rejects it)")
            }
            Self::AttributeInvalidName { .. } => {
                Cow::Borrowed("invalid attribute name `{name}` (the oracle rejects it)")
            }
            Self::AttributeInvalidEventHandler { .. } => Cow::Borrowed(
                "`{name}` event handler needs an expression value (the oracle rejects it)",
            ),
            Self::AttributeUnquotedSequence { .. } => Cow::Borrowed(
                "`{name}` attribute value with multiple parts must be quoted (the oracle rejects it)",
            ),
            Self::AttributeInvalidSequenceExpression => Cow::Borrowed(
                "unparenthesized sequence expression in an attribute (the oracle rejects it)",
            ),
            Self::SlotAttributeInvalidPlacement => {
                Cow::Borrowed("misplaced slot=\"…\" attribute (the oracle rejects it)")
            }
            Self::DuplicateAttribute { .. } => Cow::Borrowed(
                "duplicate `{name}` attribute on one element (the oracle rejects it)",
            ),
            // The message names the offending tag pair, so it is collapsed away —
            // every HTML content-model violation shares one corpus bucket.
            Self::NodeInvalidPlacement { .. } => {
                Cow::Borrowed("invalid HTML node placement (the oracle rejects it)")
            }
            Self::SpecialElementChildren { .. } => {
                Cow::Borrowed("<{name}> cannot have children (the oracle rejects it)")
            }
            Self::SpecialElementIllegalAttribute { .. } => {
                Cow::Borrowed("invalid attribute on <{name}> (the oracle rejects it)")
            }
            Self::CssAtRule => Cow::Borrowed("css at-rule in <style>"),
            Self::CssNestedRule => Cow::Borrowed("nested css rule in <style>"),
            Self::CssEmptyRule => {
                Cow::Borrowed("empty css rule in <style> (the oracle comment-wraps it)")
            }
            Self::CssCombinatorSelector => Cow::Borrowed("css combinator selector in <style>"),
            Self::CssUnsupportedSelector => Cow::Borrowed(
                "unsupported css selector in <style> (:global/:is/:where/:has/:not/:root/nesting)",
            ),
            Self::CssDynamicAttributeMatch => Cow::Borrowed(
                "css attribute selector against a dynamic attribute value (static-eval not ported)",
            ),
            Self::CssCaseInsensitiveNonAscii => Cow::Borrowed(
                "css case-insensitive match with a non-ASCII operand (Unicode case-fold not ported)",
            ),
        }
    }

    /// One representative of every [`Refusal`] variant, for enumerating the
    /// bucket-key catalog.
    ///
    /// Parameter values are the field name in braces (`"{name}"`), so a
    /// parameterized variant whose key *is* its `Display` message renders in the
    /// same placeholder form `docs/checklist_svelte_compiler.md` quotes; for a
    /// variant whose key collapses its parameters the value is irrelevant.
    ///
    /// ⚠️ Hand-maintained, and **not** compiler-enforced — a new variant compiles
    /// fine while missing here (unlike [`bucket_key`](Refusal::bucket_key), whose
    /// match is exhaustive). `compile_conformance_audit`'s drift check reads this
    /// list, so an omission would silently narrow that audit's oracle. Add the
    /// variant here in the same change.
    ///
    /// The omission is caught at test time rather than compile time, from the
    /// enum's own source: `tests::refusal_buckets::every_variant_covers_the_enum`
    /// derives the variant list from `refusal.rs` and diffs it against this one.
    /// That guard exists because the downstream pin cannot see an omission —
    /// `compile_conformance_audit`'s `EXPECTED_BUCKET_KEYS` is a snapshot of what
    /// this list *produces*, so a variant absent from both changes no key.
    #[must_use]
    pub fn every_variant() -> Vec<Self> {
        vec![
            Self::ClientGeneration,
            Self::DevMode,
            Self::ModuleDefaultExport,
            Self::ModuleInstanceNameCollision {
                name: "{name}".to_string(),
            },
            Self::SvelteOptions,
            Self::InstanceScriptExport,
            Self::LegacyReactiveStatement,
            Self::SvelteInternalImport,
            Self::RunesInvalidImport {
                name: "{name}".to_string(),
            },
            Self::GenericsAttribute,
            Self::LangInstanceScript {
                lang: "{lang}".to_string(),
            },
            Self::TypeScriptWithoutLangTs,
            Self::CommentInErasedTypeRegion,
            Self::TsEnum,
            Self::TsNamespaceWithValue,
            Self::TsDottedNamespace,
            Self::TsParameterProperty,
            Self::Decorator,
            Self::TsAccessorField,
            Self::TsAbstractProperty,
            Self::TsOverloadSignature,
            Self::TsIndexSignature,
            Self::TsImportEquals,
            Self::TsExportAssignment,
            Self::TsNamespaceExport,
            Self::GeneratedNameCollision {
                name: "{name}".to_string(),
            },
            Self::PropsBindingPattern,
            Self::BindingPatternShape { kind: "{kind}" },
            Self::DestructuringState,
            Self::DestructuringStateSnapshot,
            Self::DestructuringDerived,
            Self::DestructuringDerivedBy,
            Self::PropsIdBindingPattern,
            Self::DuplicatePropsId,
            Self::DuplicateProps,
            Self::ClassFieldStateReactiveArg,
            Self::Rune {
                name: "{name}".to_string(),
            },
            Self::DollarPrefixedIdentifier {
                name: "{name}".to_string(),
            },
            Self::DollarPrefixedBinding {
                name: "{name}".to_string(),
            },
            Self::DerivedBindingRead {
                name: "{name}".to_string(),
            },
            Self::DerivedReadShadowed {
                name: "{name}".to_string(),
            },
            Self::RuneNameBoundAsStore {
                name: "{name}".to_string(),
            },
            Self::TopLevelAwait,
            Self::StoreScopedSubscription,
            Self::StoreMemberWrite,
            Self::StoreDestructuringWrite,
            Self::MemberCallAmbiguousRoot {
                name: "{name}".to_string(),
            },
            Self::MemberCallEscapedRoot,
            Self::CommentsAlongsideMultiDeclarator,
            Self::CommentsWithStore,
            Self::CommentInRewrittenRuneRegion,
            Self::CommentAfterLastStatementWithBlock,
            Self::ModuleCommentAfterInstanceScript,
            Self::LeadingCommentGluedToScript,
            Self::CommentsWithTemplateBeforeScript,
            Self::CommentsWithArglessState,
            Self::CommentsWithRestProps,
            Self::CommentsWithNonDestructuredProps,
            Self::CommentsWithPropsId,
            Self::CommentsWithBindable,
            Self::CommentsWithSlots,
            Self::MultilineBlockComment,
            Self::FormatIgnoreComment,
            Self::TemplateComments,
            Self::TemplateNode { kind: "{kind}" },
            Self::ConstTagAtRoot,
            Self::DestructuredConstTag,
            Self::ConstTagNonPlainName,
            Self::ConstTagOutsideBlock,
            Self::NestedEach,
            Self::SnippetSignatureUnparsed,
            Self::SnippetEscapedName,
            Self::SnippetRestParameter,
            Self::SnippetHoistAmbiguous {
                name: "{name}".to_string(),
            },
            Self::SnippetHoistOrder,
            Self::DuplicateSnippetName {
                name: "{name}".to_string(),
            },
            Self::SnippetDeclarationDuplicate {
                name: "{name}".to_string(),
            },
            Self::SnippetShadowingProp {
                name: "{name}".to_string(),
            },
            Self::SnippetChildrenConflict,
            Self::SnippetInvalidExport {
                name: "{name}".to_string(),
            },
            Self::ExportUndefined {
                name: "{name}".to_string(),
            },
            Self::RenderTagUnsupportedCallee,
            Self::BlockScopeShadowsDerived {
                name: "{name}".to_string(),
            },
            Self::HtmlTagStaticValue,
            Self::MutationInTemplateExpr,
            Self::StaticEvalNotPortable("{reason}".to_string()),
            Self::StaticFoldNotPortable("{reason}".to_string()),
            Self::EventAttribute {
                name: "{name}".to_string(),
            },
            Self::EventCaptureAttribute {
                name: "{name}".to_string(),
            },
            Self::UseDirectiveOnLoadErrorElement,
            Self::TransitionDirectiveConflict,
            Self::AnimateDirectiveInvalid,
            Self::RunesOnlyFence {
                directive: "{directive}",
            },
            Self::InvalidAssignmentTarget {
                target: INVALID_ASSIGNMENT_CONSTANT,
            },
            Self::InvalidAssignmentTarget {
                target: INVALID_ASSIGNMENT_EACH_ITEM,
            },
            Self::InvalidAssignmentTarget {
                target: INVALID_ASSIGNMENT_SNIPPET_PARAMETER,
            },
            Self::SpreadOnSelect,
            Self::SpreadOnLoadErrorElement,
            Self::BindDirective {
                name: "{name}".to_string(),
            },
            Self::ClassDirectiveWithMixedClass,
            Self::StyleDirectiveWithMixedStyle,
            Self::StyleDirectiveWithMixedValue,
            Self::StyleDirectiveInvalidModifier,
            Self::StringLiteralExprAttribute,
            Self::DynamicClassOnStyled,
            Self::DynamicStyleOnStyled,
            Self::InterpolatedAttrOnStyled {
                name: "{name}".to_string(),
            },
            Self::ValueAttribute {
                name: "{name}".to_string(),
            },
            Self::DynamicComponent {
                name: "{name}".to_string(),
            },
            Self::ComponentNamedSlot {
                name: "{name}".to_string(),
            },
            Self::ComponentChildrenPropConflict {
                name: "{name}".to_string(),
            },
            Self::ComponentCustomProperty {
                name: "{name}".to_string(),
            },
            Self::ComponentBindDirective {
                name: "{name}".to_string(),
            },
            Self::ComponentDirective {
                name: "{name}".to_string(),
            },
            Self::ElementWithChildren {
                name: "{name}".to_string(),
            },
            Self::TemplateLevelElement {
                name: "{name}".to_string(),
            },
            Self::VoidElementChildren {
                name: "{name}".to_string(),
            },
            Self::OptionElement,
            Self::SvelteHeadAttributes,
            Self::BoundaryInvalidAttribute,
            Self::BoundaryInvalidAttributeValue {
                name: "{name}".to_string(),
            },
            Self::BoundaryAttributeSnippet { name: "{name}" },
            Self::TitleAttributes,
            Self::TitleInvalidContent,
            Self::SvelteHeadWithConstTag,
            Self::RuneInvalidSpread {
                rune: "{rune}".to_string(),
            },
            Self::SvelteMetaInvalidTag {
                name: "{name}".to_string(),
            },
            Self::SpecialElementInvalidPlacement {
                name: "{name}".to_string(),
            },
            Self::DuplicateSpecialElement {
                name: "{name}".to_string(),
            },
            Self::AttributeInvalidName {
                name: "{name}".to_string(),
            },
            Self::AttributeInvalidEventHandler {
                name: "{name}".to_string(),
            },
            Self::AttributeUnquotedSequence {
                name: "{name}".to_string(),
            },
            Self::AttributeInvalidSequenceExpression,
            Self::SlotAttributeInvalidPlacement,
            Self::DuplicateAttribute {
                name: "{name}".to_string(),
            },
            // The key collapses this variant's parameter, so the value is
            // irrelevant — it is never rendered into the catalog.
            Self::NodeInvalidPlacement {
                message: "{message}".to_string(),
            },
            Self::SpecialElementChildren {
                name: "{name}".to_string(),
            },
            Self::SpecialElementIllegalAttribute {
                name: "{name}".to_string(),
            },
            Self::CssAtRule,
            Self::CssNestedRule,
            Self::CssEmptyRule,
            Self::CssCombinatorSelector,
            Self::CssUnsupportedSelector,
            Self::CssDynamicAttributeMatch,
            Self::CssCaseInsensitiveNonAscii,
            Self::CssSelectorNoMatch {
                selector: "{selector}".to_string(),
            },
        ]
    }

    /// Every bucket key the refusal catalog can produce, in the placeholder form
    /// the checklist document quotes. See [`every_variant`](Refusal::every_variant)
    /// for the caveat on completeness.
    #[must_use]
    pub fn all_bucket_keys() -> BTreeSet<String> {
        Self::every_variant()
            .iter()
            .map(|r| r.bucket_key().into_owned())
            .collect()
    }

    /// Whether this refusal is a **deliberate product fence** rather than an
    /// unimplemented feature.
    ///
    /// tsv's Svelte compiler is runes-only by choice, so the legacy authoring
    /// syntax it declines will never be implemented — it is not a gap, and a file
    /// containing one is not an achievable parity target. Measurement uses this to
    /// keep the fenced population out of the parity denominator; every other
    /// refusal is a "not yet" that counts against it.
    ///
    /// The fenced set is the legacy **slot system** and the legacy **directive
    /// syntax**, both superseded in Svelte 5:
    ///
    /// - [`RunesOnlyFence`](Refusal::RunesOnlyFence) — a legacy `on:` event
    ///   directive and `let:`, on a regular or special element;
    /// - the legacy special-element tags
    ///   (`special_element_kind::SPECIAL_ELEMENT_FENCED_KINDS`)
    ///   — `<slot>`, `<svelte:fragment>`, `<svelte:component>`, `<svelte:self>`; and
    /// - [`ComponentNamedSlot`](Refusal::ComponentNamedSlot) — a `slot="…"` on a
    ///   component's child, the *consumer* half of the same slot system whose
    ///   `<slot>` / `<svelte:fragment>` *declaration* half is fenced above.
    ///   Snippets supersede it, and this compiler already emits them.
    ///
    /// Each is deprecation-warned or superseded by the oracle in Svelte 5, so
    /// counting them as future work books work that will never be done.
    ///
    /// Deliberately **outside** the set: `<svelte:boundary>` (a first-class Svelte 5
    /// feature and a real gap), and
    /// [`ComponentDirective`](Refusal::ComponentDirective) — which a legacy `on:` /
    /// `let:` on a *component* raises instead of `RunesOnlyFence`, but whose bucket
    /// mixes those with unimplemented `class:` / `use:` / `transition:` directives,
    /// so it cannot be fenced wholesale.
    #[must_use]
    pub fn is_deliberate_fence(&self) -> bool {
        match self {
            Self::RunesOnlyFence { .. } | Self::ComponentNamedSlot { .. } => true,
            Self::TemplateNode { kind } => SPECIAL_ELEMENT_FENCED_KINDS.contains(kind),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Refusal;

    #[test]
    fn bucket_key_collapses_parameters() {
        // Distinct event handlers share one bucket.
        assert_eq!(
            Refusal::EventAttribute {
                name: "onclick".to_string()
            }
            .bucket_key(),
            "event attribute {name}"
        );
        assert_eq!(
            Refusal::EventAttribute {
                name: "onkeydown".to_string()
            }
            .bucket_key(),
            "event attribute {name}"
        );
        assert_eq!(
            Refusal::Rune {
                name: "$inspect".to_string()
            }
            .bucket_key(),
            "rune {name}"
        );
        assert_eq!(
            Refusal::StaticEvalNotPortable("string-to-number coercion".to_string()).bucket_key(),
            "static evaluation not portable"
        );
    }

    #[test]
    fn bucket_key_keeps_feature_discriminants() {
        // Closed-set discriminants stay distinct (the key is the full message).
        assert_eq!(
            Refusal::TemplateNode {
                kind: "special element"
            }
            .bucket_key(),
            "template node special element"
        );
        assert_eq!(
            Refusal::TemplateNode {
                kind: "{@render} tag"
            }
            .bucket_key(),
            "template node {@render} tag"
        );
        assert_eq!(
            Refusal::BindingPatternShape {
                kind: "member expression"
            }
            .bucket_key(),
            "binding pattern shape (member expression)"
        );
    }

    #[test]
    fn bucket_key_passes_static_reasons_through() {
        assert_eq!(
            Refusal::InstanceScriptExport.bucket_key(),
            "instance-script export (component exports / $.bind_props not implemented)"
        );
        assert_eq!(
            Refusal::ModuleDefaultExport.bucket_key(),
            "default export in <script module> (the oracle rejects it)"
        );
    }

    /// The fenced set is a product decision, so it is pinned by name rather than
    /// left to whatever the label table happens to contain.
    #[test]
    fn deliberate_fences_are_the_legacy_syntax_only() {
        use crate::special_element_kind::{
            SPECIAL_ELEMENT_SLOT, SPECIAL_ELEMENT_SVELTE_COMPONENT,
            SPECIAL_ELEMENT_SVELTE_FRAGMENT, SPECIAL_ELEMENT_SVELTE_SELF,
        };

        // Legacy directives.
        assert!(Refusal::RunesOnlyFence { directive: "on:" }.is_deliberate_fence());
        assert!(Refusal::RunesOnlyFence { directive: "let:" }.is_deliberate_fence());
        // The consumer half of the legacy slot system — `<div slot="header">` on a
        // component child, superseded by snippets like the `<slot>` half below.
        assert!(
            Refusal::ComponentNamedSlot {
                name: "Foo".to_string()
            }
            .is_deliberate_fence()
        );
        // Legacy special-element tags — superseded by snippets / plain references.
        for kind in [
            SPECIAL_ELEMENT_SLOT,
            SPECIAL_ELEMENT_SVELTE_FRAGMENT,
            SPECIAL_ELEMENT_SVELTE_COMPONENT,
            SPECIAL_ELEMENT_SVELTE_SELF,
        ] {
            assert!(
                Refusal::TemplateNode { kind }.is_deliberate_fence(),
                "{kind} is a runes-only fence"
            );
        }
        // `<svelte:boundary>` is a first-class Svelte 5 feature, so it never joined
        // the fence set — and it now COMPILES, so it has no `TemplateNode` label at
        // all. Its residual refusals are ordinary gaps, never fences.
        assert!(!Refusal::BoundaryInvalidAttribute.is_deliberate_fence());
        assert!(!Refusal::BoundaryAttributeSnippet { name: "failed" }.is_deliberate_fence());
        // Neither is any other template node, or an ordinary "not yet".
        assert!(
            !Refusal::TemplateNode {
                kind: "{@debug} tag"
            }
            .is_deliberate_fence()
        );
        assert!(
            !Refusal::ComponentDirective {
                name: "Foo".to_string()
            }
            .is_deliberate_fence()
        );
        assert!(!Refusal::CssAtRule.is_deliberate_fence());
    }
}
