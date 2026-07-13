//! The typed catalog of compiler refusal reasons.
//!
//! Every shape [`compile`](crate::compile) declines to emit surfaces as a
//! [`Refusal`], carried by
//! [`CompileError::Unsupported`](crate::CompileError::Unsupported). Each variant
//! owns its parameters and provides two projections:
//!
//! - its [`Display`](std::fmt::Display) message — the human-readable reason,
//!   derived via `thiserror`; and
//! - its [`bucket_key`](Refusal::bucket_key) — a stable identifier the corpus
//!   runner groups by. User-chosen identifiers (binding/tag/class names, `lang`
//!   values, runes) collapse to a `{placeholder}` in the key, so e.g. every
//!   `event attribute …` shares one bucket; closed-set feature discriminants
//!   keep their full message.
//!
//! This is the single source of truth for the refusal contract: the transform
//! constructs these variants, the corpus runner reads their bucket keys
//! directly (no string re-parsing), and `docs/checklist_svelte_compiler.md`
//! quotes their messages.

use std::borrow::Cow;

/// A component shape the Svelte-to-JS compiler declines to emit, with a stable
/// corpus bucket key.
///
/// See the module documentation for the two projections every variant carries.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    // ── Compile options ────────────────────────────────────────────────────
    /// Client-side generation (only server output is implemented).
    #[error("client generation")]
    ClientGeneration,
    /// Development-mode output (extra runtime checks / metadata).
    #[error("dev mode output")]
    DevMode,

    // ── Script shell / module scaffold ─────────────────────────────────────
    /// A `<script context="module">` block.
    #[error("module <script context=\"module\">")]
    ModuleScript,
    /// A `<svelte:options>` element.
    #[error("<svelte:options>")]
    SvelteOptions,
    /// Any instance-script `export` form (the oracle uses `$.bind_props`).
    #[error("instance-script export (component exports / $.bind_props not implemented)")]
    InstanceScriptExport,
    /// A `generics` attribute on the instance script (implies TypeScript).
    #[error("generics attribute on instance script (implies TypeScript)")]
    GenericsAttribute,
    /// An instance script with a `lang` other than `js`/empty.
    #[error("lang=\"{lang}\" instance script (type stripping not implemented)")]
    LangInstanceScript {
        /// The declared `lang` attribute value.
        lang: String,
    },
    /// A TypeScript `enum`/`module` declaration in the instance script.
    #[error("TS enum/module declaration in instance script")]
    TsEnumOrModule,
    /// A generated block name (`each_array`/`$$index`/…) collides with a user
    /// binding, so the oracle would pick a different suffix.
    #[error("generated name {name} collides with a user binding")]
    GeneratedNameCollision {
        /// The colliding generated name.
        name: String,
    },

    // ── `$props()` / declarator patterns ───────────────────────────────────
    /// A `$props()` binding pattern that is neither an identifier nor an object
    /// pattern (the oracle rejects it).
    #[error(
        "$props() binding pattern (not an identifier or object pattern — the oracle rejects it)"
    )]
    PropsBindingPattern,
    /// A binding pattern shape the analyzer does not classify.
    #[error("binding pattern shape ({kind})")]
    BindingPatternShape {
        /// A short description of the unrecognized pattern node.
        kind: &'static str,
    },
    /// Destructuring a `$state(…)` declarator.
    #[error("destructuring a $state declarator")]
    DestructuringState,
    /// Destructuring a `$derived(…)` declarator.
    #[error("destructuring a $derived declarator")]
    DestructuringDerived,
    /// Destructuring a `$derived.by(…)` declarator.
    #[error("destructuring a $derived.by declarator")]
    DestructuringDerivedBy,

    // ── Runes ──────────────────────────────────────────────────────────────
    /// A non-sanctioned rune call (or a rune in a non-sanctioned position).
    #[error("rune {name}")]
    Rune {
        /// The rune identifier as written.
        name: String,
    },
    /// A bare rune reference or any other `$`-prefixed identifier read.
    #[error("$-prefixed identifier {name}")]
    DollarPrefixedIdentifier {
        /// The `$`-prefixed identifier.
        name: String,
    },
    /// A `$derived` binding read outside a bare template-expression position.
    #[error("read of derived binding {name} (supported only as a bare template expression)")]
    DerivedBindingRead {
        /// The derived binding name.
        name: String,
    },
    /// A top-level `await` (async component output is not implemented).
    #[error("top-level await (async component output not implemented)")]
    TopLevelAwait,

    // ── `needs_context` classification ─────────────────────────────────────
    /// A member/call rooted at a prop/import that a nested scope also binds —
    /// ambiguous for the name-based `needs_context` port.
    #[error(
        "member/call rooted at prop/import `{name}` that is also bound in a nested scope \
         (needs_context classification ambiguous)"
    )]
    MemberCallAmbiguousRoot {
        /// The ambiguous root name.
        name: String,
    },
    /// A member/call rooted at a unicode-escaped identifier (not ported).
    #[error("member/call rooted at an escaped identifier (classification not ported)")]
    MemberCallEscapedRoot,

    // ── Instance-script comment placement classes ──────────────────────────
    /// Comments alongside a multi-declarator declaration (the oracle re-anchors
    /// them inside the split).
    #[error(
        "comments in a script alongside a multi-declarator declaration \
         (the oracle re-anchors comments inside the split)"
    )]
    CommentsAlongsideMultiDeclarator,
    /// Comments alongside hoisted imports.
    #[error(
        "comments in a script alongside imports \
         (placement around hoisted imports not carried through yet)"
    )]
    CommentsAlongsideImports,
    /// Comments alongside template blocks.
    #[error("comments in a script alongside template blocks (placement not carried through yet)")]
    CommentsAlongsideTemplateBlocks,
    /// Comments in a script that uses `$derived`.
    #[error("comments in a script that uses $derived (not carried through yet)")]
    CommentsWithDerived,
    /// A comment inside a rewritten (dropped) rune region.
    #[error("comment inside a rewritten rune region (dropped by the transform)")]
    CommentInRewrittenRuneRegion,
    /// A comment after the last script statement (the oracle re-attaches it into
    /// the template).
    #[error(
        "comment after the last script statement (the oracle re-attaches it into the template)"
    )]
    CommentAfterLastStatement,
    /// A leading comment glued to the `<script>` line.
    #[error("leading comment glued to the <script> line (no newline before it)")]
    LeadingCommentGluedToScript,
    /// Comments with template markup preceding the script (window ordering).
    #[error("comments with template markup before the script (window ordering)")]
    CommentsWithTemplateBeforeScript,
    /// Comments in a script with an argument-less `$state()`.
    #[error("comments in a script with an argument-less $state()")]
    CommentsWithArglessState,
    /// Comments in a script with a rest-element `$props()`.
    #[error("comments in a script with a rest-element $props() (injected $$slots/$$events)")]
    CommentsWithRestProps,
    /// Comments in a script with a non-destructured `$props()`.
    #[error("comments in a script with a non-destructured $props() (injected $$slots/$$events)")]
    CommentsWithNonDestructuredProps,
    /// Comments alongside expression-valued attributes.
    #[error("comments in a script alongside expression-valued attributes")]
    CommentsAlongsideExprAttributes,
    /// Comments alongside a `$$slots` reference (the injected
    /// `sanitize_slots` first statement would sweep the comment windows).
    #[error("comments in a script with a $$slots reference (injected sanitize_slots)")]
    CommentsWithSlots,
    /// A `format-ignore` directive comment in the script.
    #[error("format-ignore directive comment in script")]
    FormatIgnoreComment,
    /// Comments in template markup (only instance-script comments carry through).
    #[error("template comments (only instance-script comments are carried through)")]
    TemplateComments,

    // ── Template blocks / `{@const}` ───────────────────────────────────────
    /// A template node kind the transform does not emit.
    #[error("template node {kind}")]
    TemplateNode {
        /// The fragment node kind (`{@render} tag`, `special element`, …).
        kind: &'static str,
    },
    /// `{@const}` at the component root.
    #[error("{{@const}} at the component root (only valid inside a block)")]
    ConstTagAtRoot,
    /// A destructured `{@const}`.
    #[error("destructured {{@const}} (only `{{@const name = …}}`)")]
    DestructuredConstTag,
    /// `{@const}` with a non-plain binding name.
    #[error("{{@const}} with a non-plain binding name")]
    ConstTagNonPlainName,
    /// `{@const}` outside a block scope.
    #[error("{{@const}} outside a block scope")]
    ConstTagOutsideBlock,
    /// A nested `{#each}` (unique-name allocation order is not reproducible).
    #[error("nested {{#each}} (the oracle's unique-name allocation order is not reproducible)")]
    NestedEach,
    /// A block-scope binding shadows a `$derived` binding.
    #[error("block-scope binding {name} shadows a $derived binding")]
    BlockScopeShadowsDerived {
        /// The shadowing binding name.
        name: String,
    },

    // ── Template expressions ───────────────────────────────────────────────
    /// `{@html}` with a statically-known value (the oracle folds it).
    #[error("{{@html}} with a statically-known value")]
    HtmlTagStaticValue,
    /// A mutation inside a template expression.
    #[error("mutation inside a template expression")]
    MutationInTemplateExpr,
    /// A statically-known value whose byte-exact stringification is unproven.
    #[error("static evaluation not portable: {0}")]
    StaticEvalNotPortable(String),
    /// A static fold whose byte-exact stringification is unproven.
    #[error("static fold not portable: {0}")]
    StaticFoldNotPortable(String),

    // ── Attributes ─────────────────────────────────────────────────────────
    /// An `on`-prefixed event attribute with an expression value. Retained for
    /// the mixed-value shape (`onclick="a {b}"`), which the oracle rejects, so
    /// tsv refuses rather than guess; the single-expression shape is dropped.
    #[error("event attribute {name}")]
    EventAttribute {
        /// The event attribute name.
        name: String,
    },
    /// An `onload`/`onerror` handler on a load-error element (`img`, `iframe`,
    /// …): the oracle emits an `on{name}="this.__e=event"` capture attribute
    /// rather than dropping it, which tsv does not yet reproduce.
    #[error("{name} on a load-error element (event-capture markup not implemented)")]
    EventCaptureAttribute {
        /// The event attribute name (`onload` or `onerror`).
        name: String,
    },
    /// A directive or spread attribute.
    #[error("non-plain attribute (directive/spread)")]
    NonPlainAttribute,
    /// A string-literal expression attribute value (inline-literal path).
    #[error("string-literal expression attribute value (inline-literal path)")]
    StringLiteralExprAttribute,
    /// A dynamic `class` attribute on a styled component.
    #[error("dynamic class attribute on a styled component")]
    DynamicClassOnStyled,
    /// A dynamic `style` attribute on a styled component.
    #[error("dynamic style attribute on a styled component")]
    DynamicStyleOnStyled,
    /// An interpolated `class`/`style` attribute on a styled component.
    #[error("interpolated {name} attribute on a styled component")]
    InterpolatedAttrOnStyled {
        /// The attribute name (`class` or `style`).
        name: String,
    },
    /// A `value` attribute on `<textarea>`/`<select>`.
    #[error("value attribute on <{name}>")]
    ValueAttribute {
        /// The element name.
        name: String,
    },

    // ── Elements ───────────────────────────────────────────────────────────
    /// A component element (`<Foo>`, `<foo.bar>`).
    #[error("<{name}> component (component rendering not implemented)")]
    ComponentElement {
        /// The component tag as written.
        name: String,
    },
    /// A foreign-namespace element (SVG/MathML).
    #[error("<{name}> (foreign namespace)")]
    ForeignNamespace {
        /// The element name.
        name: String,
    },
    /// A populated `<select>`/`<optgroup>` (the oracle emits a `<!>` anchor).
    #[error("<{name}> with children (oracle emits a `<!>` anchor)")]
    ElementWithChildren {
        /// The element name.
        name: String,
    },
    /// A template-level `<script>`/`<style>` element.
    #[error("template-level <{name}>")]
    TemplateLevelElement {
        /// The element name.
        name: String,
    },
    /// Children on a void element.
    #[error("children on void element <{name}>")]
    VoidElementChildren {
        /// The void element name.
        name: String,
    },
    /// An `<option>` element (the oracle emits `$$renderer.option` closures).
    #[error("<option> (oracle emits $$renderer.option closures)")]
    OptionElement,

    // ── CSS scoping ────────────────────────────────────────────────────────
    /// An at-rule in `<style>`.
    #[error("css at-rule in <style>")]
    CssAtRule,
    /// A nested rule in `<style>`.
    #[error("nested css rule in <style>")]
    CssNestedRule,
    /// A combinator selector in `<style>`.
    #[error("css combinator selector in <style>")]
    CssCombinatorSelector,
    /// A non-class selector in `<style>` (only `.class` is supported).
    #[error("non-class css selector in <style> (only `.class` is supported)")]
    CssNonClassSelector,
    /// A scoped class selector that matches no element (pruning not implemented).
    #[error("css selector .{class} matches no element (pruning not implemented)")]
    CssSelectorNoMatch {
        /// The unmatched class name.
        class: String,
    },
}

impl Refusal {
    /// The stable corpus bucket key for this refusal.
    ///
    /// User-chosen identifiers collapse to a `{placeholder}` so many concrete
    /// refusals share one bucket; closed-set feature discriminants
    /// ([`TemplateNode`](Refusal::TemplateNode),
    /// [`BindingPatternShape`](Refusal::BindingPatternShape)) keep their full
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
            Self::TemplateNode { .. } | Self::BindingPatternShape { .. } => {
                Cow::Owned(self.to_string())
            }
            // Parameterized reasons — the user-chosen value collapses away.
            Self::LangInstanceScript { .. } => Cow::Borrowed("lang=\"{lang}\" instance script"),
            Self::GeneratedNameCollision { .. } => {
                Cow::Borrowed("generated name {name} collides with a user binding")
            }
            Self::Rune { .. } => Cow::Borrowed("rune {name}"),
            Self::DollarPrefixedIdentifier { .. } => Cow::Borrowed("$-prefixed identifier {name}"),
            Self::DerivedBindingRead { .. } => Cow::Borrowed("read of derived binding {name}"),
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
            Self::ComponentElement { .. } => Cow::Borrowed("<{name}> component"),
            Self::ForeignNamespace { .. } => Cow::Borrowed("<{name}> (foreign namespace)"),
            Self::ElementWithChildren { .. } => Cow::Borrowed("<{name}> with children"),
            Self::TemplateLevelElement { .. } => Cow::Borrowed("template-level <{name}>"),
            Self::VoidElementChildren { .. } => Cow::Borrowed("children on void element <{name}>"),
            Self::CssSelectorNoMatch { .. } => {
                Cow::Borrowed("css selector .{class} matches no element")
            }
            // Static reasons — the message is already the bucket.
            Self::ClientGeneration => Cow::Borrowed("client generation"),
            Self::DevMode => Cow::Borrowed("dev mode output"),
            Self::ModuleScript => Cow::Borrowed("module <script context=\"module\">"),
            Self::SvelteOptions => Cow::Borrowed("<svelte:options>"),
            Self::InstanceScriptExport => Cow::Borrowed(
                "instance-script export (component exports / $.bind_props not implemented)",
            ),
            Self::GenericsAttribute => {
                Cow::Borrowed("generics attribute on instance script (implies TypeScript)")
            }
            Self::TsEnumOrModule => Cow::Borrowed("TS enum/module declaration in instance script"),
            Self::PropsBindingPattern => Cow::Borrowed(
                "$props() binding pattern (not an identifier or object pattern — the oracle rejects it)",
            ),
            Self::DestructuringState => Cow::Borrowed("destructuring a $state declarator"),
            Self::DestructuringDerived => Cow::Borrowed("destructuring a $derived declarator"),
            Self::DestructuringDerivedBy => Cow::Borrowed("destructuring a $derived.by declarator"),
            Self::TopLevelAwait => {
                Cow::Borrowed("top-level await (async component output not implemented)")
            }
            Self::MemberCallEscapedRoot => Cow::Borrowed(
                "member/call rooted at an escaped identifier (classification not ported)",
            ),
            Self::CommentsAlongsideMultiDeclarator => Cow::Borrowed(
                "comments in a script alongside a multi-declarator declaration (the oracle re-anchors comments inside the split)",
            ),
            Self::CommentsAlongsideImports => Cow::Borrowed(
                "comments in a script alongside imports (placement around hoisted imports not carried through yet)",
            ),
            Self::CommentsAlongsideTemplateBlocks => Cow::Borrowed(
                "comments in a script alongside template blocks (placement not carried through yet)",
            ),
            Self::CommentsWithDerived => {
                Cow::Borrowed("comments in a script that uses $derived (not carried through yet)")
            }
            Self::CommentInRewrittenRuneRegion => {
                Cow::Borrowed("comment inside a rewritten rune region (dropped by the transform)")
            }
            Self::CommentAfterLastStatement => Cow::Borrowed(
                "comment after the last script statement (the oracle re-attaches it into the template)",
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
            Self::CommentsAlongsideExprAttributes => {
                Cow::Borrowed("comments in a script alongside expression-valued attributes")
            }
            Self::CommentsWithSlots => Cow::Borrowed(
                "comments in a script with a $$slots reference (injected sanitize_slots)",
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
            Self::NestedEach => Cow::Borrowed(
                "nested {#each} (the oracle's unique-name allocation order is not reproducible)",
            ),
            Self::HtmlTagStaticValue => Cow::Borrowed("{@html} with a statically-known value"),
            Self::MutationInTemplateExpr => Cow::Borrowed("mutation inside a template expression"),
            Self::NonPlainAttribute => Cow::Borrowed("non-plain attribute (directive/spread)"),
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
            Self::CssAtRule => Cow::Borrowed("css at-rule in <style>"),
            Self::CssNestedRule => Cow::Borrowed("nested css rule in <style>"),
            Self::CssCombinatorSelector => Cow::Borrowed("css combinator selector in <style>"),
            Self::CssNonClassSelector => {
                Cow::Borrowed("non-class css selector in <style> (only `.class` is supported)")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Refusal;

    #[test]
    fn display_substitutes_parameters() {
        assert_eq!(
            Refusal::EventAttribute {
                name: "onclick".to_string()
            }
            .to_string(),
            "event attribute onclick"
        );
        assert_eq!(
            Refusal::LangInstanceScript {
                lang: "ts".to_string()
            }
            .to_string(),
            "lang=\"ts\" instance script (type stripping not implemented)"
        );
        assert_eq!(
            Refusal::ComponentElement {
                name: "Foo.Bar".to_string()
            }
            .to_string(),
            "<Foo.Bar> component (component rendering not implemented)"
        );
        // Literal braces render verbatim.
        assert_eq!(
            Refusal::ConstTagOutsideBlock.to_string(),
            "{@const} outside a block scope"
        );
    }

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
            Refusal::ModuleScript.bucket_key(),
            "module <script context=\"module\">"
        );
    }
}
