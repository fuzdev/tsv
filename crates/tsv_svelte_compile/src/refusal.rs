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
    /// A top-level `$:` legacy reactive statement. Invalid in runes mode (the
    /// oracle rejects it), and cloning it through would emit a dead JS label
    /// with no reactivity — a silent mis-compile. Nested `$` labels and plain
    /// labels stay ordinary JS the oracle clones through.
    #[error("legacy reactive statement `$:` (invalid in runes mode)")]
    LegacyReactiveStatement,
    /// An import from `svelte/internal*` — private runtime code the oracle
    /// forbids in runes mode.
    #[error("import from svelte/internal (forbidden)")]
    SvelteInternalImport,
    /// A named import from `svelte` that is invalid in runes mode
    /// (`beforeUpdate`/`afterUpdate`; also an escaped imported name, which
    /// can't be classified from its raw span and refuses conservatively).
    #[error("runes-invalid import of {name} from svelte")]
    RunesInvalidImport {
        /// The offending imported name.
        name: String,
    },
    /// A `generics` attribute on the instance script (an open type-parameter
    /// binding, not annotation erasure — a separate slice).
    #[error("generics attribute on instance script (implies TypeScript)")]
    GenericsAttribute,
    /// An instance script with a `lang` other than `ts`/`js`/empty. The oracle's
    /// TypeScript flag tests `lang === 'ts'` **exactly**, so `lang="typescript"`
    /// and `lang="TS"` are not TypeScript to it; rather than compile them as
    /// plain JS on a guess, tsv refuses.
    #[error("lang=\"{lang}\" instance script (only ts/js supported)")]
    LangInstanceScript {
        /// The declared `lang` attribute value.
        lang: String,
    },

    // ── TypeScript: refuse-don't-erase ─────────────────────────────────────
    /// TypeScript syntax in a script the oracle does **not** parse as
    /// TypeScript (no `<script lang="ts">` anywhere in the document). tsv's
    /// parser is TypeScript-permissive and would accept it silently; the oracle
    /// hits a plain-JS parse error, so compiling it would be an over-acceptance.
    #[error("TypeScript syntax without lang=\"ts\" (the oracle parse-errors)")]
    TypeScriptWithoutLangTs,
    /// A comment inside an erased TypeScript region (or glued to its tail,
    /// before the next surviving token). The oracle's surviving-comment
    /// placement is an emergent artifact of its printer's flush points over
    /// stale spans — not a rule this transform can reproduce, so the class
    /// refuses rather than diverge.
    #[error("comment inside an erased TypeScript region")]
    CommentInErasedTypeRegion,
    /// A TypeScript `enum`. Lowers to an object plus a reverse mapping at
    /// runtime, so erasure would silently delete behavior — and the oracle
    /// rejects every enum outright (`typescript_invalid_feature`), `declare
    /// enum` included.
    #[error("TS enum (the oracle rejects it)")]
    TsEnum,
    /// A TypeScript `namespace`/`module` with any value member (it lowers to an
    /// IIFE). A **type-only** namespace erases away cleanly and compiles.
    #[error("TS namespace/module with a value member (the oracle rejects it)")]
    TsNamespaceWithValue,
    /// A dotted `namespace A.B { … }`. The oracle's strip visitor assumes a block
    /// body and calls `node.body.body.map(…)` on the nested module declaration —
    /// it **throws** (`node.body.body.map is not a function`), at any body
    /// content. Not a compilable shape, so refuse rather than guess.
    #[error("dotted TS namespace A.B (the oracle crashes on it)")]
    TsDottedNamespace,
    /// A constructor parameter property carrying `readonly` or an accessibility
    /// modifier (`constructor(public x: number)`) — real TypeScript synthesizes
    /// `this.x = x` into the body, so unwrapping to the bare parameter would drop
    /// behavior. Exactly the oracle's rule: a lone `override`, or a modifier
    /// outside a constructor, is unwrapped instead and compiles.
    #[error("TS parameter property with readonly/accessibility (the oracle rejects it)")]
    TsParameterProperty,
    /// A decorator. The oracle rejects every decorator
    /// (`typescript_invalid_feature`), and without `lang="ts"` it is a plain-JS
    /// parse error.
    #[error("decorator (the oracle rejects it)")]
    Decorator,
    /// An `accessor` class field (the ES decorator proposal) — a
    /// `typescript_invalid_feature` hard error in the oracle.
    #[error("accessor class field (the oracle rejects it)")]
    TsAccessorField,
    /// An `abstract` class *property*. The oracle's strip pass has no case for
    /// it: the member survives and prints as `abstract x;` — invalid JS. tsv
    /// refuses rather than reproduce a broken module. (An `abstract` *method* is
    /// dropped, matching the oracle — the split is by node kind.)
    #[error("abstract class property (the oracle emits invalid JS)")]
    TsAbstractProperty,
    /// A bodiless, non-`abstract` class method — an overload signature, or an
    /// ambient member outside a `declare` class. The oracle's strip pass has no
    /// case for it, so it survives and collides with the implementation
    /// (`duplicate_class_field`) or prints as invalid JS.
    #[error("bodiless class method (overload signature — the oracle rejects it)")]
    TsOverloadSignature,
    /// A class-body index signature (`[key: string]: T`). A pure type construct,
    /// but the oracle's strip pass has no case for it and its transform then
    /// crashes outright.
    #[error("index signature in a class body (the oracle crashes on it)")]
    TsIndexSignature,
    /// `import x = require('y')` / `import x = A.B`. CommonJS interop with
    /// runtime semantics that don't map to ESM; the oracle has no strip case, so
    /// it emits the statement verbatim inside the component function — invalid
    /// runtime JS. tsv refuses rather than reproduce it.
    #[error("import x = require(…) (the oracle emits invalid JS)")]
    TsImportEquals,
    /// `export = value`. Same class as [`Self::TsImportEquals`] — the oracle
    /// emits it verbatim inside the component function.
    #[error("export = … (the oracle emits invalid JS)")]
    TsExportAssignment,
    /// `export as namespace Foo`. Same class — no strip case in the oracle, so
    /// it lands inside the component function as invalid JS.
    #[error("export as namespace … (the oracle emits invalid JS)")]
    TsNamespaceExport,

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
    /// Comments alongside a component invocation (the component call's minted
    /// object-literal / borrowed prop-value spans would sweep the comment
    /// windows).
    #[error("comments in a script alongside a component invocation")]
    CommentsWithComponent,
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

    // ── Snippets / render tags ─────────────────────────────────────────────
    /// A `{#snippet}` whose signature head (`<T>(params)`) the parser could not
    /// parse: it kept the raw text instead of an AST, so there is nothing to
    /// erase or emit.
    #[error("{{#snippet}} signature the parser fell back to raw text for")]
    SnippetSignatureUnparsed,
    /// A `{#snippet}` whose name is an escaped identifier — the name-based port
    /// can't reproduce it.
    #[error("{{#snippet}} with an escaped name")]
    SnippetEscapedName,
    /// A `{#snippet}` with a **top-level** rest parameter (`{#snippet s(...xs)}`).
    /// The oracle rejects it in its analysis phase
    /// (`snippet_invalid_rest_parameter`). A rest element *nested* inside a
    /// destructuring parameter (`{#snippet s({ ...rest })}`) is legal and
    /// compiles — the oracle checks only the top level.
    #[error("{{#snippet}} rest parameter (the oracle rejects it)")]
    SnippetRestParameter,
    /// A `{#snippet}` whose hoist classification is ambiguous for the name-based
    /// port: a name it references is both an instance binding and a nested
    /// (non-parameter) local, so free-vs-shadowed can't be told apart.
    #[error(
        "{{#snippet}} {name} references a binding that is both an instance binding \
         and a nested local (hoist classification ambiguous)"
    )]
    SnippetHoistAmbiguous {
        /// The snippet name.
        name: String,
    },
    /// A `{#snippet}` sharing a fragment with a `{@const}`/`<svelte:head>` — the
    /// relative hoist order across kinds isn't reproduced.
    #[error("{{#snippet}} alongside a {{@const}}/<svelte:head> in the same fragment (hoist order)")]
    SnippetHoistOrder,
    /// A duplicate top-level `{#snippet}` name (the oracle rejects it).
    #[error("duplicate {{#snippet}} {name} (the oracle rejects it)")]
    DuplicateSnippetName {
        /// The duplicated snippet name.
        name: String,
    },
    /// A `{@render}` whose callee is neither a resolvable local snippet nor a
    /// snippet prop (a member callee or an unresolved identifier).
    #[error("{{@render}} callee is not a resolvable local snippet or snippet prop")]
    RenderTagUnsupportedCallee,
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
    /// A `use:` directive on a load-error element (`img`, `iframe`, …). The oracle
    /// adds `onload`/`onerror` capture attributes there (its `events_to_capture`
    /// set — `shared/element.js`), which tsv does not yet reproduce, so the `use:`
    /// drop that applies on every other element refuses here. Only `use:` (and a
    /// spread) triggers this; `transition:`/`in:`/`out:`/`animate:`/`{@attach}` on
    /// a load-error element drop cleanly.
    #[error("use: directive on a load-error element (event-capture markup not implemented)")]
    UseDirectiveOnLoadErrorElement,
    /// Two or more transition directives on one element claim the same animation
    /// channel. The oracle's phase-2 placement check (`shared/element.js:115-132`)
    /// runs before it discards the SSR visit: a `transition:` contributes both
    /// intro and outro, `in:` intro only, `out:` outro only, and a channel claimed
    /// twice is `transition_duplicate` (same signature) or `transition_conflict`
    /// (different) — both oracle-rejected, so a combination it rejects must refuse
    /// rather than drop and compile. tsv folds the whole union into one refusal;
    /// modifiers are irrelevant (direction alone decides). A single
    /// `transition:`/`in:`/`out:`, or an `in:`+`out:` pair, is legal.
    #[error(
        "conflicting transition directives (an element may have at most one intro \
         and one outro — the oracle rejects it)"
    )]
    TransitionDirectiveConflict,
    /// An `animate:` directive in a position the oracle rejects at phase-2
    /// (`shared/element.js:92-114`): it is legal only as the **sole** non-trivial
    /// child of a **keyed** `{#each}` body (comments, `{@const}`/declaration tags,
    /// and whitespace-only text are the trivial siblings the filter ignores), and
    /// only one may appear on the element — `animation_invalid_placement` /
    /// `animation_missing_key` / `animation_duplicate` respectively. Those checks
    /// run before the SSR visit is discarded, so a rejected placement must refuse
    /// rather than drop and compile.
    #[error(
        "invalid animate: directive (one per element, only on the sole child of a \
         keyed {{#each}} — the oracle rejects it)"
    )]
    AnimateDirectiveInvalid,
    /// A directive or spread attribute the transform does not yet emit —
    /// `class:`/`style:`/`bind:`, a legacy `on:` event directive, `let:`, or an
    /// element `{...spread}`. The no-op drop family (`use:`/`transition:`/`in:`/
    /// `out:`/`animate:`/`{@attach}`) is dropped, not refused, on a regular element.
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

    // ── Components ─────────────────────────────────────────────────────────
    /// A dynamic component — a member component (`<Foo.Bar>`) or a component
    /// whose name binding is reactive (a prop, `$state`/`$derived`, or a
    /// block-local): the oracle emits the `if (expr) {…}` truthiness guard with
    /// hydration anchors, not a plain `Name($$renderer, …)` call.
    #[error("dynamic <{name}> component (member or reactive binding)")]
    DynamicComponent {
        /// The component tag as written.
        name: String,
    },
    /// A `slot="…"` (named slot) on a component's child (the oracle groups it into
    /// a `$$slots.<name>` closure).
    #[error("named slot on <{name}> component")]
    ComponentNamedSlot {
        /// The component tag as written.
        name: String,
    },
    /// A component with both an explicit `children` prop and default children (the
    /// oracle routes the children to `$$slots.default` with a `children` error).
    #[error("<{name}> component with both a children prop and default children")]
    ComponentChildrenPropConflict {
        /// The component tag as written.
        name: String,
    },
    /// A `--custom-property` attribute on a component (the oracle wraps the call
    /// in `$.css_props`).
    #[error("--custom-property attribute on <{name}> component")]
    ComponentCustomProperty {
        /// The component tag as written.
        name: String,
    },
    /// A `bind:` directive on a component (the oracle emits a `do…while` settle
    /// loop).
    #[error("bind: directive on <{name}> component")]
    ComponentBindDirective {
        /// The component tag as written.
        name: String,
    },
    /// A non-`bind:` directive (`use:`/`transition:`/`class:`/…) on a component
    /// (mostly oracle-rejected input; refused rather than guessed).
    #[error("directive on <{name}> component")]
    ComponentDirective {
        /// The component tag as written.
        name: String,
    },

    // ── Elements ───────────────────────────────────────────────────────────
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
    /// Attributes on a `<svelte:head>` element (not carried in this subset).
    #[error("attributes on <svelte:head>")]
    SvelteHeadAttributes,
    /// A `<svelte:head>` sharing a fragment with a `{@const}` — their hoisted
    /// order can't be fixed.
    #[error("<svelte:head> alongside a {{@const}} in the same fragment (hoist order)")]
    SvelteHeadWithConstTag,

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
            Self::LegacyReactiveStatement => {
                Cow::Borrowed("legacy reactive statement `$:` (invalid in runes mode)")
            }
            Self::SvelteInternalImport => Cow::Borrowed("import from svelte/internal (forbidden)"),
            Self::RunesInvalidImport { .. } => {
                Cow::Borrowed("runes-invalid import of {name} from svelte")
            }
            Self::GenericsAttribute => {
                Cow::Borrowed("generics attribute on instance script (implies TypeScript)")
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
            | Self::TsNamespaceExport => Cow::Owned(self.to_string()),
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
            Self::CommentsWithComponent => {
                Cow::Borrowed("comments in a script alongside a component invocation")
            }
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
            Self::SvelteHeadAttributes => Cow::Borrowed("attributes on <svelte:head>"),
            Self::SvelteHeadWithConstTag => Cow::Borrowed(
                "<svelte:head> alongside a {@const} in the same fragment (hoist order)",
            ),
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
                lang: "typescript".to_string()
            }
            .to_string(),
            "lang=\"typescript\" instance script (only ts/js supported)"
        );
        assert_eq!(
            Refusal::DynamicComponent {
                name: "Foo.Bar".to_string()
            }
            .to_string(),
            "dynamic <Foo.Bar> component (member or reactive binding)"
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
