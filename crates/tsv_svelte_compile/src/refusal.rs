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
use std::collections::BTreeSet;

use crate::fragment::SPECIAL_ELEMENT_FENCED_KINDS;

/// The three [`Refusal::InvalidAssignmentTarget`] targets — one per oracle rule
/// in the `validate_assignment` family. A closed set, so each is its own bucket
/// key; naming them as constants keeps the refusal sites, the bucket-key catalog
/// (`Refusal::every_variant`) and the checklist document quoting one string.
pub(crate) const INVALID_ASSIGNMENT_CONSTANT: &str =
    "a constant (a const declarator or import local — the oracle's constant_assignment)";
/// See [`INVALID_ASSIGNMENT_CONSTANT`]. Runes-only in the oracle, which this
/// runes-only compiler is unconditionally.
pub(crate) const INVALID_ASSIGNMENT_EACH_ITEM: &str =
    "an {#each} item (the oracle's each_item_invalid_assignment)";
/// See [`INVALID_ASSIGNMENT_CONSTANT`].
pub(crate) const INVALID_ASSIGNMENT_SNIPPET_PARAMETER: &str =
    "a {#snippet} parameter (the oracle's snippet_parameter_assignment)";

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
    /// An `export default` in a `<script module>`. The oracle errors
    /// `module_illegal_default_export` (a component cannot have a default
    /// export), so refusing is never an over-acceptance.
    #[error("default export in <script module> (the oracle rejects it)")]
    ModuleDefaultExport,
    /// A top-level binding name declared in BOTH the module and instance scripts.
    /// The oracle resolves a template `{name}` read to the instance (inner-scope)
    /// binding, but the name-based binding table would overwrite it with the
    /// module binding and fold the module value — a real MISMATCH. The port can't
    /// disambiguate which scope a reference resolves to (a hoisted module-scope
    /// snippet may legitimately reference the module binding), so refuse.
    #[error("binding {name} declared in both the module and instance scripts")]
    ModuleInstanceNameCollision {
        /// The colliding binding name.
        name: String,
    },
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
    /// A `generics` attribute on a `<script>` (an open type-parameter binding,
    /// not annotation erasure — a separate slice). Refused on either script.
    #[error("generics attribute on <script> (implies TypeScript)")]
    GenericsAttribute,
    /// A `<script>` with a `lang` other than `ts`/`js`/empty (instance or module).
    /// The oracle's TypeScript flag tests `lang === 'ts'` **exactly**, so
    /// `lang="typescript"` and `lang="TS"` are not TypeScript to it; rather than
    /// compile them as plain JS on a guess, tsv refuses.
    #[error("lang=\"{lang}\" script (only ts/js supported)")]
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
    /// Destructuring a `$state.snapshot(…)` declarator. The oracle lowers
    /// `const {a} = $state.snapshot(x)` into a temp-destructure
    /// (`const tmp = x, a = tmp.a`), a shape this transform does not reproduce (a
    /// safe over-refusal); a plain-identifier target unwraps to the argument.
    #[error("destructuring a $state.snapshot declarator")]
    DestructuringStateSnapshot,
    /// Destructuring a `$derived(…)` declarator.
    #[error("destructuring a $derived declarator")]
    DestructuringDerived,
    /// Destructuring a `$derived.by(…)` declarator.
    #[error("destructuring a $derived.by declarator")]
    DestructuringDerivedBy,
    /// A `$props.id()` in a position other than a plain-identifier declarator init
    /// (a destructure, or a nested/non-declarator use). The oracle's
    /// `props_id_invalid_placement` restricts it to a top-level variable
    /// declarator with an identifier target.
    #[error("$props.id() outside a plain top-level variable declaration")]
    PropsIdBindingPattern,
    /// A second `$props.id()` in the component (the oracle's `props_duplicate`).
    #[error("$props.id() used more than once")]
    DuplicatePropsId,
    /// A second `$props()` in the component (the oracle's `props_duplicate`, raised
    /// from its analyze-phase `CallExpression` visitor before the placement check).
    #[error("$props() used more than once")]
    DuplicateProps,
    /// A class-field `$state(…)` / `$state.raw(…)` whose **whole** argument is a
    /// lone reactive-binding identifier — a store read (`$state($count)`) or a
    /// `$derived` binding (`$state(d)`). The oracle keeps such a lone reactive read
    /// **bare** in the unwrapped field (`x = $count` / `x = d`), NOT feeding it
    /// through the store-subscription / derived-call pass — unlike a top-level
    /// `let` declarator, a plain field, or a compound argument (`$state($count +
    /// 1)`), where the read IS rewritten. tsv's store rewrite descends into class
    /// bodies unconditionally, so it would rewrite the kept argument
    /// (`$.store_get(…)` / `d()`) — a MISMATCH. Refused: a narrow, safe
    /// over-refusal (a compound or plain-variable argument still compiles).
    #[error("class-field $state with a lone store/$derived argument (the oracle keeps it bare)")]
    ClassFieldStateReactiveArg,

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
    /// A `$`-prefixed **binding** name — the oracle's `dollar_prefix_invalid`
    /// (`phases/2-analyze/visitors/shared/utils.js:278`). Distinct from
    /// [`Self::DollarPrefixedIdentifier`], which is a *read*: the oracle accepts
    /// a `$$slots` read and rejects a `$$slots` declaration, so the two
    /// positions cannot share one verdict.
    #[error("$-prefixed binding {name}")]
    DollarPrefixedBinding {
        /// The `$`-prefixed binding name.
        name: String,
    },
    /// A `$derived` binding read in a position the value-walk does not rewrite to
    /// `d()` — a pattern default, a read under an unsupported wrapper, an
    /// escaped-identifier read whose decoded name is a `$derived` binding, or a
    /// write to the derived binding itself (`d = v` / `d++`, which the oracle
    /// lowers to `d(v)` / `$.update_derived(d)` — not implemented).
    #[error("read of derived binding {name} (unsupported read position)")]
    DerivedBindingRead {
        /// The derived binding name.
        name: String,
    },
    /// A `$derived` binding whose name is also declared in a nested scope of the
    /// emitted script (a parameter or nested local). The script-position derived
    /// read rewrite is name-based, so it can't tell a read of the derived from a
    /// read of the shadowing binding — rewriting the latter to `d()` would
    /// MISMATCH. Shadowing a derived is legal (unlike a store), so this is a
    /// tsv-side over-refusal, kept narrow (checks `nested_declared`) and rare.
    #[error("read of derived binding {name} shadowed in a nested scope")]
    DerivedReadShadowed {
        /// The shadowed derived binding name.
        name: String,
    },
    /// A rune keyword whose `$`-stripped stem is also a binding **in scope at the
    /// instance script** — `import { state } from './store'` beside a `$state`
    /// reference. The oracle's `analyze_component`
    /// (`phases/2-analyze/index.js`) reclassifies such a `$stem` reference as a
    /// STORE subscription rather than the rune, and deletes it from
    /// `module.scope.references` *before* it infers runes mode — so the
    /// collision can also flip the whole component out of runes mode. tsv is a
    /// runes-only compiler and models neither the reclassification nor mode
    /// inference, so refuse. The oracle EXEMPTS the common shapes — `let state =
    /// $state(0)`, `const props = $props()` — because the binding's own
    /// initializer is a rune call; those keep compiling.
    ///
    /// Scope is the oracle's `instance.scope.get`, which walks UP the chain
    /// (`phases/scope.js:748`) into the MODULE scope — so a `<script module>`
    /// binding collides too. It never walks DOWN, so a function parameter, a
    /// block-scoped `let`, or a name bound in a nested function body is a child
    /// scope and does not collide. Two nested forms still reach script scope: a
    /// function-scoped `var` in any block, for-head, switch, or try/catch (which
    /// arrives with its initializer DROPPED, `scope.js:673-681`, so the rune
    /// exemption can never apply to it), and a declaration in a class STATIC
    /// BLOCK, which the oracle gives no scope at all (no `StaticBlock` visitor)
    /// so the initializer survives. The first is modelled exactly; the second is
    /// covered by a lexical fence — a component containing ANY static block
    /// refuses on the first rune reference, a deliberate over-refusal at
    /// measured-zero corpus cost (see `script_rewrite`'s
    /// `script_contains_static_block`).
    ///
    /// ⚠️ The fence is a SOURCE SCAN, so its completeness is exactly the
    /// completeness of its whitespace class: the trivia between `static` and its
    /// `{` must be matched with ECMAScript's `WhiteSpace`/`LineTerminator`
    /// (`text_class::is_js_whitespace`), never Rust's `char::is_whitespace`.
    /// The two differ at `U+FEFF`, and a `static\u{FEFF}{ … }` block written with
    /// one was invisible to the fence — the rune compiled where the oracle emits
    /// a store read (pinned by
    /// `compile_refuses_static_block_separated_by_zwnbsp`).
    #[error("rune {name} whose base is also an instance binding (the oracle reads it as a store)")]
    RuneNameBoundAsStore {
        /// The rune keyword (`$state`, `$derived`, …).
        name: String,
    },
    /// A top-level `await` (async component output is not implemented).
    #[error("top-level await (async component output not implemented)")]
    TopLevelAwait,
    /// A `$name` store subscription whose `$`-stripped base is bound in a nested
    /// scope rather than at the component top level (the oracle's
    /// `store_invalid_scoped_subscription` error). Refused by the name-based
    /// shadow check — a base declared inside any function/block subtree is
    /// ambiguous, so refuse rather than read the wrong binding.
    #[error("store subscription $name whose base is not a top-level component binding")]
    StoreScopedSubscription,
    /// A store write to a member target (`$obj.foo = …` / `$obj.foo++`). The
    /// oracle emits `$.store_mutate(…)`; not implemented, so refuse.
    #[error("store member write ($store.x = …) — $.store_mutate not implemented")]
    StoreMemberWrite,
    /// A store write through a destructuring assignment target (`[$count] = …`,
    /// `({ x: $count } = …)`). The oracle builds an IIFE/sequence; not
    /// implemented, so refuse.
    #[error("store destructuring write ([$store] = …) not implemented")]
    StoreDestructuringWrite,

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
    /// Comments in a script that makes a store reference. The `var $$store_subs;`
    /// injection (and the `$.store_get`/`$.store_set` mints) are synthetic
    /// (appendix-span) nodes whose leading comment window would sweep the carried
    /// script comments — a safe over-refusal, like [`Self::CommentsWithSlots`].
    /// Fires for a template-only `$name` read too (the var is injected all the
    /// same).
    #[error("comments in a script that references a store ($$store_subs injection)")]
    CommentsWithStore,
    /// A comment inside a rewritten (dropped) rune region.
    #[error("comment inside a rewritten rune region (dropped by the transform)")]
    CommentInRewrittenRuneRegion,
    /// A comment after the last surviving script statement in a component whose
    /// template emits a nested block (the oracle drops it).
    #[error(
        "comment after the last script statement in a template that emits a nested block (the oracle drops it)"
    )]
    CommentAfterLastStatementWithBlock,
    /// A comment inside a `<script module>` that sits AFTER the instance
    /// `<script>` in source. The oracle drops a module comment only when its
    /// printer's comment index has already advanced past it; with the module
    /// second, the component body block (which carries the instance script's
    /// `loc`) re-seeks the index BACKWARD over the comment, and esrap then
    /// re-attaches it to the next loc-bearing node it reaches — a template
    /// expression the comment has nothing to do with. tsv drops it either way, so
    /// this ordering is a comment PRESENCE difference the parity bar grades as a
    /// mismatch.
    #[error(
        "comment in a module script placed after the instance script (the oracle re-attaches it into the template)"
    )]
    ModuleCommentAfterInstanceScript,
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
    /// Comments in a script with a `$props.id()` declarator. The hoisted
    /// `const <name> = $.props_id($$renderer)` is a synthetic first statement whose
    /// leading comment window would sweep the carried script comments — a safe
    /// over-refusal, like [`Self::CommentsWithSlots`].
    #[error("comments in a script with a $props.id() declarator")]
    CommentsWithPropsId,
    /// Comments in a script with a `$bindable()` prop default. The bindable
    /// rewrite mints an appendix `void 0` and rewrites the `$bindable(...)` call
    /// syntax inside the destructure pattern, so a carried comment's window would
    /// sweep those synthetic spans — a safe over-refusal.
    #[error("comments in a script with a $bindable() prop default")]
    CommentsWithBindable,
    /// Comments alongside a `$$slots` reference (the injected
    /// `sanitize_slots` first statement would sweep the comment windows).
    #[error("comments in a script with a $$slots reference (injected sanitize_slots)")]
    CommentsWithSlots,
    /// A multi-line block comment in the script (the oracle re-indents its
    /// interior lines to the emit position; tsv carries them verbatim).
    #[error(
        "multi-line block comment in script (interior-line re-indentation not carried through)"
    )]
    MultilineBlockComment,
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
    /// A nested `{#each}` — the emission path is not yet validated.
    ///
    /// This refusal originally read "the oracle's unique-name allocation order is
    /// not reproducible". That claim is **false** and was retired: the two orders
    /// are now both modelled (`each_array` pre-order at emission, `$$index`
    /// post-order upfront — see `blocks::assign_each_index_names`), and a nested
    /// `{#each}` probes at parity. What remains unvalidated is the rest of the
    /// nested emission surface (a keyed inner each, `animate:` placement,
    /// `{@const}` overlay nesting), which carries no fixture coverage — so this
    /// stays a deliberate, safe over-refusal until that coverage exists, NOT a
    /// statement that parity is unreachable.
    #[error("nested {{#each}} (the nested emission path is not yet validated)")]
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

    /// An assignment / update / `bind:` whose target the oracle rejects outright
    /// — its `validate_assignment` family
    /// (`phases/2-analyze/visitors/shared/utils.js:18-120`, reached from
    /// `AssignmentExpression`, `UpdateExpression`, and `BindDirective`). Three
    /// oracle rules share one refusal because they share one call site and one
    /// question ("may this name be written?"): `constant_assignment` (a `const`
    /// declarator or import local), `each_item_invalid_assignment` (an `{#each}`
    /// context binding, runes-only), and `snippet_parameter_assignment` (a
    /// `{#snippet}` parameter). A closed set, so each keeps its own bucket key.
    ///
    /// ⚠️ The rule is **name-based** where the oracle is scope-sensitive, so a
    /// local that merely shares a name with an immutable binding over-refuses.
    /// Safe by the refusal contract, and corpus-reachable rather than
    /// theoretical: it costs one parity point over the compile corpus (a helper
    /// function reusing a component-level name). See
    /// `../../docs/checklist_svelte_compiler.md` §The `validate_assignment`
    /// family.
    #[error("assignment to {target}")]
    InvalidAssignmentTarget {
        /// What the target is — a closed set of three phrases, one per oracle
        /// rule.
        target: &'static str,
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
    /// A **deliberate runes-only fence**, not a gap: a legacy `on:` event
    /// directive or `let:`. The oracle still compiles both in runes mode, but they
    /// are deprecated Svelte-4 syntax and tsv's compiler is runes-only by product
    /// choice — migrate to `onclick={fn}` / the runes event attribute, and to
    /// `{#snippet}`. Because it is a choice rather than an unimplemented feature,
    /// it is [`is_deliberate_fence`](Refusal::is_deliberate_fence) and belongs
    /// OUTSIDE the achievable-parity denominator.
    ///
    /// Everything else on an element is handled: the no-op drop family
    /// (`use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`) is dropped, not
    /// refused, on a regular element; `class:`/`style:` on a regular element are
    /// emitted (`$.attr_class`/`$.attr_style`), `bind:` is handled by
    /// [`BindDirective`](Refusal::BindDirective) (a handled core kind emits,
    /// everything else refuses), and an element `{...spread}` routes to the fused
    /// `$.attributes({…})` object-builder — `class:`/`style:` become its `classes`/
    /// `styles` arguments and `bind:` folds into the object, so a spread co-present
    /// with those compiles; a legacy `on:`/`let:` alongside a spread still refuses
    /// here.
    #[error("legacy {directive} directive (runes-only fence)")]
    RunesOnlyFence {
        /// The fenced directive prefix as authored — `on:` or `let:`. A closed
        /// set, so each keeps its own bucket key.
        directive: &'static str,
    },
    /// An element `{...spread}` on a `<select>`. The oracle routes a spread (or a
    /// bind) on a select through `$$renderer.select(object, ($$renderer) => …)`, a
    /// different callee than `$.attributes` — not implemented, so refuse.
    #[error("{{...spread}} on <select> (the oracle routes to $$renderer.select)")]
    SpreadOnSelect,
    /// An element `{...spread}` on a load-error element (`img`, `iframe`, …). The
    /// oracle adds `onload`/`onerror` capture attributes there (its
    /// `events_to_capture` set — a spread triggers it like a `use:`), which tsv
    /// does not yet reproduce.
    #[error("{{...spread}} on a load-error element (event-capture markup not implemented)")]
    SpreadOnLoadErrorElement,
    /// A `bind:` directive on a regular element outside the handled core set. The
    /// handled kinds emit (`bind:this` omits; `bind:value`/`bind:checked`/
    /// `bind:group` on `<input>` synthesize a `$.attr(...)`); everything else — a
    /// bind on a non-`<input>` target, `value` on `<textarea>`/`<select>`, the
    /// `omit_in_ssr` media/dimension binds, the content-editable trio, `open`,
    /// `focused`, an invalid target/type, or a bind expression that isn't a
    /// `$state`-rooted lvalue — refuses. The `{name}` collapses in the bucket key.
    #[error("bind: directive {name}")]
    BindDirective {
        /// The bind property name (`value`, `checked`, `clientWidth`, …).
        name: String,
    },
    /// A `class:` directive alongside a **mixed-value** `class="a {b}"` attribute.
    /// The oracle passes the mixed template value to `build_attr_class` as the
    /// base; tsv defers reproducing that rare shape rather than build it.
    #[error("class: directive alongside a mixed-value class attribute")]
    ClassDirectiveWithMixedClass,
    /// A `style:` directive alongside a **mixed-value** `style="a {b}"` attribute.
    /// The oracle passes the mixed template value to `build_attr_style` as the
    /// base; tsv defers reproducing that rare shape rather than build it.
    #[error("style: directive alongside a mixed-value style attribute")]
    StyleDirectiveWithMixedStyle,
    /// A `style:` directive with a **mixed-value** `style:color="a {b}"` value
    /// (text interleaved with an expression). The oracle builds a template
    /// concatenation for the property value; tsv defers that rare shape.
    #[error("style: directive with a mixed-value (text + expression) value")]
    StyleDirectiveWithMixedValue,
    /// A `style:` directive with an invalid modifier — the oracle allows only a
    /// single `|important`, so any other modifier, or two or more modifiers,
    /// is `style_directive_invalid_modifier` (an oracle error).
    #[error("style: directive with an invalid modifier (only |important, once, is allowed)")]
    StyleDirectiveInvalidModifier,
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
    /// An attribute on `<svelte:boundary>` outside the oracle's closed valid set
    /// (`onerror`/`failed`/`pending`) — its
    /// `svelte_boundary_invalid_attribute` analysis error. Covers an unknown plain
    /// attribute, a `{...spread}`, and every directive; tsv's parser accepts all
    /// three, so the compiler refuses rather than emit for oracle-rejected input.
    #[error("invalid attribute on <svelte:boundary> (the oracle rejects it)")]
    BoundaryInvalidAttribute,
    /// A valid-named `<svelte:boundary>` attribute whose value is not exactly one
    /// `{expression}` — a boolean attribute, a static string, or a mixed
    /// text/expression value. The oracle's
    /// `svelte_boundary_invalid_attribute_value` analysis error.
    #[error("non-expression value for <svelte:boundary> attribute {name} (the oracle rejects it)")]
    BoundaryInvalidAttributeValue {
        /// The attribute name (`onerror`/`failed`/`pending`).
        name: String,
    },
    /// The `failed={expr}` / `pending={expr}` **attribute** forms of
    /// `<svelte:boundary>`. The snippet forms compile; the attribute forms are a
    /// deliberate v1 gap — their precedence against a same-named snippet is
    /// asymmetric (`failed`: the snippet wins; `pending`: the attribute wins) and a
    /// statically-nullish `pending` emits an extra `if`/`else` fork keyed on the
    /// evaluator, so they are refused rather than guessed.
    #[error("<svelte:boundary> {name}={{…}} attribute form")]
    BoundaryAttributeSnippet {
        /// `failed` or `pending`.
        name: &'static str,
    },
    /// An attribute on a `<title>` element. The oracle rejects every attribute on
    /// `<title>` in its analysis phase (`title_illegal_attribute`); tsv's parser
    /// accepts them, so the compiler refuses rather than emit for oracle-rejected
    /// input.
    #[error("attribute on <title> (the oracle rejects it)")]
    TitleAttributes,
    /// A `<title>` child that is neither `Text` nor `ExpressionTag` (an element, a
    /// block, an `{@html}`, …). The oracle rejects it in its analysis phase
    /// (`title_invalid_content`); tsv's parser accepts it, so the compiler refuses
    /// rather than emit for oracle-rejected input.
    #[error("invalid <title> content (only text and {{expression}} — the oracle rejects it)")]
    TitleInvalidContent,
    /// A `<svelte:head>` sharing a fragment with a `{@const}` — their hoisted
    /// order can't be fixed.
    #[error("<svelte:head> alongside a {{@const}} in the same fragment (hoist order)")]
    SvelteHeadWithConstTag,
    /// An SSR-inert special element (`<svelte:window>`/`<svelte:body>`/
    /// `<svelte:document>`) nested inside an element/block/snippet. These are legal
    /// only as a direct child of the component root; the oracle errors
    /// `svelte_meta_invalid_placement` at analysis. tsv's parser is permissive about
    /// placement, so the compiler refuses the nested case rather than emit nothing
    /// for oracle-rejected input.
    #[error("<{name}> must be a top-level element (the oracle rejects it)")]
    SpecialElementInvalidPlacement {
        /// The special-element tag (`svelte:window`, …).
        name: String,
    },
    /// Markup a browser would REPAIR by moving, removing, or inserting elements —
    /// the oracle's `node_invalid_placement` (`2-analyze/visitors/RegularElement.js`,
    /// `Text.js`, `ExpressionTag.js`, over the tables in `src/html-tree-validation.js`).
    /// tsv's parser imposes no HTML content model, so the compiler refuses rather
    /// than emit output for a component the oracle rejects.
    ///
    /// The `message` is the oracle's own, so a refusal names the offending pair.
    /// It does NOT vary the bucket key — every placement violation shares one
    /// bucket.
    #[error("{message} (the oracle rejects it)")]
    NodeInvalidPlacement {
        /// The oracle's message, e.g. "`<div>` cannot be a descendant of `<p>`".
        message: String,
    },
    /// Two attributes of the same kind and name on one element — the oracle's
    /// parse-time `attribute_duplicate` (`phases/1-parse/state/element.js:250`).
    /// tsv's parser is permissive here, so the compiler refuses rather than emit
    /// output for a component the oracle rejects.
    /// An attribute name carrying a character the oracle forbids — its phase-2
    /// `attribute_invalid_name` (`2-analyze/visitors/shared/element.js:59`,
    /// `regex_illegal_attribute_character`). tsv's parser is permissive here, so
    /// the compiler refuses rather than emit output for a component the oracle
    /// rejects.
    ///
    /// ⚠️ Scoped to a REGULAR element and `<svelte:element>` — the oracle reaches
    /// the rule only from `RegularElement.js` / `SvelteElement.js`, never from its
    /// Component visitor, so a component prop may legally carry a name no element
    /// attribute could.
    #[error("invalid attribute name `{name}` (the oracle rejects it)")]
    AttributeInvalidName {
        /// The offending attribute name.
        name: String,
    },
    /// A `slot="…"` attribute whose slot OWNER is not its direct parent — the
    /// oracle's `slot_attribute_invalid_placement`
    /// (`2-analyze/visitors/shared/attribute.js:90,123`).
    ///
    /// ⚠️ Distinct from the [`Self::ComponentNamedSlot`] FENCE, and the split is
    /// load-bearing for the achievable-parity denominator. The fence covers the
    /// shape the oracle **accepts** — a slot attribute on a component's direct
    /// child — which tsv declines by product choice. This variant covers the
    /// shapes the oracle **rejects**, which are ordinary over-acceptance debt and
    /// must NOT be fenced: counting them as permanent scope would shrink the
    /// denominator and flatter the parity rate.
    #[error("misplaced slot=\"…\" attribute (the oracle rejects it)")]
    SlotAttributeInvalidPlacement,
    #[error("duplicate `{name}` attribute on one element (the oracle rejects it)")]
    DuplicateAttribute {
        /// The repeated attribute/directive name.
        name: String,
    },
    /// A second `<svelte:window>`/`<svelte:body>`/`<svelte:document>` of the same
    /// kind in the component (the oracle errors `svelte_meta_duplicate`: a component
    /// may have at most one of each). tsv's parser accepts it, so the compiler
    /// refuses the duplicate rather than emit nothing for oracle-rejected input.
    #[error("duplicate <{name}> element (the oracle rejects it)")]
    DuplicateSpecialElement {
        /// The special-element tag (`svelte:window`, …).
        name: String,
    },
    /// Children on an SSR-inert special element (`<svelte:window>`/`<svelte:body>`/
    /// `<svelte:document>`). The oracle rejects them (`svelte_meta_invalid_content`:
    /// these elements cannot have children); tsv's parser parses them into the
    /// element's fragment, so the compiler refuses rather than emit nothing for
    /// oracle-rejected input.
    #[error("<{name}> cannot have children (the oracle rejects it)")]
    SpecialElementChildren {
        /// The special-element tag (`svelte:window`, …).
        name: String,
    },
    /// An illegal attribute on an SSR-inert special element
    /// (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`): a spread, or a plain
    /// attribute that is not a modern event attribute (`on*={expr}`). The oracle
    /// rejects it (`illegal_element_attribute` / `svelte_body_illegal_attribute`);
    /// tsv's parser accepts it, so the compiler refuses rather than emit nothing for
    /// oracle-rejected input.
    #[error("invalid attribute on <{name}> (the oracle rejects it)")]
    SpecialElementIllegalAttribute {
        /// The special-element tag (`svelte:window`, …).
        name: String,
    },
    // ── CSS scoping ────────────────────────────────────────────────────────
    /// An at-rule in `<style>`.
    #[error("css at-rule in <style>")]
    CssAtRule,
    /// A nested rule in `<style>`.
    #[error("nested css rule in <style>")]
    CssNestedRule,
    /// A rule with no declarations (`.foo {}` / only comments). The oracle
    /// comment-wraps it `/* (empty) … */` in non-dev mode; tsv declines to
    /// reproduce the wrap and refuses.
    #[error("empty css rule in <style> (the oracle comment-wraps it)")]
    CssEmptyRule,
    /// A combinator selector in `<style>`.
    #[error("css combinator selector in <style>")]
    CssCombinatorSelector,
    /// A selector shape outside the supported same-element set: `:global`,
    /// `:is`/`:where`/`:has`/`:not`, `:root`/`:host`, nesting (`&`), a namespaced
    /// or escaped name, an `An+B`/percentage/invalid simple selector, or a bare
    /// pseudo-only compound. Type/id/class/attribute/universal compounds (plus
    /// trailing pseudo) are supported.
    #[error("unsupported css selector in <style> (:global/:is/:where/:has/:not/:root/nesting)")]
    CssUnsupportedSelector,
    /// An attribute selector matched against a dynamic, potentially-enumerable
    /// attribute value — the oracle's `get_possible_values` bounded static-eval,
    /// which tsv declines to port (refusing rather than risk a false match).
    #[error("css attribute selector against a dynamic attribute value (static-eval not ported)")]
    CssDynamicAttributeMatch,
    /// A case-insensitive attribute match with a non-ASCII operand (the selector
    /// name/value or the element's attribute name/value). The oracle folds case
    /// with full-Unicode `.toLowerCase()`; tsv folds ASCII-only, which can
    /// disagree (final-sigma, İ, Kelvin/Ångström, …), so a non-ASCII operand
    /// refuses rather than risk a mis-fold — a safe over-refusal.
    #[error("css case-insensitive match with a non-ASCII operand (Unicode case-fold not ported)")]
    CssCaseInsensitiveNonAscii,
    /// A scoped selector that matches no element (pruning not implemented — the
    /// oracle comment-wraps the unused rule).
    #[error("css selector {selector} matches no element (pruning not implemented)")]
    CssSelectorNoMatch {
        /// The unmatched compound's source text.
        selector: String,
    },
}

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
            Self::SpecialElementInvalidPlacement { .. } => {
                Cow::Borrowed("<{name}> must be a top-level element (the oracle rejects it)")
            }
            Self::DuplicateSpecialElement { .. } => {
                Cow::Borrowed("duplicate <{name}> element (the oracle rejects it)")
            }
            Self::AttributeInvalidName { .. } => {
                Cow::Borrowed("invalid attribute name `{name}` (the oracle rejects it)")
            }
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
    ///   ([`SPECIAL_ELEMENT_FENCED_KINDS`](crate::fragment::SPECIAL_ELEMENT_FENCED_KINDS))
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
    /// list, so an omission silently narrows that audit's oracle. Add the variant
    /// here in the same change.
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
            Self::SpecialElementInvalidPlacement {
                name: "{name}".to_string(),
            },
            Self::DuplicateSpecialElement {
                name: "{name}".to_string(),
            },
            Self::AttributeInvalidName {
                name: "{name}".to_string(),
            },
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
            "lang=\"typescript\" script (only ts/js supported)"
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
            Refusal::ModuleDefaultExport.bucket_key(),
            "default export in <script module> (the oracle rejects it)"
        );
    }

    /// The fenced set is a product decision, so it is pinned by name rather than
    /// left to whatever the label table happens to contain.
    #[test]
    fn deliberate_fences_are_the_legacy_syntax_only() {
        use crate::fragment::{
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
