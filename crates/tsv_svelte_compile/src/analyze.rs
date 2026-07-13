//! Script binding analysis and the static-evaluation port.
//!
//! The oracle folds statically-known template expressions into the emitted
//! template text (its `Evaluation` abstract interpreter in the compiler's scope
//! phase). Parity therefore needs the same fold decision here. This module
//! ports that evaluator **faithfully for a bounded domain** and refuses
//! (`Gray`) anywhere the oracle could produce a result this port cannot
//! reproduce byte-exactly:
//!
//! - The value domain is strings / `f64` numbers / booleans / `null` /
//!   `undefined`, plus the oracle's sentinels (STRING / NUMBER / FUNCTION /
//!   UNKNOWN). A node type the oracle defaults to UNKNOWN is UNKNOWN here too
//!   (that is portable); a node the oracle *computes* through machinery this
//!   port doesn't carry — its `globals` tables, `RegExp`/`BigInt` values,
//!   string→number coercion, non-ASCII string comparison — is `Gray`.
//! - Bindings mirror the oracle's rules: a prop, an updated binding, or a
//!   binding without an initial value is UNKNOWN; otherwise its initial
//!   evaluates in place (rune inits evaluate through to their argument). A
//!   top-level name shadowed anywhere in nested scopes is `Gray` — the
//!   mutation walk is shadow-naive, and a wrongly-`updated` binding would flip
//!   a fold into a silent mismatch.
//!
//! Fold *stringification* (`(value ?? '') + ''`) is restricted to exactly the
//! values this port can print byte-identically to JS: strings, booleans,
//! null/undefined (→ empty), and integer-valued numbers in the safe range —
//! anything else is `Gray`.

use std::collections::{HashMap, HashSet};

use tsv_ts::ast::internal::{ArrowFunctionBody, BinaryOperator, Expression, UnaryOperator};

use crate::{CompileError, Refusal};

/// How a top-level script binding behaves under evaluation and read rewriting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingKind {
    /// From the `$props()` destructure — always UNKNOWN to the evaluator.
    Prop,
    /// `$derived(...)` / `$derived.by(...)` — reads become calls (`d()`).
    Derived,
    /// Everything else (including `$state` after rewrite — a plain variable
    /// on the server).
    Normal,
    /// A binding this analysis can't model (destructured non-prop declarator,
    /// shadowed name) — evaluating through it refuses.
    Opaque,
}

/// A binding's evaluation-relevant initial value.
#[derive(Clone, Copy)]
pub(crate) enum Initial<'arena> {
    /// Evaluate through this expression (for rune inits, the rune's argument).
    Expr(&'arena Expression<'arena>),
    /// A function declaration (the FUNCTION sentinel).
    Function,
    /// An argument-less `$state()` (evaluates to `undefined`).
    Undefined,
    /// No initial / not modeled — UNKNOWN.
    None,
}

pub(crate) struct Binding<'arena> {
    pub kind: BindingKind,
    pub initial: Initial<'arena>,
    pub updated: bool,
}

/// The top-level script binding table.
pub(crate) struct Bindings<'arena> {
    map: HashMap<String, Binding<'arena>>,
}

impl<'arena> Bindings<'arena> {
    pub fn empty() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Binding<'arena>> {
        self.map.get(name)
    }

    /// Whether a top-level binding by this name exists (block-name collision
    /// guard).
    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// The top-level binding names (snippet-hoist instance-binding set).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    /// Whether this binding is a `$props()` destructure (render-callee dynamic
    /// classification: a prop callee is dynamic, so its render tag is not
    /// standalone).
    pub fn is_prop(&self, name: &str) -> bool {
        self.map
            .get(name)
            .is_some_and(|b| b.kind == BindingKind::Prop)
    }

    pub fn insert(&mut self, name: String, binding: Binding<'arena>) {
        self.map.insert(name, binding);
    }

    pub fn mark_updated(&mut self, name: &str) {
        if let Some(binding) = self.map.get_mut(name) {
            binding.updated = true;
        }
    }

    pub fn mark_opaque(&mut self, name: &str) {
        if let Some(binding) = self.map.get_mut(name) {
            binding.kind = BindingKind::Opaque;
        }
    }
}

/// A concrete value in the ported domain.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Undefined,
}

impl Value {
    fn truthy(&self) -> bool {
        match self {
            Value::Str(s) => !s.is_empty(),
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Bool(b) => *b,
            Value::Null | Value::Undefined => false,
        }
    }

    fn nullish(&self) -> bool {
        matches!(self, Value::Null | Value::Undefined)
    }

    /// Same-value-zero equality (JS `Set` semantics — what the oracle's value
    /// set deduplicates by): `NaN` equals `NaN`, `-0` equals `0`.
    // Exact bitwise-style comparison IS the JS semantics being ported — an
    // epsilon here would be wrong.
    #[allow(clippy::float_cmp)]
    fn same_value_zero(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => (a.is_nan() && b.is_nan()) || a == b,
            _ => self == other,
        }
    }
}

/// A member of the oracle's value set: a concrete value or one of its symbols.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Entry {
    Value(Value),
    StringSentinel,
    NumberSentinel,
    FunctionSentinel,
    Unknown,
}

/// The ported evaluation product: the (deduplicated) value set plus the derived
/// flags the emitters consult.
pub(crate) struct Evaluation {
    values: Vec<Entry>,
}

/// The evaluator refused: the oracle could compute something this port cannot
/// bound byte-exactly. Carries the reason for the `Unsupported` message.
pub(crate) struct Gray(pub String);

fn gray<T>(what: impl Into<String>) -> Result<T, Gray> {
    Err(Gray(what.into()))
}

type EvalResult = Result<Evaluation, Gray>;

impl Evaluation {
    fn single(entry: Entry) -> Self {
        Self {
            values: vec![entry],
        }
    }

    fn known(value: Value) -> Self {
        Self::single(Entry::Value(value))
    }

    fn add(&mut self, entry: Entry) {
        let duplicate = self.values.iter().any(|e| match (e, &entry) {
            (Entry::Value(a), Entry::Value(b)) => a.same_value_zero(b),
            (a, b) => a == b,
        });
        if !duplicate {
            self.values.push(entry);
        }
    }

    fn extend(&mut self, other: Evaluation) {
        for entry in other.values {
            self.add(entry);
        }
    }

    /// `is_known`: exactly one possible value and it is concrete.
    pub fn known_value(&self) -> Option<&Value> {
        match self.values.as_slice() {
            [Entry::Value(v)] => Some(v),
            _ => None,
        }
    }

    /// The oracle's `is_string && is_defined` (drives `$.stringify` omission in
    /// attribute templates): every possible value is a string (or the STRING
    /// sentinel) and none is nullish/UNKNOWN.
    pub fn is_defined_string(&self) -> bool {
        self.values
            .iter()
            .all(|e| matches!(e, Entry::Value(Value::Str(_)) | Entry::StringSentinel))
    }
}

/// Stringify a known value exactly as the fold does (`(value ?? '') + ''`).
/// `Gray` for values whose JS stringification this port doesn't reproduce
/// (non-integer numbers).
pub(crate) fn stringify_value(value: &Value) -> Result<String, Gray> {
    match value {
        Value::Str(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        // `value ?? ''` — nullish folds to the empty string.
        Value::Null | Value::Undefined => Ok(String::new()),
        Value::Num(n) => stringify_number(*n),
    }
}

/// JS `String(number)` for the safe subset: integer-valued f64 within the safe
/// range (including `-0` → `"0"`) and `NaN`/`Infinity`. Everything else `Gray`
/// (shortest-roundtrip + exponent formatting is not ported).
fn stringify_number(n: f64) -> Result<String, Gray> {
    if n.is_nan() {
        return Ok("NaN".to_string());
    }
    if n.is_infinite() {
        return Ok(if n > 0.0 { "Infinity" } else { "-Infinity" }.to_string());
    }
    const SAFE: f64 = 9_007_199_254_740_991.0; // 2^53 - 1
    if n.fract() == 0.0 && n.abs() <= SAFE {
        // -0 prints as "0".
        return Ok(format!("{}", n as i64));
    }
    gray(format!("non-integer numeric fold ({n})"))
}

/// Evaluate `expr` against the binding table — the ported `scope.evaluate`.
pub(crate) fn evaluate(
    expr: &Expression<'_>,
    scope: &Scope<'_, '_>,
    source: &str,
    depth: usize,
) -> EvalResult {
    if depth > 16 {
        return gray("evaluation recursion limit (cyclic bindings?)");
    }
    match expr {
        Expression::Literal(lit) => literal_value(lit, source),

        Expression::Identifier(id) => {
            if id.escaped_name.is_some() {
                // Synthetic identifiers never appear on an evaluated spine.
                return Ok(Evaluation::single(Entry::Unknown));
            }
            let start = id.span.start as usize;
            let name = &source[start..start + id.name_len as usize];
            match scope.resolve(name) {
                Resolved::Masked => Ok(Evaluation::single(Entry::Unknown)),
                Resolved::Binding(binding) => {
                    if binding.kind == BindingKind::Prop {
                        return Ok(Evaluation::single(Entry::Unknown));
                    }
                    if binding.kind == BindingKind::Opaque {
                        return gray(format!("binding {name} is not statically modeled"));
                    }
                    if binding.updated {
                        return Ok(Evaluation::single(Entry::Unknown));
                    }
                    match binding.initial {
                        Initial::Expr(init) => evaluate(init, scope, source, depth + 1),
                        Initial::Function => Ok(Evaluation::single(Entry::FunctionSentinel)),
                        Initial::Undefined => Ok(Evaluation::known(Value::Undefined)),
                        Initial::None => Ok(Evaluation::single(Entry::Unknown)),
                    }
                }
                Resolved::None if name == "undefined" => Ok(Evaluation::known(Value::Undefined)),
                Resolved::None => Ok(Evaluation::single(Entry::Unknown)),
            }
        }

        Expression::BinaryExpression(bin) => {
            use BinaryOperator::{
                Ampersand, AmpersandAmpersand, BangEquals, BangEqualsEquals, Caret, EqualsEquals,
                EqualsEqualsEquals, GreaterThan, GreaterThanEquals, In, Instanceof, LeftShift,
                LessThan, LessThanEquals, Minus, Percent, Pipe, PipePipe, Plus, QuestionQuestion,
                RightShift, Slash, Star, StarStar, UnsignedRightShift,
            };
            let a = evaluate(bin.left, scope, source, depth + 1)?;
            let b = evaluate(bin.right, scope, source, depth + 1)?;
            match bin.operator {
                // Logical operators (merged into BinaryExpression in this AST)
                // follow the oracle's LogicalExpression case.
                AmpersandAmpersand | PipePipe | QuestionQuestion => {
                    if let Some(av) = a.known_value() {
                        if let Some(bv) = b.known_value() {
                            let take_b = match bin.operator {
                                AmpersandAmpersand => av.truthy(),
                                PipePipe => !av.truthy(),
                                _ => av.nullish(),
                            };
                            return Ok(Evaluation::known(if take_b {
                                bv.clone()
                            } else {
                                av.clone()
                            }));
                        }
                        let short_circuit = match bin.operator {
                            AmpersandAmpersand => !av.truthy(),
                            PipePipe => av.truthy(),
                            _ => !av.nullish(),
                        };
                        if short_circuit {
                            return Ok(Evaluation::known(av.clone()));
                        }
                        return Ok(b);
                    }
                    let mut union = a;
                    union.extend(b);
                    Ok(union)
                }
                _ => {
                    if let (Some(av), Some(bv)) = (a.known_value(), b.known_value()) {
                        return Ok(Evaluation::known(binary_op(bin.operator, av, bv)?));
                    }
                    // Not known: the oracle's per-operator result-shape table —
                    // always a non-known set, portable wholesale.
                    let mut eval = Evaluation { values: Vec::new() };
                    match bin.operator {
                        BangEquals | BangEqualsEquals | LessThan | LessThanEquals | GreaterThan
                        | GreaterThanEquals | EqualsEquals | EqualsEqualsEquals | In
                        | Instanceof => {
                            eval.add(Entry::Value(Value::Bool(true)));
                            eval.add(Entry::Value(Value::Bool(false)));
                        }
                        Percent | Ampersand | Star | StarStar | Minus | Slash | LeftShift
                        | RightShift | UnsignedRightShift | Caret | Pipe => {
                            eval.add(Entry::NumberSentinel);
                        }
                        Plus => {
                            let a_string = a.is_defined_string();
                            let b_string = b.is_defined_string();
                            let a_number = all_numbers(&a);
                            let b_number = all_numbers(&b);
                            if a_string || b_string {
                                eval.add(Entry::StringSentinel);
                            } else if a_number && b_number {
                                eval.add(Entry::NumberSentinel);
                            } else {
                                eval.add(Entry::StringSentinel);
                                eval.add(Entry::NumberSentinel);
                            }
                        }
                        _ => eval.add(Entry::Unknown),
                    }
                    Ok(eval)
                }
            }
        }

        Expression::ConditionalExpression(cond) => {
            let test = evaluate(cond.test, scope, source, depth + 1)?;
            let consequent = evaluate(cond.consequent, scope, source, depth + 1)?;
            let alternate = evaluate(cond.alternate, scope, source, depth + 1)?;
            if let Some(tv) = test.known_value() {
                return Ok(if tv.truthy() { consequent } else { alternate });
            }
            let mut union = consequent;
            union.extend(alternate);
            Ok(union)
        }

        Expression::UnaryExpression(unary) => {
            let argument = evaluate(unary.argument, scope, source, depth + 1)?;
            if let Some(v) = argument.known_value() {
                return Ok(Evaluation::known(unary_op(unary.operator, v)?));
            }
            let mut eval = Evaluation { values: Vec::new() };
            match unary.operator {
                UnaryOperator::Bang | UnaryOperator::Delete => {
                    eval.add(Entry::Value(Value::Bool(false)));
                    eval.add(Entry::Value(Value::Bool(true)));
                }
                UnaryOperator::Plus | UnaryOperator::Minus | UnaryOperator::Tilde => {
                    eval.add(Entry::NumberSentinel);
                }
                UnaryOperator::Typeof => {
                    eval.add(Entry::StringSentinel);
                }
                UnaryOperator::Void => {
                    eval.add(Entry::Value(Value::Undefined));
                }
            }
            Ok(eval)
        }

        Expression::CallExpression(call) => {
            match global_keypath(call.callee, scope, source) {
                Some(keypath) if keypath.starts_with('$') => {
                    // The rune table.
                    let arg = call.arguments.first();
                    match keypath.as_str() {
                        "$state" | "$state.raw" | "$derived" => match arg {
                            Some(arg) => evaluate(arg, scope, source, depth + 1),
                            None => Ok(Evaluation::known(Value::Undefined)),
                        },
                        "$props.id" => Ok(Evaluation::single(Entry::StringSentinel)),
                        "$effect.tracking" => {
                            let mut eval = Evaluation { values: Vec::new() };
                            eval.add(Entry::Value(Value::Bool(false)));
                            eval.add(Entry::Value(Value::Bool(true)));
                            Ok(eval)
                        }
                        "$derived.by" => match arg {
                            Some(Expression::ArrowFunctionExpression(arrow)) => match &arrow.body {
                                ArrowFunctionBody::Expression(body) => {
                                    evaluate(body, scope, source, depth + 1)
                                }
                                ArrowFunctionBody::BlockStatement(_) => {
                                    Ok(Evaluation::single(Entry::Unknown))
                                }
                            },
                            _ => Ok(Evaluation::single(Entry::Unknown)),
                        },
                        _ => Ok(Evaluation::single(Entry::Unknown)),
                    }
                }
                Some(keypath) => {
                    // A global function call — the oracle computes through its
                    // `globals` table (Math.max, …), which is not ported.
                    gray(format!("global call {keypath} (globals table not ported)"))
                }
                None => Ok(Evaluation::single(Entry::Unknown)),
            }
        }

        Expression::TemplateLiteral(template) => {
            let mut result = String::new();
            match quasi_cooked(template, 0, source) {
                Some(text) => result.push_str(&text),
                None => return gray("template quasi with invalid escape"),
            }
            for (i, e) in template.expressions.iter().enumerate() {
                let evaluated = evaluate(e, scope, source, depth + 1)?;
                match evaluated.known_value() {
                    Some(value) => {
                        // The oracle concatenates `e.value + cooked` — plain JS
                        // stringification, same bounded rules as the fold.
                        result.push_str(&template_concat_value(value)?);
                        match quasi_cooked(template, i + 1, source) {
                            Some(text) => result.push_str(&text),
                            None => return gray("template quasi with invalid escape"),
                        }
                    }
                    None => {
                        return Ok(Evaluation::single(Entry::StringSentinel));
                    }
                }
            }
            Ok(Evaluation::known(Value::Str(result)))
        }

        Expression::MemberExpression(_) => match global_keypath(expr, scope, source) {
            // The oracle folds `global_constants` keypaths (Math.PI, …) — not
            // ported, so any global-rooted member read refuses.
            Some(keypath) => gray(format!(
                "global member {keypath} (global constants not ported)"
            )),
            None => Ok(Evaluation::single(Entry::Unknown)),
        },

        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
            Ok(Evaluation::single(Entry::FunctionSentinel))
        }

        // Values the oracle carries but this port cannot stringify/compare.
        Expression::RegexLiteral(_) => gray("regex literal value"),

        // Everything else is the oracle's `default: UNKNOWN` — portable.
        _ => Ok(Evaluation::single(Entry::Unknown)),
    }
}

fn all_numbers(eval: &Evaluation) -> bool {
    eval.values
        .iter()
        .all(|e| matches!(e, Entry::Value(Value::Num(_)) | Entry::NumberSentinel))
}

/// The cooked text of quasi `i` (decoded escapes), `None` for invalid escapes.
fn quasi_cooked(
    template: &tsv_ts::ast::internal::TemplateLiteral<'_>,
    i: usize,
    source: &str,
) -> Option<String> {
    use tsv_ts::ast::internal::TemplateCooked;
    let quasi = &template.quasis[i];
    match &quasi.cooked {
        TemplateCooked::Verbatim => Some(quasi.raw_span.extract(source).to_string()),
        TemplateCooked::Decoded(s) => Some((*s).to_string()),
        TemplateCooked::Invalid => None,
    }
}

/// JS string concatenation operand (`'' + value`) — like the fold but `null`
/// prints as `"null"` and `undefined` as `"undefined"` (no `?? ''` here).
fn template_concat_value(value: &Value) -> Result<String, Gray> {
    match value {
        Value::Null => Ok("null".to_string()),
        Value::Undefined => Ok("undefined".to_string()),
        _ => stringify_value(value),
    }
}

/// The literal's value in the ported domain.
fn literal_value(lit: &tsv_ts::ast::internal::Literal<'_>, source: &str) -> EvalResult {
    use tsv_ts::ast::internal::{LiteralValue, StringCooked};
    match &lit.value {
        LiteralValue::Number(n) => Ok(Evaluation::known(Value::Num(*n))),
        LiteralValue::Boolean(b) => Ok(Evaluation::known(Value::Bool(*b))),
        LiteralValue::Null => Ok(Evaluation::known(Value::Null)),
        LiteralValue::String(cooked) => {
            let raw = lit.span.extract(source);
            let inner = &raw[1..raw.len() - 1];
            let value = match cooked {
                StringCooked::Verbatim => inner.to_string(),
                StringCooked::Decoded(s) => (*s).to_string(),
            };
            Ok(Evaluation::known(Value::Str(value)))
        }
        LiteralValue::BigInt => gray("bigint literal value"),
    }
}

/// The callee's global keypath (`Math.max`, `$state.raw`): a chain of
/// non-computed member accesses rooted at an identifier with **no** local
/// binding. `None` when the root is a local binding, computed, or not an
/// identifier.
fn global_keypath(expr: &Expression<'_>, scope: &Scope<'_, '_>, source: &str) -> Option<String> {
    match expr {
        Expression::Identifier(id) => {
            if id.escaped_name.is_some() {
                return None;
            }
            let start = id.span.start as usize;
            let name = &source[start..start + id.name_len as usize];
            if scope.is_local(name) {
                None
            } else {
                Some(name.to_string())
            }
        }
        Expression::MemberExpression(member) if !member.computed => {
            let object = global_keypath(member.object, scope, source)?;
            let Expression::Identifier(prop) = member.property else {
                return None;
            };
            if prop.escaped_name.is_some() {
                return None;
            }
            let start = prop.span.start as usize;
            let name = &source[start..start + prop.name_len as usize];
            Some(format!("{object}.{name}"))
        }
        _ => None,
    }
}

/// Compute a known binary operation, `Gray` outside the ported combos.
// The `** ` special-case compares against exactly ±1.0 — the ECMAScript
// `Number::exponentiate` rule being ported, not an approximate comparison.
#[allow(clippy::float_cmp)]
fn binary_op(op: BinaryOperator, a: &Value, b: &Value) -> Result<Value, Gray> {
    use BinaryOperator::{
        BangEquals, BangEqualsEquals, EqualsEquals, EqualsEqualsEquals, GreaterThan,
        GreaterThanEquals, LessThan, LessThanEquals, Minus, Percent, Plus, Slash, Star, StarStar,
    };
    use Value::{Bool, Num, Str};
    match (op, a, b) {
        (Plus, Num(x), Num(y)) => Ok(Num(x + y)),
        (Plus, Str(x), Str(y)) => Ok(Str(format!("{x}{y}"))),
        (Plus, Str(x), y) => Ok(Str(format!("{x}{}", template_concat_value(y)?))),
        (Plus, x, Str(y)) => Ok(Str(format!("{}{y}", template_concat_value(x)?))),
        (Minus, Num(x), Num(y)) => Ok(Num(x - y)),
        (Star, Num(x), Num(y)) => Ok(Num(x * y)),
        (Slash, Num(x), Num(y)) => Ok(Num(x / y)),
        (Percent, Num(x), Num(y)) => Ok(Num(x % y)),
        (StarStar, Num(x), Num(y)) => {
            // ECMAScript `Number::exponentiate` diverges from IEEE `pow`: a NaN
            // exponent is always NaN (IEEE: `1 ** NaN` is 1), and |base| == 1
            // with an infinite exponent is NaN (IEEE: 1).
            if y.is_nan() || (x.abs() == 1.0 && y.is_infinite()) {
                Ok(Num(f64::NAN))
            } else {
                Ok(Num(x.powf(*y)))
            }
        }
        (LessThan, Num(x), Num(y)) => Ok(Bool(x < y)),
        (LessThanEquals, Num(x), Num(y)) => Ok(Bool(x <= y)),
        (GreaterThan, Num(x), Num(y)) => Ok(Bool(x > y)),
        (GreaterThanEquals, Num(x), Num(y)) => Ok(Bool(x >= y)),
        (EqualsEqualsEquals, x, y) => Ok(Bool(strict_equals(x, y))),
        (BangEqualsEquals, x, y) => Ok(Bool(!strict_equals(x, y))),
        (EqualsEquals | BangEquals, x, y) => {
            // Loose equality: only the coercion-free subset is ported.
            let same_type = std::mem::discriminant(x) == std::mem::discriminant(y);
            let both_nullish = x.nullish() && y.nullish();
            if same_type || both_nullish {
                let eq = if both_nullish {
                    true
                } else {
                    strict_equals(x, y)
                };
                Ok(Bool(if matches!(op, EqualsEquals) { eq } else { !eq }))
            } else {
                gray("loose equality with type coercion")
            }
        }
        _ => gray(format!(
            "binary `{}` on this operand combination",
            op.as_str()
        )),
    }
}

// Exact comparison IS the JS `===` semantics being ported.
#[allow(clippy::float_cmp)]
fn strict_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        // `===`: NaN is not equal to itself; -0 equals 0 (f64 == does both).
        (Value::Num(x), Value::Num(y)) => x == y,
        _ => a == b,
    }
}

/// Compute a known unary operation, `Gray` outside the ported combos.
fn unary_op(op: UnaryOperator, v: &Value) -> Result<Value, Gray> {
    use UnaryOperator::{Bang, Delete, Minus, Plus, Tilde, Typeof, Void};
    match op {
        Bang => Ok(Value::Bool(!v.truthy())),
        Void => Ok(Value::Undefined),
        Typeof => Ok(Value::Str(
            match v {
                Value::Str(_) => "string",
                Value::Num(_) => "number",
                Value::Bool(_) => "boolean",
                Value::Undefined => "undefined",
                Value::Null => "object",
            }
            .to_string(),
        )),
        Minus => Ok(Value::Num(-numeric_coerce(v)?)),
        Plus => Ok(Value::Num(numeric_coerce(v)?)),
        Tilde => {
            let n = numeric_coerce(v)?;
            Ok(Value::Num(f64::from(!to_int32(n))))
        }
        Delete => gray("delete on a known value"),
    }
}

/// JS ToNumber for the coercion-free subset (`Gray` for strings — string
/// numeric parsing is not ported).
fn numeric_coerce(v: &Value) -> Result<f64, Gray> {
    match v {
        Value::Num(n) => Ok(*n),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Null => Ok(0.0),
        Value::Undefined => Ok(f64::NAN),
        Value::Str(_) => gray("string-to-number coercion"),
    }
}

/// ECMAScript ToInt32.
// The modulo-2^32 wrap through u32 is the spec's ToInt32 — sign loss intended.
#[allow(clippy::cast_sign_loss)]
fn to_int32(n: f64) -> i32 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    let m = n.trunc() as i64;
    (m & 0xFFFF_FFFF) as u32 as i32
}

/// Collect the identifier names a binding pattern declares (destructure
/// properties, defaults, rests, nested patterns).
pub(crate) fn pattern_binding_names(
    pattern: &Expression<'_>,
    source: &str,
    out: &mut Vec<String>,
) -> Result<(), CompileError> {
    use tsv_ts::ast::internal::{ObjectPatternProperty, ObjectProperty};
    match pattern {
        Expression::Identifier(id) => {
            if id.escaped_name.is_none() {
                let start = id.span.start as usize;
                out.push(source[start..start + id.name_len as usize].to_string());
            }
            Ok(())
        }
        Expression::ObjectPattern(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        pattern_binding_names(&p.value, source, out)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        pattern_binding_names(rest.argument, source, out)?;
                    }
                }
            }
            Ok(())
        }
        Expression::ObjectExpression(obj) => {
            // Patterns reuse expression shapes in some positions.
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        pattern_binding_names(&p.value, source, out)?;
                    }
                    ObjectProperty::SpreadElement(s) => {
                        pattern_binding_names(s.argument, source, out)?;
                    }
                }
            }
            Ok(())
        }
        Expression::ArrayPattern(arr) => {
            for element in arr.elements.iter().flatten() {
                pattern_binding_names(element, source, out)?;
            }
            Ok(())
        }
        Expression::AssignmentPattern(assign) => pattern_binding_names(assign.left, source, out),
        Expression::RestElement(rest) => pattern_binding_names(rest.argument, source, out),
        other => Err(CompileError::Unsupported(Refusal::BindingPatternShape {
            kind: expression_kind(other),
        })),
    }
}

fn expression_kind(expr: &Expression<'_>) -> &'static str {
    // Only used for error messages on unusual pattern shapes.
    match expr {
        Expression::MemberExpression(_) => "member expression",
        Expression::CallExpression(_) => "call expression",
        _ => "unrecognized expression",
    }
}

/// A sanctioned rune declarator init, classified for rewriting.
pub(crate) enum RuneInit<'arena> {
    /// `$props()` (argument-less, direct call).
    Props,
    /// `$state(arg?)` / `$state.raw(arg?)` — the server drops the wrapper.
    State(Option<&'arena Expression<'arena>>),
    /// `$derived(expr)` — becomes `$.derived(() => expr)`.
    Derived(&'arena Expression<'arena>),
    /// `$derived.by(fn)` — becomes `$.derived(fn)`.
    DerivedBy(&'arena Expression<'arena>),
}

/// Classify a declarator init as a sanctioned rune call. `None` for anything
/// else (including malformed rune calls — the guard walk refuses those).
pub(crate) fn classify_rune_init<'arena>(
    init: &'arena Expression<'arena>,
    source: &str,
) -> Option<RuneInit<'arena>> {
    let Expression::CallExpression(call) = init else {
        return None;
    };
    let keypath = callee_keypath(call.callee, source)?;
    let arg = call.arguments.first();
    match keypath.as_str() {
        "$props" if call.arguments.is_empty() => Some(RuneInit::Props),
        "$state" | "$state.raw" if call.arguments.len() <= 1 => Some(RuneInit::State(arg)),
        "$derived" if call.arguments.len() == 1 => arg.map(RuneInit::Derived),
        "$derived.by" if call.arguments.len() == 1 => arg.map(RuneInit::DerivedBy),
        _ => None,
    }
}

/// Whether `expr` is a direct statement-position effect call
/// (`$effect(fn)` / `$effect.pre(fn)`) — dropped on the server, forcing the
/// `$$renderer.component(…)` wrapper.
pub(crate) fn is_effect_call<'arena>(
    expr: &'arena Expression<'arena>,
    source: &str,
) -> Option<&'arena Expression<'arena>> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let keypath = callee_keypath(call.callee, source)?;
    if (keypath == "$effect" || keypath == "$effect.pre") && call.arguments.len() == 1 {
        call.arguments.first()
    } else {
        None
    }
}

/// The plain (non-computed) identifier keypath of a callee: `$state`,
/// `$state.raw` — one identifier or one member level.
fn callee_keypath(callee: &Expression<'_>, source: &str) -> Option<String> {
    fn plain_name<'s>(
        id: &tsv_ts::ast::internal::Identifier<'_>,
        source: &'s str,
    ) -> Option<&'s str> {
        if id.escaped_name.is_some() {
            return None;
        }
        let start = id.span.start as usize;
        Some(&source[start..start + id.name_len as usize])
    }
    match callee {
        Expression::Identifier(id) => plain_name(id, source).map(str::to_string),
        Expression::MemberExpression(member) if !member.computed => {
            let Expression::Identifier(obj) = member.object else {
                return None;
            };
            let Expression::Identifier(prop) = member.property else {
                return None;
            };
            Some(format!(
                "{}.{}",
                plain_name(obj, source)?,
                plain_name(prop, source)?
            ))
        }
        _ => None,
    }
}

// Keep HashSet in the module's public surface for the callers' collections.
pub(crate) type NameSet = HashSet<String>;

/// A block-scope overlay entry (each item/index, `{:then}` value, `{@const}`).
pub(crate) enum ScopeEntry<'arena> {
    /// Masked to UNKNOWN: the binding exists but is never statically known to
    /// this port (each items/indexes, await values). Behaviorally equivalent to
    /// the oracle's UNKNOWN/NUMBER sentinels for every emission decision this
    /// port makes.
    Masked,
    /// A `{@const}` binding — evaluates through its initial like a top-level
    /// binding (the oracle folds statically-known const-tag reads).
    Const(Binding<'arena>),
}

/// The name-resolution context evaluation runs against: the top-level table
/// plus the active block-scope overlays (innermost last).
pub(crate) struct Scope<'a, 'arena> {
    pub bindings: &'a Bindings<'arena>,
    pub overlays: &'a [HashMap<String, ScopeEntry<'arena>>],
}

pub(crate) enum Resolved<'a, 'arena> {
    Masked,
    Binding(&'a Binding<'arena>),
    None,
}

impl<'a, 'arena> Scope<'a, 'arena> {
    pub fn resolve(&self, name: &str) -> Resolved<'a, 'arena> {
        for overlay in self.overlays.iter().rev() {
            match overlay.get(name) {
                Some(ScopeEntry::Masked) => return Resolved::Masked,
                Some(ScopeEntry::Const(binding)) => return Resolved::Binding(binding),
                None => {}
            }
        }
        match self.bindings.get(name) {
            Some(binding) => Resolved::Binding(binding),
            None => Resolved::None,
        }
    }

    /// Whether `name` resolves to anything local (overlay or table) — the
    /// global-keypath test (a rune/global root must be unresolved).
    pub fn is_local(&self, name: &str) -> bool {
        !matches!(self.resolve(name), Resolved::None)
    }
}
