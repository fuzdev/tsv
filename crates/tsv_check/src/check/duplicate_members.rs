//! The duplicate-member check — a syntactic port of tsgo's
//! `checkObjectTypeForDuplicateDeclarations`.
//!
//! For one class body / interface-declaration body / type literal, this runs the
//! two-map (instance / static) state machine tsgo runs, and on the transition into
//! the `Reported` state re-scans the whole body (the `reportDuplicateMemberErrors`
//! batch), emitting one **TS2300** per declaration whose (key, is_static) matches
//! the offending bucket. It is deliberately **purely syntactic** — it never
//! consults the binder's symbol tables (walking the shared interface member table
//! would break declaration-merging), so it works off the AST members directly.
//!
//! Which members participate:
//! - **classified** (feed the state machine): a class field / property signature
//!   (kind [`MemberKind::Property`]), a get/set accessor or an auto-`accessor`
//!   field (kind [`MemberKind::Accessor`]), and a constructor **parameter
//!   property** (kind `Property`, always instance).
//! - **methods, call/construct/index signatures, static blocks** are *not*
//!   classified — they never drive a transition. A method still carries a name, so
//!   it participates in the batch (its symbol name can match a bucket), matching
//!   tsgo's `reportDuplicateMemberErrors` (which emits for any same-named member).
//!
//! Disjointness with the bind cascade is by construction: the binder reports on a
//! same-table *flag* conflict (any pair touching a method, or a same-kind
//! accessor/accessor), so those pairs never reach a classified→`Reported`
//! transition here; this pass fires only on property/property and
//! property/accessor pairs, which silent-merge in the binder. Where both do emit
//! (e.g. `x; get x; m()`), the identical (span, code, args) diagnostics collapse in
//! the program-wide sort/dedup — exactly as tsgo's binder + checker outputs do.
//
// tsgo: internal/checker/checker.go checkObjectTypeForDuplicateDeclarations (:3128)
//       + reportDuplicateMemberErrors

use crate::diag::{Category, Diagnostic};
use crate::hash::FxHashMap;
use crate::ids::FileId;
use string_interner::DefaultStringInterner;
use tsv_lang::Span;
use tsv_ts::ast::internal::{
    ClassMember, Expression, Literal, LiteralValue, MethodKind, TSTypeElement,
};

/// The per-body derivation context (source + interner for names, file for spans).
pub(super) struct MemberCtx<'a> {
    pub source: &'a str,
    pub interner: &'a DefaultStringInterner,
    pub file: FileId,
}

/// The two member classes tsgo's check distinguishes: `1` (property/property
/// signature) and `2` (accessor). Methods and signatures are neither.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Property,
    Accessor,
}

/// A body member reduced to what the check needs: its canonical key string, the
/// span its diagnostic points at (the name node), its static-ness, whether it
/// feeds the state machine (`classify`), and whether it is a constructor parameter
/// property (batched irrespective of the bucket's static-ness, per tsgo).
struct Entry {
    key: String,
    span: Span,
    is_static: bool,
    classify: Option<MemberKind>,
    ctor_param: bool,
}

/// The state machine state for one (key, is_static) bucket — tsgo's `0/1/2/3`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Unseen,
    SeenProperty,
    SeenAccessor,
    Reported,
}

/// Check a class body for duplicate property/accessor declarations, appending
/// TS2300 diagnostics to `out`.
pub(super) fn check_class_members(
    ctx: &MemberCtx<'_>,
    members: &[ClassMember<'_>],
    out: &mut Vec<Diagnostic>,
) {
    let entries = class_entries(ctx, members);
    run(ctx, &entries, out);
}

/// Check an interface / type-literal body (a `TSTypeElement` list) for duplicate
/// property/accessor signatures, appending TS2300 diagnostics to `out`.
pub(super) fn check_type_elements(
    ctx: &MemberCtx<'_>,
    members: &[TSTypeElement<'_>],
    out: &mut Vec<Diagnostic>,
) {
    let entries = type_element_entries(ctx, members);
    run(ctx, &entries, out);
}

/// Build the entry list for a class body (constructor param-properties expanded in
/// place; methods kept for the batch but unclassified).
fn class_entries(ctx: &MemberCtx<'_>, members: &[ClassMember<'_>]) -> Vec<Entry> {
    let mut entries = Vec::new();
    for member in members {
        match member {
            ClassMember::MethodDefinition(m) => match m.kind {
                MethodKind::Constructor => {
                    for param in m.value.params {
                        if let Expression::TSParameterProperty(pp) = param
                            && let Some((key, span)) = param_property_key(ctx, pp.parameter)
                        {
                            entries.push(Entry {
                                key,
                                span,
                                is_static: false,
                                classify: Some(MemberKind::Property),
                                ctor_param: true,
                            });
                        }
                    }
                }
                MethodKind::Method => {
                    if let Some((key, span)) = member_key(ctx, &m.key, m.computed) {
                        entries.push(Entry {
                            key,
                            span,
                            is_static: m.is_static,
                            classify: None,
                            ctor_param: false,
                        });
                    }
                }
                MethodKind::Get | MethodKind::Set => {
                    if let Some((key, span)) = member_key(ctx, &m.key, m.computed) {
                        entries.push(Entry {
                            key,
                            span,
                            is_static: m.is_static,
                            classify: Some(MemberKind::Accessor),
                            ctor_param: false,
                        });
                    }
                }
            },
            ClassMember::PropertyDefinition(p) => {
                if let Some((key, span)) = member_key(ctx, &p.key, p.computed) {
                    let classify = if p.accessor {
                        MemberKind::Accessor
                    } else {
                        MemberKind::Property
                    };
                    entries.push(Entry {
                        key,
                        span,
                        is_static: p.is_static,
                        classify: Some(classify),
                        ctor_param: false,
                    });
                }
            }
            ClassMember::StaticBlock(_) | ClassMember::IndexSignature(_) => {}
        }
    }
    entries
}

/// Build the entry list for an interface / type-literal body. Every member is
/// instance (no static); call/construct/index signatures carry no name.
fn type_element_entries(ctx: &MemberCtx<'_>, members: &[TSTypeElement<'_>]) -> Vec<Entry> {
    let mut entries = Vec::new();
    for member in members {
        match member {
            TSTypeElement::PropertySignature(p) => {
                if let Some((key, span)) = member_key(ctx, &p.key, p.computed) {
                    entries.push(Entry {
                        key,
                        span,
                        is_static: false,
                        classify: Some(MemberKind::Property),
                        ctor_param: false,
                    });
                }
            }
            TSTypeElement::MethodSignature(m) => {
                if let Some((key, span)) = member_key(ctx, &m.key, m.computed) {
                    let classify = match m.kind {
                        MethodKind::Get | MethodKind::Set => Some(MemberKind::Accessor),
                        // A plain method signature is unclassified but still batched.
                        _ => None,
                    };
                    entries.push(Entry {
                        key,
                        span,
                        is_static: false,
                        classify,
                        ctor_param: false,
                    });
                }
            }
            TSTypeElement::CallSignature(_)
            | TSTypeElement::ConstructSignature(_)
            | TSTypeElement::IndexSignature(_) => {}
        }
    }
    entries
}

/// Run the state machine over `entries` (source order), firing the batch on each
/// transition into `Reported`.
fn run(ctx: &MemberCtx<'_>, entries: &[Entry], out: &mut Vec<Diagnostic>) {
    let mut states: FxHashMap<(&str, bool), State> = FxHashMap::default();
    for entry in entries {
        let Some(kind) = entry.classify else { continue };
        let bucket = (entry.key.as_str(), entry.is_static);
        // Scope the mutable borrow so `fire_batch` (which reads `entries`, not
        // `states`) can run after the transition is decided.
        let transition = {
            let state = states.entry(bucket).or_insert(State::Unseen);
            match (*state, kind) {
                (State::Unseen, MemberKind::Property) => {
                    *state = State::SeenProperty;
                    false
                }
                (State::Unseen, MemberKind::Accessor) => {
                    *state = State::SeenAccessor;
                    false
                }
                // A second property, or a property after an accessor — always an error.
                (State::SeenProperty, _) | (State::SeenAccessor, MemberKind::Property) => {
                    *state = State::Reported;
                    true
                }
                // An accessor after an accessor is a legal get/set pair (the coarse
                // kind can't tell get from set) — leave it to the binder's cascade.
                (State::SeenAccessor, MemberKind::Accessor) => false,
                (State::Reported, _) => false,
            }
        };
        if transition {
            fire_batch(ctx, entries, &entry.key, entry.is_static, out);
        }
    }
}

/// tsgo `reportDuplicateMemberErrors`: emit one TS2300 per declaration whose
/// (key, is_static) matches the offending bucket. A constructor parameter property
/// matches on key alone (tsgo's constructor branch ignores `checkStatic`).
fn fire_batch(
    ctx: &MemberCtx<'_>,
    entries: &[Entry],
    key: &str,
    is_static: bool,
    out: &mut Vec<Diagnostic>,
) {
    for entry in entries {
        let matches = if entry.ctor_param {
            entry.key == key
        } else {
            entry.key == key && entry.is_static == is_static
        };
        if matches {
            out.push(make_2300(ctx.file, entry.span, &entry.key));
        }
    }
}

/// Build one `Duplicate identifier '{0}'.` diagnostic.
fn make_2300(file: FileId, span: Span, display: &str) -> Diagnostic {
    Diagnostic {
        file: Some(file),
        span,
        code: 2300,
        category: Category::Error,
        message: format!("Duplicate identifier '{display}'."),
        args: vec![display.to_string()],
        chain: Vec::new(),
        related: Vec::new(),
    }
}

/// Derive a member's canonical key string and the span its diagnostic points at
/// (the `member.Name()` node). Returns `None` for a member with no stable key — a
/// dynamic (non-literal) computed name, or a non-name key.
fn member_key(ctx: &MemberCtx<'_>, key: &Expression<'_>, computed: bool) -> Option<(String, Span)> {
    if computed {
        // A computed name is a stable key only for a string/number literal; the
        // diagnostic points at the whole `[ … ]` name node, so the span starts at
        // the `[`.
        return match key {
            Expression::Literal(lit)
                if matches!(lit.value, LiteralValue::String(_) | LiteralValue::Number(_)) =>
            {
                let k = literal_key(ctx, lit)?;
                let bracket = bracket_start(ctx.source, lit.span.start);
                Some((k, Span::new(bracket, lit.span.end)))
            }
            // A dynamic computed name is late-bound (bucket G, deferred) — skip.
            _ => None,
        };
    }
    match key {
        Expression::Identifier(id) => Some((
            id.name(ctx.source, ctx.interner).to_string(),
            id.name_span(),
        )),
        Expression::Literal(lit) => literal_key(ctx, lit).map(|k| (k, lit.span)),
        Expression::PrivateIdentifier(pid) => {
            // A `#name` — key it with the `#` so it never collides with the public
            // `name`; the diagnostic covers the whole `#name` node.
            let name = pid.name(ctx.source, ctx.interner);
            Some((format!("#{name}"), pid.span))
        }
        _ => None,
    }
}

/// The key of a constructor parameter property: the parameter identifier's name
/// (unwrapping a default `= …`). `None` when the name is a binding pattern (tsgo's
/// `!ast.IsBindingPattern(param.Name())` guard) — those contribute no member.
fn param_property_key(ctx: &MemberCtx<'_>, parameter: &Expression<'_>) -> Option<(String, Span)> {
    let inner = match parameter {
        Expression::AssignmentPattern(a) => a.left,
        other => other,
    };
    match inner {
        Expression::Identifier(id) => Some((
            id.name(ctx.source, ctx.interner).to_string(),
            id.name_span(),
        )),
        _ => None,
    }
}

/// The canonical key string of a literal property name: a string's decoded value,
/// a number's ECMA-262 `Number::toString` form (so `0`, `0.0`, `0x0` all key `0`
/// and collide with the string `'0'`), a bigint's verbatim source (conservative —
/// never over-collides).
fn literal_key(ctx: &MemberCtx<'_>, lit: &Literal<'_>) -> Option<String> {
    match &lit.value {
        LiteralValue::String(cooked) => Some(cooked.resolve(lit.span, ctx.source).to_string()),
        LiteralValue::Number(n) => Some(ecma_number_to_string(*n)),
        LiteralValue::BigInt => Some(lit.span.extract(ctx.source).to_string()),
        LiteralValue::Boolean(_) | LiteralValue::Null => None,
    }
}

/// The byte offset of the `[` opening a computed key, scanning back from the key
/// expression's start (a plain byte loop — `[` is ASCII). Falls back to the
/// expression start if no `[` precedes it (never for a well-formed computed name).
fn bracket_start(source: &str, expr_start: u32) -> u32 {
    let bytes = source.as_bytes();
    let mut i = expr_start as usize;
    while i > 0 {
        i -= 1;
        if bytes[i] == b'[' {
            return i as u32;
        }
    }
    expr_start
}

/// ECMA-262 `Number::toString` for a finite property-name value — the string tsgo
/// keys a numeric member on (`jsnum.FromString(text).String()` via the scanner's
/// `tokenValue`). Faithful to the spec's digit/exponent rules: `100` → `"100"`,
/// `0.5` → `"0.5"`, `1e21` → `"1e+21"`, `1e-7` → `"1e-7"`. The shortest
/// round-tripping significand comes from Rust's `{:e}` (Grisu), matching the spec's
/// "s as small as possible".
///
/// Reusable free fn (the number→string helper the check family needs).
// tsgo: internal/scanner/scanner.go scanNumber (tokenValue = jsnum.FromString(...).String())
pub(crate) fn ecma_number_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value == 0.0 {
        // Covers +0 and -0 (both are `"0"`).
        return "0".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    let negative = value < 0.0;
    let abs = value.abs();
    // Rust's lower-exp form yields the shortest significand plus a base-10
    // exponent, e.g. `2.55e2`. For any finite non-zero `f64` it is always
    // `<significand>e<int>`; the `else` fallbacks below never fire, but avoid a
    // panic on the impossible.
    let formatted = format!("{abs:e}");
    let Some((mantissa, exp_str)) = formatted.split_once('e') else {
        return formatted;
    };
    let Ok(exp) = exp_str.parse::<i32>() else {
        return formatted;
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = i32::try_from(digits.len()).unwrap_or(i32::MAX); // significant-digit count
    let n = exp + 1; // ECMA-262 `n`: the value is digits × 10^(n-k)
    // `usize` views of the split points (each branch establishes `n > 0` first).
    let n_split = usize::try_from(n).unwrap_or(0);

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if k <= n && n <= 21 {
        // Integer with trailing zeros.
        out.push_str(&digits);
        for _ in 0..(n - k) {
            out.push('0');
        }
    } else if 0 < n && n <= 21 {
        // Decimal point inside the digits.
        out.push_str(&digits[..n_split]);
        out.push('.');
        out.push_str(&digits[n_split..]);
    } else if -6 < n && n <= 0 {
        // Leading `0.` and `-n` zeros.
        out.push_str("0.");
        for _ in 0..(-n) {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        // Exponential form.
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        if n > 0 {
            out.push('+');
        } else {
            out.push('-');
        }
        out.push_str(&(n - 1).abs().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::ecma_number_to_string;

    #[test]
    fn ecma_number_to_string_integers_and_decimals() {
        assert_eq!(ecma_number_to_string(0.0), "0");
        assert_eq!(ecma_number_to_string(-0.0), "0");
        // `0` and `0.0` and `0x0` all parse to the same f64 → same key (they collide).
        assert_eq!(ecma_number_to_string(1.0), "1");
        assert_eq!(ecma_number_to_string(100.0), "100");
        assert_eq!(ecma_number_to_string(255.0), "255"); // 0xff
        assert_eq!(ecma_number_to_string(0.5), "0.5");
        assert_eq!(ecma_number_to_string(2.5), "2.5");
        assert_eq!(ecma_number_to_string(1.25), "1.25");
    }

    #[test]
    fn ecma_number_to_string_exponent_thresholds() {
        assert_eq!(ecma_number_to_string(1e21), "1e+21");
        assert_eq!(ecma_number_to_string(1e-7), "1e-7");
        assert_eq!(ecma_number_to_string(1e-6), "0.000001");
        assert_eq!(ecma_number_to_string(1e20), "100000000000000000000");
        assert_eq!(ecma_number_to_string(1.5e22), "1.5e+22");
    }
}
