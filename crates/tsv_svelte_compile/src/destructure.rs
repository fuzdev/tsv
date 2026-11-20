//! Destructured rune declarators — the 1→N lowering, shared by `$derived` /
//! `$derived.by` and `$state` / `$state.raw` / `$state.snapshot`.
//!
//! Oracle phase 3, **server**: both branches of
//! `packages/svelte/src/compiler/phases/3-transform/server/visitors/VariableDeclaration.js`
//! — the `$derived`/`$derived.by` branch (lines 87–134) and
//! `create_state_declarators` (lines 229–247) — backed by the ONE shared
//! `utils/ast.js` `extract_paths` / `_extract_paths` (line 269) and `build_fallback`
//! (line 585). A single source declarator lowers to N synthetic declarators, one
//! per destructuring leaf. The two lowerings share the extractor and differ only in
//! how each leaf/array-intermediate is wrapped ([`Lowering`]):
//!
//! ```text
//! let {a, b} = $derived(o);   ->  let a = $.derived(() => o.a), b = $.derived(() => o.b);
//! let {a, b} = $state(o);     ->  let tmp = o, a = tmp.a, b = tmp.b;
//! ```
//!
//! The rule, mirrored from the oracle:
//!
//! - **Derived** ([`Lowering::Derived`]): `value` is the rune argument (`$derived`)
//!   or the compute function (`$derived.by`). Mint an intermediate `$$d` when the
//!   rune is `$derived.by`, OR when it is `$derived` and `value` is not a bare
//!   identifier; then `rhs = $$d()` and a `const $$d = $.derived(<init>)` leads the
//!   group (`<init>` reusing the identifier path's unthunk collapse). A
//!   bare-identifier `$derived(o)` projects directly from `o`. Each leaf is
//!   `$.derived(() => access)`; each array intermediate a `$$derived_array` derived.
//! - **State** ([`Lowering::State`]): ALWAYS mint a `tmp` holding the value
//!   (`let tmp = value`, `value` being the argument — `void 0` for the argless
//!   `$state()`, the argument for `$state.raw`/`$state.snapshot`, the snapshot
//!   wrapper simply dropped). `rhs = tmp`, and each leaf is a RAW `name = access`
//!   projection (no `$.derived` wrap); each array intermediate a RAW
//!   `$$array = $.to_array(...)` (a plain `const`, not a derived). A store/`$derived`
//!   read inside the value is lowered afterward by
//!   [`store_rewrite`](crate::store_rewrite) (`let {a} = $state(d)` → `let tmp = d(),
//!   a = tmp.a`), never special-cased here.
//!
//! `extract_paths(id, rhs)` yields **inserts** (array `$.to_array` intermediates)
//! and **paths** (leaf projections). Inserts are emitted first, then paths — the
//! oracle's declaration order.
//!
//! The `$$derived_array` intermediate is itself a derived binding, so every read of
//! it is a call: the projections are `$$derived_array()[i]` /
//! `$$derived_array().slice(i)` with the `()` baked in (the oracle relies on its
//! visitor to add it; tsv bakes it because a synthetic identifier is invisible to
//! the store rewrite's derived-read detection). `$$d` reads are likewise `$$d()`.
//! The state `$$array` is a plain `const`, so its reads are BARE `$$array[i]` /
//! `$$array.slice(i)` — no call.
//!
//! **Comment safety.** The lowering scatters the pattern leaves across N synthetic
//! declarators and mints `$$d`/`$$derived_array`/`tmp`/`$$array` intermediates whose
//! comment windows would sweep a carried script comment. Rather than reproduce the
//! oracle's emergent placement, the caller refuses
//! [`CommentsWithDestructuredDerived`](crate::Refusal::CommentsWithDestructuredDerived)
//! / [`CommentsWithDestructuredState`](crate::Refusal::CommentsWithDestructuredState)
//! when the script carries comments — a safe over-refusal (absent from the gating
//! Svelte corpus, though it occurs in ecosystem code), so every node built here
//! runs in a comment-free script and may mint freely.
//!
//! **Names.** `$$d`, `$$derived_array`, `tmp`, and `$$array` are allocated by
//! [`GeneratedNames`], mirroring the oracle's `scope.generate`: a per-base counter
//! plus a bump past any collision (`tmp`, then `tmp_1` for a second state
//! destructure; `$$array`, then `$$array_1`). A `$$`-prefixed user binding is
//! refused upstream (`dollar_prefix_invalid`), so a generated-vs-user collision is
//! unreachable for the `$$` bases; `tmp` dedups against the frozen user binding set
//! (`let tmp = 9` forces the generated one to `tmp_1`).

use std::collections::HashMap;

use tsv_ts::ast::{
    AstView, ExprId, Expression, ExpressionKind, ObjectPatternProperty, Property,
    VariableDeclarator,
};

use crate::CompileError;
use crate::analyze::{NameSet, RuneInit};
use crate::build::Builder;
use crate::refusal::Refusal;
use crate::rune_guard::{WalkCtx, walk_expression_guarded};
use crate::script_decls::plain_identifier_name;
use crate::script_rewrite::unthunk_callee;
use crate::transform_server::unsupported;

/// The oracle's `scope.generate` for the derived-destructure intermediates.
///
/// A component-wide, per-base counter (`$$d`, `$$derived_array`) that yields the
/// preferred name first and appends `_N` on collision, bumping past both prior
/// generated names and the frozen set of user binding names. Created once per
/// component (before the script-rewrite loop) so its counters persist across
/// declarators, exactly as the oracle's root counter does.
pub(crate) struct GeneratedNames<'a> {
    /// The collision floor — every top-level user binding name in scope
    /// (`bindings.names()`, the whole binding set rather than just stores),
    /// matching what the oracle's `scope.generate` dedups a fresh name against
    /// at the root scope. A `$$`-prefixed user binding is refused upstream, so a
    /// generated `$$`-name never actually collides; the floor is defensive only.
    taken: &'a NameSet,
    /// Names this allocator has already handed out (the primary collision set,
    /// carrying the `$$derived_array` → `$$derived_array_1` bump).
    generated: NameSet,
    /// Per-base next-suffix counter (the oracle's `root.next_counter`).
    counters: HashMap<String, usize>,
}

impl<'a> GeneratedNames<'a> {
    pub(crate) fn new(taken: &'a NameSet) -> Self {
        Self {
            taken,
            generated: NameSet::default(),
            counters: HashMap::new(),
        }
    }

    /// Generate a unique name for `base`, mirroring `scope.generate`.
    fn generate(&mut self, base: &str) -> String {
        let mut n = self.counters.get(base).copied().unwrap_or(0);
        let mut name = if n == 0 {
            n = 1;
            base.to_string()
        } else {
            let s = format!("{base}_{n}");
            n += 1;
            s
        };
        while self.taken.contains(&name) || self.generated.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        self.counters.insert(base.to_string(), n);
        self.generated.insert(name.clone());
        name
    }
}

/// Which rune family a destructure is being lowered for — the only axis on which
/// the shared [`Extractor`] forks. `$derived` wraps every leaf in
/// `$.derived(() => …)` and its array intermediates are themselves derived (read
/// with a `()` call); `$state` emits RAW projections and plain-`const` array
/// intermediates (read bare). `$.fallback` / `$.exclude_from_object` / object
/// member projection are identical on both.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lowering {
    Derived,
    State,
}

impl Lowering {
    /// The array-intermediate base name the array pattern mints (`scope.generate`
    /// base): `$$derived_array` (derived) vs `$$array` (state).
    fn array_base(self) -> &'static str {
        match self {
            Lowering::Derived => "$$derived_array",
            Lowering::State => "$$array",
        }
    }
}

/// Lower a destructured `$derived` / `$derived.by` declarator into N declarators,
/// pushed onto `declarations` (inserts first, then paths — the oracle's order).
///
/// The caller has already established the target is a pattern (not an identifier)
/// and that the script carries no comments. `ctx` is the script-body guard context
/// (store/derived reads exempt) reused to walk the pattern defaults and the rune
/// argument for stray runes.
pub(crate) fn expand_destructured_derived(
    b: &mut Builder<'_>,
    ctx: &mut WalkCtx<'_>,
    declarations: &mut Vec<VariableDeclarator>,
    declarator: VariableDeclarator,
    rune: RuneInit,
    source: &str,
    names: &mut GeneratedNames<'_>,
) -> Result<(), CompileError> {
    // The pattern's defaults are real value expressions — walk them for a stray
    // rune (`{a = $state(0)}`) or a top-level `await`. The binding leaves are
    // exempt (derived reads allowed; `$`-prefixed leaves already refused by
    // `refuse_dollar_binding_pattern` before this arm).
    let view = b.view();
    walk_expression_guarded(view, view.expr(declarator.id), ctx)?;

    let (value, is_derived, refusal) = match rune {
        RuneInit::Derived(expr) => (expr, true, Refusal::DestructuringDerived),
        RuneInit::DerivedBy(f) => (f, false, Refusal::DestructuringDerivedBy),
        // Only the two derived arms reach here (the caller matched them).
        _ => return Err(unsupported(Refusal::DestructuringDerived)),
    };
    // Guard the rune argument (a bare derived read is exempt — the store rewrite
    // lowers it to `base()` inside each projection).
    walk_expression_guarded(view, view.expr(value), ctx)?;

    // rhs: a bare-identifier `$derived(o)` projects directly from the host `o`;
    // every other shape mints `$$d` and projects from `$$d()`.
    let mint_d = !(is_derived && matches!(view.expr(value).kind, ExpressionKind::Identifier(_)));
    let rhs = if mint_d {
        let d_name = names.generate("$$d");
        // Mint the id BEFORE the init so the declarator span runs forward
        // (`id.start < init.end`) — the invariant `build_sanitize_slots_decl`
        // relies on, else the printer's `callee.end - decl.start` underflows.
        let d_id = Expression::from_identifier(b.ident(&d_name));
        let span = d_id.span();
        // `$.derived(<init>)`: `$derived` thunks the value (reusing the identifier
        // path's unthunk collapse); `$derived.by` passes the compute fn directly.
        let argument = if is_derived {
            match unthunk_callee(b.view(), b.view().expr(value)) {
                Some(callee) => callee,
                None => {
                    let thunk = b.thunk_on_body(value);
                    b.push(thunk)
                }
            }
        } else {
            value
        };
        let init = derived_wrap(b, argument);
        let id = b.push(d_id);
        let init = b.push(init);
        declarations.push(VariableDeclarator {
            id,
            init: Some(init),
            definite: false,
            span,
        });
        let callee = b.ident_expr(&d_name);
        let call = b.call_of(callee, &[], false);
        b.push(call)
    } else {
        value
    };

    Extractor::new(b, source, refusal, Lowering::Derived).extract_onto(
        declarator.id,
        rhs,
        names,
        declarations,
    )
}

/// Lower a destructured `$state` / `$state.raw` / `$state.snapshot` declarator into
/// N declarators via the oracle's `create_state_declarators`
/// (`VariableDeclaration.js:229-247`), pushed onto `declarations` (`tmp` first, then
/// inserts, then paths — the oracle's order).
///
/// Unlike the derived branch this ALWAYS mints a `tmp` holding the value
/// (`let tmp = value`) and projects raw from it (no `$.derived` wrap). `value` is
/// the argument (`void 0` for the argless `$state()`); the `$state.snapshot`
/// wrapper is simply dropped, its argument becoming `value` exactly like `$state`
/// — the two differ only in the leaf `initial` the binding table assigns (snapshot
/// never folds), decided upstream in `script_bindings`. A store/`$derived` read
/// inside `value` is lowered afterward by [`store_rewrite`](crate::store_rewrite)
/// (`let {a} = $state(d)` → `let tmp = d(), a = tmp.a`), never special-cased here.
///
/// The caller has already established the target is a pattern (not an identifier)
/// and that the script carries no comments. `ctx` is the script-body guard context
/// (store/derived reads exempt) reused to walk the pattern defaults and the rune
/// argument for stray runes.
pub(crate) fn expand_destructured_state(
    b: &mut Builder<'_>,
    ctx: &mut WalkCtx<'_>,
    declarations: &mut Vec<VariableDeclarator>,
    declarator: VariableDeclarator,
    rune: RuneInit,
    source: &str,
    names: &mut GeneratedNames<'_>,
) -> Result<(), CompileError> {
    // The pattern's defaults are real value expressions — walk them for a stray
    // rune (`{a = $state(0)}`) or a top-level `await`. The binding leaves are
    // exempt (`$`-prefixed leaves already refused by `refuse_dollar_binding_pattern`
    // before this arm).
    let view = b.view();
    walk_expression_guarded(view, view.expr(declarator.id), ctx)?;

    // `value` is the argument the `tmp` holds — `void 0` when argless. The rune's
    // argument is guard-walked (a bare store/derived read is exempt — the store
    // rewrite lowers it inside `let tmp = value` afterward). `$state.snapshot`'s
    // wrapper is dropped, so its argument becomes `value` just like `$state`.
    let (value, refusal) = match rune {
        RuneInit::State(Some(arg)) => {
            walk_expression_guarded(view, view.expr(arg), ctx)?;
            (arg, Refusal::DestructuringState)
        }
        RuneInit::State(None) => {
            let void_zero = b.void_zero();
            (b.push(void_zero), Refusal::DestructuringState)
        }
        RuneInit::StateSnapshot(arg) => {
            walk_expression_guarded(view, view.expr(arg), ctx)?;
            (arg, Refusal::DestructuringStateSnapshot)
        }
        // Only the three state arms reach here (the caller matched them).
        _ => return Err(unsupported(Refusal::DestructuringState)),
    };

    // `let tmp = value` leads the group. `tmp` is generated BEFORE `$$array` (the
    // oracle's order: `scope.generate('tmp')` runs before `extract_paths`, whose
    // `$$array` names the Extractor mints inline during the walk).
    let tmp_name = names.generate("tmp");
    // `tmp`'s init is the host `value` (not a mint), so — unlike the minted
    // `$$d` / `$$array` inits — this declarator does NOT run forward: the id is a
    // fresh appendix span (high) while `value` keeps its lower host span, so
    // `id.start > init.end`. Safe anyway, because the printer's underflow-prone
    // `callee.end - decl.span.start` (`variable.rs`) reads the reused host
    // STATEMENT span (`rewrite_script_statement`'s `span: decl.span`), never this
    // appendix id span.
    let tmp_id = Expression::from_identifier(b.ident(&tmp_name));
    let tmp_span = tmp_id.span();
    let id = b.push(tmp_id);
    declarations.push(VariableDeclarator {
        id,
        init: Some(value),
        definite: false,
        span: tmp_span,
    });
    // Every leaf projects from a fresh `tmp` read (a plain `const`, so no `()`).
    let rhs = b.ident_expr(&tmp_name);

    Extractor::new(b, source, refusal, Lowering::State).extract_onto(
        declarator.id,
        rhs,
        names,
        declarations,
    )
}

/// `$.derived(<argument>)` — the intermediate/insert wrapper (comment-free, so a
/// plain appendix mint is safe; byte-identical to the span-stealing form).
fn derived_wrap(b: &mut Builder<'_>, argument: ExprId) -> Expression {
    b.member_call("$", "derived", &[argument])
}

/// The port of the oracle's `_extract_paths`: it walks the binding pattern in
/// parallel with the access expression `rhs`, collecting `inserts` (array
/// `$.to_array` intermediates) and `paths` (leaf `const name = $.derived(() =>
/// access)` declarators), each already wrapped as its own declarator.
struct Extractor<'b, 'a> {
    b: &'b mut Builder<'a>,
    source: &'b str,
    /// The refusal for an unrecognized shape (per rune family).
    refusal: Refusal,
    /// Which rune family — the only fork (leaf/array-intermediate wrapping).
    mode: Lowering,
    inserts: Vec<VariableDeclarator>,
    paths: Vec<VariableDeclarator>,
}

impl<'b, 'a> Extractor<'b, 'a> {
    fn new(b: &'b mut Builder<'a>, source: &'b str, refusal: Refusal, mode: Lowering) -> Self {
        Self {
            b,
            source,
            refusal,
            mode,
            inserts: Vec::new(),
            paths: Vec::new(),
        }
    }

    /// Extract `param` against `expr`, then drain the products onto
    /// `declarations`: **inserts before paths**, the oracle's order.
    ///
    /// Both rune families end on this, so the order lives in one place — a site
    /// that drained them the other way round would reorder the emitted
    /// declarators against the oracle's, which is a MISMATCH rather than a
    /// cosmetic difference.
    fn extract_onto(
        mut self,
        param: ExprId,
        expr: ExprId,
        names: &mut GeneratedNames<'_>,
        declarations: &mut Vec<VariableDeclarator>,
    ) -> Result<(), CompileError> {
        self.extract(param, expr, names)?;
        declarations.extend(self.inserts);
        declarations.extend(self.paths);
        Ok(())
    }

    fn view(&self) -> AstView<'_> {
        self.b.view()
    }

    fn extract(
        &mut self,
        param: ExprId,
        expr: ExprId,
        names: &mut GeneratedNames<'_>,
    ) -> Result<(), CompileError> {
        let kind = self.view().expr(param).kind;
        match kind {
            // Leaf: `const <param> = $.derived(() => expr)`. A declaration pattern
            // never binds a member expression, so an Identifier is the only leaf.
            ExpressionKind::Identifier(_) => {
                self.push_path(param, expr);
                Ok(())
            }
            ExpressionKind::ObjectPattern(obj) => {
                // The properties, copied out: each projection pushes.
                let properties = self.view().pat_props(obj.properties).to_vec();
                for prop in &properties {
                    match prop {
                        ObjectPatternProperty::RestElement(rest) => {
                            // `$.exclude_from_object(expr, [<sibling key literals>])`.
                            let keys = self.rest_keys(&properties)?;
                            let rest_expr = self.exclude_from_object(expr, &keys);
                            self.extract(rest.argument, rest_expr, names)?;
                        }
                        ObjectPatternProperty::Property(p) => {
                            let member = self.object_member(expr, p)?;
                            self.extract(p.value, member, names)?;
                        }
                    }
                }
                Ok(())
            }
            ExpressionKind::ArrayPattern(arr) => {
                // Derived: `const $$derived_array = $.derived(() => $.to_array(expr[,
                // len]))`. State: `const $$array = $.to_array(expr[, len])` (a plain
                // `const`, no `$.derived` wrap). The length is omitted when the last
                // element is a rest.
                let elements = self.view().expr_opt_list(arr.elements).to_vec();
                let has_rest = matches!(
                    elements.last(),
                    Some(Some(last))
                        if matches!(self.view().expr(*last).kind, ExpressionKind::RestElement(_))
                );
                let len = (!has_rest).then_some(elements.len());
                let name = names.generate(self.mode.array_base());
                // Mint the id BEFORE the `$.to_array` / `$.derived` init so the
                // declarator span runs forward (see the `$$d` mint's forward-span
                // note in `expand_destructured_derived`).
                let id = Expression::from_identifier(self.b.ident(&name));
                let id_span = id.span();
                let to_array = self.build_to_array(expr, len);
                let init = match self.mode {
                    Lowering::Derived => {
                        let value = self.b.push(to_array);
                        self.derived_of(value)
                    }
                    Lowering::State => to_array,
                };
                let id = self.b.push(id);
                let init = self.b.push(init);
                self.inserts.push(VariableDeclarator {
                    id,
                    init: Some(init),
                    definite: false,
                    span: id_span,
                });

                for (i, element) in elements.into_iter().enumerate() {
                    let Some(element) = element else {
                        continue; // a hole (`[a, , b]`) projects nothing
                    };
                    let kind = self.view().expr(element).kind;
                    if let ExpressionKind::RestElement(rest) = kind {
                        let slice = self.array_rest(&name, i);
                        self.extract(rest.argument, slice, names)?;
                    } else {
                        let index = self.array_index(&name, i);
                        self.extract(element, index, names)?;
                    }
                }
                Ok(())
            }
            ExpressionKind::AssignmentPattern(assign) => {
                // A default: project `$.fallback(expr, <default>)`, then recurse on
                // the left (Identifier → leaf, nested → recurse).
                let fallback = self.build_fallback(expr, assign.right);
                self.extract(assign.left, fallback, names)
            }
            // Any other shape (a computed/literal/escaped object key routes through
            // `object_member`'s refusal; a stray node here) is unrecognized.
            _ => Err(unsupported(self.refusal.clone())),
        }
    }

    /// A leaf projection: `$.derived(() => access)` (derived) or the RAW `access`
    /// (state) as `const <node> = <init>`, the leaf binding by its host id.
    fn push_path(&mut self, node: ExprId, access: ExprId) {
        let init = match self.mode {
            Lowering::Derived => {
                let derived = self.derived_of(access);
                self.b.push(derived)
            }
            Lowering::State => access,
        };
        let span = self.view().expr(node).span;
        self.paths.push(VariableDeclarator {
            id: node,
            init: Some(init),
            definite: false,
            span,
        });
    }

    /// `$.derived(() => access)`.
    fn derived_of(&mut self, access: ExprId) -> Expression {
        let arrow = self.b.thunk_on_body(access);
        let arrow = self.b.push(arrow);
        self.b.member_call("$", "derived", &[arrow])
    }

    /// `expr.<key>` — an object-pattern property projection. Only a plain,
    /// non-computed identifier key is built; a computed / literal / escaped key
    /// refuses (a safe over-refusal — the corpus has only identifier keys). The
    /// object `expr` is shared by id between sibling projections (the node
    /// contract: an id may print at more than one position).
    fn object_member(&mut self, expr: ExprId, prop: &Property) -> Result<ExprId, CompileError> {
        if prop.computed {
            return Err(unsupported(self.refusal.clone()));
        }
        let ExpressionKind::Identifier(key) = &self.view().expr(prop.key).kind else {
            return Err(unsupported(self.refusal.clone()));
        };
        let Some(name) = plain_identifier_name(key, self.source) else {
            return Err(unsupported(self.refusal.clone()));
        };
        let member = self.b.member_prop(expr, &name);
        Ok(self.b.push(member))
    }

    /// The `[<key literals>]` for `$.exclude_from_object` — the string names of
    /// every non-rest sibling property's KEY, in source order.
    fn rest_keys(
        &mut self,
        properties: &[ObjectPatternProperty],
    ) -> Result<Vec<Option<ExprId>>, CompileError> {
        let mut names: Vec<String> = Vec::new();
        for prop in properties {
            if let ObjectPatternProperty::Property(p) = prop {
                if p.computed {
                    return Err(unsupported(self.refusal.clone()));
                }
                let ExpressionKind::Identifier(key) = &self.view().expr(p.key).kind else {
                    return Err(unsupported(self.refusal.clone()));
                };
                let Some(name) = plain_identifier_name(key, self.source) else {
                    return Err(unsupported(self.refusal.clone()));
                };
                names.push(name);
            }
        }
        let mut elems = Vec::with_capacity(names.len());
        for name in names {
            let key = self.b.string_literal_expr(&name);
            elems.push(Some(self.b.push(key)));
        }
        Ok(elems)
    }

    /// `$.exclude_from_object(expr, [<keys>])`.
    fn exclude_from_object(&mut self, expr: ExprId, keys: &[Option<ExprId>]) -> ExprId {
        let array = self.b.array_of(keys);
        let array = self.b.push(array);
        let call = self
            .b
            .member_call("$", "exclude_from_object", &[expr, array]);
        self.b.push(call)
    }

    /// A small non-negative integer (an array index or length) as a numeric
    /// literal. The value is a destructuring-pattern position, always far below
    /// the f64 integer-exact range, so the cast is lossless in practice.
    #[expect(clippy::cast_precision_loss)]
    fn small_number(&mut self, n: usize) -> ExprId {
        let number = self.b.number(n as f64);
        self.b.push(number)
    }

    /// `$.to_array(expr[, len])`.
    fn build_to_array(&mut self, expr: ExprId, len: Option<usize>) -> Expression {
        let mut args = vec![expr];
        if let Some(len) = len {
            args.push(self.small_number(len));
        }
        self.b.member_call("$", "to_array", &args)
    }

    /// The array intermediate read: `$$derived_array()` (derived — read as a call,
    /// since the intermediate is itself a derived binding) or the BARE `$$array`
    /// (state — a plain `const`).
    fn array_read(&mut self, name: &str) -> ExprId {
        let ident = self.b.ident_expr(name);
        match self.mode {
            Lowering::Derived => {
                let call = self.b.call_of(ident, &[], false);
                self.b.push(call)
            }
            Lowering::State => ident,
        }
    }

    /// `$$derived_array()[i]` / `$$array[i]`.
    fn array_index(&mut self, name: &str, i: usize) -> ExprId {
        let base = self.array_read(name);
        let index = self.small_number(i);
        let member = self.b.member_computed(base, index);
        self.b.push(member)
    }

    /// `$$derived_array().slice(i)` / `$$array.slice(i)`.
    fn array_rest(&mut self, name: &str, i: usize) -> ExprId {
        let base = self.array_read(name);
        let slice = self.b.member_prop(base, "slice");
        let slice = self.b.push(slice);
        let index = self.small_number(i);
        let call = self.b.call_of(slice, &[index], false);
        self.b.push(call)
    }

    /// The port of the oracle's `build_fallback` (`utils/ast.js:585`) for the sync
    /// subset. A simple default → `$.fallback(expr, default)`; a non-simple one →
    /// `$.fallback(expr, () => default, true)` (with the `b.thunk` unthunk collapse
    /// — `f()` → `f`). An async default is unreachable: the pattern guard walk
    /// refuses the top-level `await` first.
    fn build_fallback(&mut self, expr: ExprId, default: ExprId) -> ExprId {
        let mut args = vec![expr];
        if is_simple_expression(self.view(), default) {
            args.push(default);
        } else {
            let thunk = match unthunk_callee(self.view(), self.view().expr(default)) {
                Some(callee) => callee,
                None => {
                    let thunk = self.b.thunk_on_body(default);
                    self.b.push(thunk)
                }
            };
            args.push(thunk);
            let true_literal = self.b.true_literal();
            args.push(self.b.push(true_literal));
        }
        let call = self.b.member_call("$", "fallback", &args);
        self.b.push(call)
    }
}

/// The oracle's `is_simple_expression` (`utils/ast.js:442`): an identifier /
/// literal / function / arrow, or a conditional/binary/logical whose operands are
/// all simple. tsv folds logical into [`ExpressionKind::BinaryExpression`].
fn is_simple_expression(view: AstView<'_>, node: ExprId) -> bool {
    match view.expr(node).kind {
        ExpressionKind::Literal(_)
        | ExpressionKind::Identifier(_)
        | ExpressionKind::ArrowFunctionExpression(_)
        | ExpressionKind::FunctionExpression(_) => true,
        ExpressionKind::ConditionalExpression(c) => {
            is_simple_expression(view, c.test)
                && is_simple_expression(view, c.consequent)
                && is_simple_expression(view, c.alternate)
        }
        ExpressionKind::BinaryExpression(b) => {
            // TODO: the oracle (`utils/ast.js:461`) also guards `left.type !==
            // 'PrivateIdentifier'` on this arm; omitted as unreachable — a
            // `#x in obj` default is parse-rejected at module scope.
            is_simple_expression(view, b.left) && is_simple_expression(view, b.right)
        }
        _ => false,
    }
}
