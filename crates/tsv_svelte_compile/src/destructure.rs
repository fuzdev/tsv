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

use bumpalo::collections::Vec as BumpVec;
use tsv_ts::ast::internal::{Expression, ObjectPatternProperty, Property, VariableDeclarator};

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
pub(crate) fn expand_destructured_derived<'arena>(
    b: &mut Builder<'arena>,
    ctx: &mut WalkCtx<'_>,
    declarations: &mut BumpVec<'arena, VariableDeclarator<'arena>>,
    declarator: &'arena VariableDeclarator<'arena>,
    rune: RuneInit<'arena>,
    source: &str,
    names: &mut GeneratedNames<'_>,
) -> Result<(), CompileError> {
    // The pattern's defaults are real value expressions — walk them for a stray
    // rune (`{a = $state(0)}`) or a top-level `await`. The binding leaves are
    // exempt (derived reads allowed; `$`-prefixed leaves already refused by
    // `refuse_dollar_binding_pattern` before this arm).
    walk_expression_guarded(&declarator.id, ctx)?;

    let (value, is_derived, refusal) = match rune {
        RuneInit::Derived(expr) => (expr, true, Refusal::DestructuringDerived),
        RuneInit::DerivedBy(f) => (f, false, Refusal::DestructuringDerivedBy),
        // Only the two derived arms reach here (the caller matched them).
        _ => return Err(unsupported(Refusal::DestructuringDerived)),
    };
    // Guard the rune argument (a bare derived read is exempt — the store rewrite
    // lowers it to `base()` inside each projection).
    walk_expression_guarded(value, ctx)?;

    // rhs: a bare-identifier `$derived(o)` projects directly from the borrowed
    // `o`; every other shape mints `$$d` and projects from `$$d()`.
    let mint_d = !(is_derived && matches!(value, Expression::Identifier(_)));
    let rhs: &'arena Expression<'arena> = if mint_d {
        let d_name = names.generate("$$d");
        // Mint the id BEFORE the init so the declarator span runs forward
        // (`id.start < init.end`) — the invariant `build_sanitize_slots_decl`
        // relies on, else the printer's `callee.end - decl.start` underflows.
        let d_id = Expression::Identifier(b.ident(&d_name));
        let span = d_id.span();
        // `$.derived(<init>)`: `$derived` thunks the value (reusing the identifier
        // path's unthunk collapse); `$derived.by` passes the compute fn directly.
        let argument: &'arena Expression<'arena> = if is_derived {
            match unthunk_callee(value) {
                Some(callee) => callee,
                None => {
                    let anchor = b.here();
                    &*b.arena.alloc(b.arrow_expr_at(anchor, value))
                }
            }
        } else {
            value
        };
        let init = derived_wrap(b, argument);
        declarations.push(VariableDeclarator {
            id: d_id,
            init: Some(init),
            definite: false,
            span,
        });
        let callee = b.ident_expr(&d_name);
        b.arena.alloc(b.call_of(callee, &[], false))
    } else {
        value
    };

    let mut extractor = Extractor {
        b,
        source,
        refusal,
        mode: Lowering::Derived,
        inserts: Vec::new(),
        paths: Vec::new(),
    };
    extractor.extract(&declarator.id, rhs, names)?;
    for decl in extractor.inserts {
        declarations.push(decl);
    }
    for decl in extractor.paths {
        declarations.push(decl);
    }
    Ok(())
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
pub(crate) fn expand_destructured_state<'arena>(
    b: &mut Builder<'arena>,
    ctx: &mut WalkCtx<'_>,
    declarations: &mut BumpVec<'arena, VariableDeclarator<'arena>>,
    declarator: &'arena VariableDeclarator<'arena>,
    rune: RuneInit<'arena>,
    source: &str,
    names: &mut GeneratedNames<'_>,
) -> Result<(), CompileError> {
    // The pattern's defaults are real value expressions — walk them for a stray
    // rune (`{a = $state(0)}`) or a top-level `await`. The binding leaves are
    // exempt (`$`-prefixed leaves already refused by `refuse_dollar_binding_pattern`
    // before this arm).
    walk_expression_guarded(&declarator.id, ctx)?;

    // `value` is the argument the `tmp` holds — `void 0` when argless. The rune's
    // argument is guard-walked (a bare store/derived read is exempt — the store
    // rewrite lowers it inside `let tmp = value` afterward). `$state.snapshot`'s
    // wrapper is dropped, so its argument becomes `value` just like `$state`.
    let (value, refusal): (Expression<'arena>, Refusal) = match rune {
        RuneInit::State(Some(arg)) => {
            walk_expression_guarded(arg, ctx)?;
            (arg.clone(), Refusal::DestructuringState)
        }
        RuneInit::State(None) => (b.void_zero(), Refusal::DestructuringState),
        RuneInit::StateSnapshot(arg) => {
            walk_expression_guarded(arg, ctx)?;
            (arg.clone(), Refusal::DestructuringStateSnapshot)
        }
        // Only the three state arms reach here (the caller matched them).
        _ => return Err(unsupported(Refusal::DestructuringState)),
    };

    // `let tmp = value` leads the group. `tmp` is generated BEFORE `$$array` (the
    // oracle's order: `scope.generate('tmp')` runs before `extract_paths`, whose
    // `$$array` names the Extractor mints inline during the walk).
    let tmp_name = names.generate("tmp");
    // `tmp`'s init is the borrowed host `value` (not a mint), so — unlike the
    // minted `$$d` / `$$array` inits — this declarator does NOT run forward: the
    // id is a fresh appendix span (high) while `value` keeps its lower host span,
    // so `id.start > init.end`. Safe anyway, because the printer's underflow-prone
    // `callee.end - decl.span.start` (`variable.rs`) reads the reused host
    // STATEMENT span (`rewrite_script_statement`'s `span: decl.span`), never this
    // appendix id span.
    let tmp_id = Expression::Identifier(b.ident(&tmp_name));
    let tmp_span = tmp_id.span();
    declarations.push(VariableDeclarator {
        id: tmp_id,
        init: Some(value),
        definite: false,
        span: tmp_span,
    });
    // Every leaf projects from a fresh `tmp` read (a plain `const`, so no `()`).
    let rhs: &'arena Expression<'arena> = b.ident_expr(&tmp_name);

    let mut extractor = Extractor {
        b,
        source,
        refusal,
        mode: Lowering::State,
        inserts: Vec::new(),
        paths: Vec::new(),
    };
    extractor.extract(&declarator.id, rhs, names)?;
    for decl in extractor.inserts {
        declarations.push(decl);
    }
    for decl in extractor.paths {
        declarations.push(decl);
    }
    Ok(())
}

/// `$.derived(<argument>)` — the intermediate/insert wrapper (comment-free, so a
/// plain appendix mint is safe; byte-identical to the span-stealing form).
fn derived_wrap<'arena>(
    b: &mut Builder<'arena>,
    argument: &'arena Expression<'arena>,
) -> Expression<'arena> {
    b.member_call("$", "derived", std::slice::from_ref(argument))
}

/// The port of the oracle's `_extract_paths`: it walks the binding pattern in
/// parallel with the access expression `rhs`, collecting `inserts` (array
/// `$.to_array` intermediates) and `paths` (leaf `const name = $.derived(() =>
/// access)` declarators), each already wrapped as its own declarator.
struct Extractor<'b, 'arena> {
    b: &'b mut Builder<'arena>,
    source: &'b str,
    /// The refusal for an unrecognized shape (per rune family).
    refusal: Refusal,
    /// Which rune family — the only fork (leaf/array-intermediate wrapping).
    mode: Lowering,
    inserts: Vec<VariableDeclarator<'arena>>,
    paths: Vec<VariableDeclarator<'arena>>,
}

impl<'arena> Extractor<'_, 'arena> {
    fn extract(
        &mut self,
        param: &Expression<'arena>,
        expr: &'arena Expression<'arena>,
        names: &mut GeneratedNames<'_>,
    ) -> Result<(), CompileError> {
        match param {
            // Leaf: `const <param> = $.derived(() => expr)`. A declaration pattern
            // never binds a member expression, so an Identifier is the only leaf.
            Expression::Identifier(_) => {
                self.push_path(param, expr);
                Ok(())
            }
            Expression::ObjectPattern(obj) => {
                for prop in obj.properties {
                    match prop {
                        ObjectPatternProperty::RestElement(rest) => {
                            // `$.exclude_from_object(expr, [<sibling key literals>])`.
                            let keys = self.rest_keys(obj.properties)?;
                            let rest_expr = self.exclude_from_object(expr, keys);
                            self.extract(rest.argument, rest_expr, names)?;
                        }
                        ObjectPatternProperty::Property(p) => {
                            let member = self.object_member(expr, p)?;
                            self.extract(&p.value, member, names)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::ArrayPattern(arr) => {
                // Derived: `const $$derived_array = $.derived(() => $.to_array(expr[,
                // len]))`. State: `const $$array = $.to_array(expr[, len])` (a plain
                // `const`, no `$.derived` wrap). The length is omitted when the last
                // element is a rest.
                let has_rest =
                    matches!(arr.elements.last(), Some(Some(Expression::RestElement(_))));
                let len = (!has_rest).then_some(arr.elements.len());
                let name = names.generate(self.mode.array_base());
                // Mint the id BEFORE the `$.to_array` / `$.derived` init so the
                // declarator span runs forward (see the `$$d` mint's forward-span
                // note in `expand_destructured_derived`).
                let id = Expression::Identifier(self.b.ident(&name));
                let id_span = id.span();
                let to_array = self.build_to_array(expr, len);
                let init = match self.mode {
                    Lowering::Derived => {
                        let value = &*self.b.arena.alloc(to_array);
                        self.derived_of(value)
                    }
                    Lowering::State => to_array,
                };
                self.inserts.push(VariableDeclarator {
                    id,
                    init: Some(init),
                    definite: false,
                    span: id_span,
                });

                for (i, element) in arr.elements.iter().enumerate() {
                    let Some(element) = element else {
                        continue; // a hole (`[a, , b]`) projects nothing
                    };
                    if let Expression::RestElement(rest) = element {
                        let slice = self.array_rest(&name, i);
                        self.extract(rest.argument, slice, names)?;
                    } else {
                        let index = self.array_index(&name, i);
                        self.extract(element, index, names)?;
                    }
                }
                Ok(())
            }
            Expression::AssignmentPattern(assign) => {
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
    /// (state) as `const <node-clone> = <init>`.
    fn push_path(&mut self, node: &Expression<'arena>, access: &'arena Expression<'arena>) {
        let init = match self.mode {
            Lowering::Derived => self.derived_of(access),
            Lowering::State => access.clone(),
        };
        let span = node.span();
        self.paths.push(VariableDeclarator {
            id: node.clone(),
            init: Some(init),
            definite: false,
            span,
        });
    }

    /// `$.derived(() => access)`.
    fn derived_of(&mut self, access: &'arena Expression<'arena>) -> Expression<'arena> {
        let anchor = self.b.here();
        let arrow = &*self.b.arena.alloc(self.b.arrow_expr_at(anchor, access));
        self.b
            .member_call("$", "derived", std::slice::from_ref(arrow))
    }

    /// `expr.<key>` — an object-pattern property projection. Only a plain,
    /// non-computed identifier key is built; a computed / literal / escaped key
    /// refuses (a safe over-refusal — the corpus has only identifier keys).
    fn object_member(
        &mut self,
        expr: &'arena Expression<'arena>,
        prop: &Property<'arena>,
    ) -> Result<&'arena Expression<'arena>, CompileError> {
        if prop.computed {
            return Err(unsupported(self.refusal.clone()));
        }
        let Expression::Identifier(key) = &prop.key else {
            return Err(unsupported(self.refusal.clone()));
        };
        let Some(name) = plain_identifier_name(key, self.source) else {
            return Err(unsupported(self.refusal.clone()));
        };
        // Clone the (reused) object so sibling properties don't alias one node.
        let object = &*self.b.arena.alloc(expr.clone());
        Ok(&*self.b.arena.alloc(self.b.member_prop(object, &name)))
    }

    /// The `[<key literals>]` for `$.exclude_from_object` — the string names of
    /// every non-rest sibling property's KEY, in source order.
    fn rest_keys(
        &mut self,
        properties: &'arena [ObjectPatternProperty<'arena>],
    ) -> Result<&'arena [Option<Expression<'arena>>], CompileError> {
        let mut names: Vec<String> = Vec::new();
        for prop in properties {
            if let ObjectPatternProperty::Property(p) = prop {
                if p.computed {
                    return Err(unsupported(self.refusal.clone()));
                }
                let Expression::Identifier(key) = &p.key else {
                    return Err(unsupported(self.refusal.clone()));
                };
                let Some(name) = plain_identifier_name(key, self.source) else {
                    return Err(unsupported(self.refusal.clone()));
                };
                names.push(name);
            }
        }
        let mut elems: BumpVec<'arena, Option<Expression<'arena>>> =
            BumpVec::with_capacity_in(names.len(), self.b.arena);
        for name in names {
            elems.push(Some(self.b.string_literal_expr(&name)));
        }
        Ok(elems.into_bump_slice())
    }

    /// `$.exclude_from_object(expr, [<keys>])`.
    fn exclude_from_object(
        &mut self,
        expr: &'arena Expression<'arena>,
        keys: &'arena [Option<Expression<'arena>>],
    ) -> &'arena Expression<'arena> {
        let array = self.b.array_of(keys);
        let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(self.b.arena);
        args.push(expr.clone());
        args.push(array);
        let call = self
            .b
            .member_call("$", "exclude_from_object", args.into_bump_slice());
        &*self.b.arena.alloc(call)
    }

    /// A small non-negative integer (an array index or length) as a numeric
    /// literal. The value is a destructuring-pattern position, always far below
    /// the f64 integer-exact range, so the cast is lossless in practice.
    #[allow(clippy::cast_precision_loss)]
    fn small_number(&mut self, n: usize) -> Expression<'arena> {
        self.b.number(n as f64)
    }

    /// `$.to_array(expr[, len])`.
    fn build_to_array(
        &mut self,
        expr: &'arena Expression<'arena>,
        len: Option<usize>,
    ) -> Expression<'arena> {
        let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(self.b.arena);
        args.push(expr.clone());
        if let Some(len) = len {
            args.push(self.small_number(len));
        }
        self.b.member_call("$", "to_array", args.into_bump_slice())
    }

    /// The array intermediate read: `$$derived_array()` (derived — read as a call,
    /// since the intermediate is itself a derived binding) or the BARE `$$array`
    /// (state — a plain `const`).
    fn array_read(&mut self, name: &str) -> &'arena Expression<'arena> {
        let ident = self.b.ident_expr(name);
        match self.mode {
            Lowering::Derived => &*self.b.arena.alloc(self.b.call_of(ident, &[], false)),
            Lowering::State => ident,
        }
    }

    /// `$$derived_array()[i]` / `$$array[i]`.
    fn array_index(&mut self, name: &str, i: usize) -> &'arena Expression<'arena> {
        let base = self.array_read(name);
        let index = &*self.b.arena.alloc(self.small_number(i));
        &*self.b.arena.alloc(self.b.member_computed(base, index))
    }

    /// `$$derived_array().slice(i)` / `$$array.slice(i)`.
    fn array_rest(&mut self, name: &str, i: usize) -> &'arena Expression<'arena> {
        let base = self.array_read(name);
        let slice = &*self.b.arena.alloc(self.b.member_prop(base, "slice"));
        let index = &*self.b.arena.alloc(self.small_number(i));
        &*self
            .b
            .arena
            .alloc(self.b.call_of(slice, std::slice::from_ref(index), false))
    }

    /// The port of the oracle's `build_fallback` (`utils/ast.js:585`) for the sync
    /// subset. A simple default → `$.fallback(expr, default)`; a non-simple one →
    /// `$.fallback(expr, () => default, true)` (with the `b.thunk` unthunk collapse
    /// — `f()` → `f`). An async default is unreachable: the pattern guard walk
    /// refuses the top-level `await` first.
    fn build_fallback(
        &mut self,
        expr: &'arena Expression<'arena>,
        default: &'arena Expression<'arena>,
    ) -> &'arena Expression<'arena> {
        let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(self.b.arena);
        args.push(expr.clone());
        if is_simple_expression(default) {
            args.push(default.clone());
        } else {
            let thunk = match unthunk_callee(default) {
                Some(callee) => callee,
                None => {
                    let anchor = self.b.here();
                    &*self.b.arena.alloc(self.b.arrow_expr_at(anchor, default))
                }
            };
            args.push(thunk.clone());
            args.push(self.b.true_literal());
        }
        let call = self.b.member_call("$", "fallback", args.into_bump_slice());
        &*self.b.arena.alloc(call)
    }
}

/// The oracle's `is_simple_expression` (`utils/ast.js:442`): an identifier /
/// literal / function / arrow, or a conditional/binary/logical whose operands are
/// all simple. tsv folds logical into [`Expression::BinaryExpression`].
fn is_simple_expression(node: &Expression<'_>) -> bool {
    match node {
        Expression::Literal(_)
        | Expression::Identifier(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_) => true,
        Expression::ConditionalExpression(c) => {
            is_simple_expression(c.test)
                && is_simple_expression(c.consequent)
                && is_simple_expression(c.alternate)
        }
        Expression::BinaryExpression(b) => {
            // TODO: the oracle (`utils/ast.js:461`) also guards `left.type !==
            // 'PrivateIdentifier'` on this arm; omitted as unreachable — a
            // `#x in obj` default is parse-rejected at module scope.
            is_simple_expression(b.left) && is_simple_expression(b.right)
        }
        _ => false,
    }
}
