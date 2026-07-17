//! `<style>` scoping analysis and CSS splicing.
//!
//! Supports top-level rules whose selectors are single, no-combinator compounds
//! built from type / id / class / attribute / universal simple selectors, plus
//! trailing (non-filtering) pseudo-classes/elements. Each compound becomes a
//! **kind-tagged predicate list** evaluated JOINTLY against a candidate element
//! (all predicates must hold on the SAME element), mirroring the oracle's
//! `relative_selector_might_apply_to_node` restricted to the no-combinator case
//! (`phases/2-analyze/css/css-prune.js`). The matched compounds gain the fixed
//! `svelte-tsvhash` hash class, **source-spliced** into the style text (author
//! whitespace preserved, never reprinted), matching the oracle byte-for-byte
//! (`phases/3-transform/css/index.js`).
//!
//! Combinators, `:global`, `:is`/`:where`/`:has`/`:not`, `:root`/`:host`,
//! nesting, at-rules, namespaced and escaped names, and bare pseudo-only
//! compounds all **refuse** — a later milestone. Any compound that matches no
//! element refuses (`CssSelectorNoMatch`): the oracle comment-wraps an unused
//! rule, an over-refusal tsv declines to reproduce.

use tsv_css::ast::internal::{
    AttributeMatcher, ComplexSelector, CssBlockChild, CssNode, SimpleSelector,
};
use tsv_svelte::ast::internal::{AttributeNode, AttributeValue, Element, Style};
use tsv_ts::ast::internal::Expression;

use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// The deterministic scoping class — the fixed `cssHash` the oracle sidecar
/// compiles with, so outputs are byte-comparable across runs.
pub(crate) const SCOPE_HASH_CLASS: &str = "svelte-tsvhash";

/// HTML attributes whose enumerated values are case-insensitive per the HTML
/// spec, so a CSS attribute selector matches them case-insensitively unless an
/// explicit `s` flag forces sensitivity (the oracle's `case_insensitive_attributes`,
/// `css-prune.js:30-67`). Lower-cased names.
const HTML_CASE_INSENSITIVE_ATTRIBUTES: &[&str] = &[
    "accept-charset",
    "autocapitalize",
    "autocomplete",
    "behavior",
    "charset",
    "crossorigin",
    "decoding",
    "dir",
    "direction",
    "draggable",
    "enctype",
    "enterkeyhint",
    "fetchpriority",
    "formenctype",
    "formmethod",
    "formtarget",
    "hidden",
    "http-equiv",
    "inputmode",
    "kind",
    "loading",
    "method",
    "preload",
    "referrerpolicy",
    "rel",
    "rev",
    "role",
    "rules",
    "scope",
    "shape",
    "spellcheck",
    "target",
    "translate",
    "type",
    "valign",
    "wrap",
];

/// The pseudo-classes the oracle's matcher treats specially — a filter, a
/// global-like exemption, or a nested selector list. All refuse in this slice
/// (`css-prune.js` `PseudoClassSelector` case + the top-of-loop `:has`).
const SPECIAL_PSEUDO_CLASSES: &[&str] = &["global", "host", "root", "is", "where", "has", "not"];

/// A single scoped compound: the joint-AND predicate list, the source-splice
/// site for the hash class, and the source text (for the no-match refusal).
pub(crate) struct ScopedSelector {
    predicates: Vec<Predicate>,
    splice: Splice,
    /// The compound's source text — the `CssSelectorNoMatch` refusal message.
    display: String,
}

/// One simple selector's element filter. A pseudo-class/element that imposes no
/// filter (the generic case) contributes no predicate; the special pseudos
/// refuse at analysis time, so a `Predicate` never carries a pseudo.
enum Predicate {
    /// `*` — matches any regular element.
    Universal,
    /// `div` — tag-name case-insensitive equality (the oracle's `TypeSelector`).
    Type(String),
    /// `.c` — routes through `attribute_matches(el, "class", name, "~=")`.
    Class(String),
    /// `#x` — routes through `attribute_matches(el, "id", name, "=")`.
    Id(String),
    /// `[a]` / `[a=b]` / `[a="b" i]` — the general `attribute_matches` path.
    Attribute {
        name: String,
        matcher: Option<AttributeMatcher>,
        value: Option<String>,
        case_insensitive: bool,
    },
}

/// A source-splice site: insert `.svelte-tsvhash` at `at`, dropping the source
/// bytes in `[at, remove_to)`. An **append** (the common case) has `at ==
/// remove_to` (insert, remove nothing); the bare-`*` **replace** sets `at =
/// universal.start`, `remove_to = universal.end` so the `*` vanishes.
#[derive(Clone, Copy)]
struct Splice {
    at: u32,
    remove_to: u32,
}

/// The scoping analysis product: the scoped compounds (predicate list + splice),
/// in source order.
pub(crate) struct ScopeInfo {
    pub(crate) selectors: Vec<ScopedSelector>,
}

/// Analyze a `<style>` for the supported shape: top-level rules whose selectors
/// are single, no-combinator compounds (type/id/class/attribute/universal +
/// trailing pseudo). Anything else refuses — the real matcher/pruner machinery
/// for combinators, `:global`, and pruning is a later milestone.
///
/// `sink` is the [`census`](crate::census) collect seam: when `None` the first
/// unsupported shape **bails** (the compile path), byte-identical to having no
/// parameter; when `Some`, each pushes its [`Refusal`] and the walk continues, so
/// a stylesheet's whole refusal set is collected in one pass. In collect mode the
/// returned [`ScopeInfo`] is partial and unused — only the sink matters.
pub(crate) fn analyze_style(
    style: &Style<'_>,
    source: &str,
    mut sink: Option<&mut Vec<Refusal>>,
) -> Result<ScopeInfo, CompileError> {
    let mut info = ScopeInfo {
        selectors: Vec::new(),
    };
    for node in style.css_stylesheet.nodes {
        let CssNode::Rule(rule) = node else {
            refuse(&mut sink, Refusal::CssAtRule)?;
            continue;
        };
        for child in rule.declarations {
            if matches!(child, CssBlockChild::Rule(_) | CssBlockChild::Atrule(_)) {
                refuse(&mut sink, Refusal::CssNestedRule)?;
                break;
            }
        }
        // An empty rule (no declarations — a bare `{}` or comment-only body) is
        // comment-wrapped `/* (empty) … */` by the oracle in non-dev mode; tsv
        // declines to reproduce the wrap and refuses (`is_empty`, index.js:424).
        if !rule
            .declarations
            .iter()
            .any(|child| matches!(child, CssBlockChild::Declaration(_)))
        {
            refuse(&mut sink, Refusal::CssEmptyRule)?;
            continue;
        }
        for complex in rule.selector.selectors {
            match build_selector(complex, source) {
                Ok(selector) => info.selectors.push(selector),
                Err(reason) => refuse(&mut sink, reason)?,
            }
        }
    }
    Ok(info)
}

/// Build the predicate list + splice for one top-level compound, or the
/// [`Refusal`] its shape maps to. Refuses combinators, `:global`/`:is`/`:where`/
/// `:has`/`:not`/`:root`/`:host`, namespaced or escaped names, nesting/`An+B`/
/// invalid simple selectors, and bare pseudo-only compounds.
fn build_selector(complex: &ComplexSelector<'_>, source: &str) -> Result<ScopedSelector, Refusal> {
    // A combinator = more than one relative selector, or a leading combinator.
    let [relative] = complex.children else {
        return Err(Refusal::CssCombinatorSelector);
    };
    if relative.combinator.is_some() {
        return Err(Refusal::CssCombinatorSelector);
    }

    let mut predicates = Vec::new();
    for simple in relative.selectors {
        match simple {
            SimpleSelector::Universal {
                namespace: None, ..
            } => predicates.push(Predicate::Universal),
            SimpleSelector::Type {
                namespace: None,
                span,
            } => {
                let name = span.extract(source);
                refuse_if_escaped(name)?;
                // The tag-name compare is case-insensitive (Unicode in the oracle),
                // so a non-ASCII type name refuses (the element-side name is
                // guarded in `predicate_matches`).
                refuse_if_non_ascii(name)?;
                predicates.push(Predicate::Type(name.to_string()));
            }
            SimpleSelector::Class { span } => {
                // Span text includes the leading `.`.
                let name = &span.extract(source)[1..];
                refuse_if_escaped(name)?;
                predicates.push(Predicate::Class(name.to_string()));
            }
            SimpleSelector::Id { span } => {
                // Span text includes the leading `#`.
                let name = &span.extract(source)[1..];
                refuse_if_escaped(name)?;
                predicates.push(Predicate::Id(name.to_string()));
            }
            SimpleSelector::Attribute {
                namespace: None,
                name_span,
                matcher,
                value,
                flags,
                ..
            } => {
                let name = name_span.extract(source);
                refuse_if_escaped(name)?;
                // The attribute NAME is always compared case-insensitively (the
                // oracle lowercases attribute names), so a non-ASCII selector name
                // refuses — the element-side attr name is guarded in the matcher.
                refuse_if_non_ascii(name)?;
                // The oracle's case-insensitivity rule (`css-prune.js:592-597`):
                // an explicit `i` flag, OR the HTML case-insensitive list when no
                // explicit `s` flag overrides it.
                let name_lower = name.to_ascii_lowercase();
                let case_insensitive = flags_has(*flags, 'i')
                    || (!flags_has(*flags, 's')
                        && HTML_CASE_INSENSITIVE_ATTRIBUTES.contains(&name_lower.as_str()));
                let value = match value {
                    Some(v) => {
                        refuse_if_escaped(v)?;
                        // A non-ASCII selector VALUE under a case-insensitive
                        // compare refuses (the ci value fold would be unreliable);
                        // a case-SENSITIVE compare is a byte test, so it's fine.
                        if case_insensitive {
                            refuse_if_non_ascii(v)?;
                        }
                        Some((*v).to_string())
                    }
                    None => None,
                };
                predicates.push(Predicate::Attribute {
                    name: name.to_string(),
                    matcher: *matcher,
                    value,
                    case_insensitive,
                });
            }
            // A pseudo-class imposes no element filter unless it is one of the
            // special (filtering / global-like / nested) ones, which refuse.
            SimpleSelector::PseudoClass { span, .. } => {
                let raw = span.extract(source);
                refuse_if_escaped(raw)?;
                let name = pseudo_name(raw);
                if SPECIAL_PSEUDO_CLASSES.contains(&name.as_str()) {
                    return Err(Refusal::CssUnsupportedSelector);
                }
            }
            // A pseudo-element never gates matching (`css-prune.js:598-600`).
            SimpleSelector::PseudoElement { span, .. } => {
                refuse_if_escaped(span.extract(source))?;
            }
            // Namespaced type/universal/attribute, nesting (`&`), `An+B`,
            // percentage, and invalid selectors are all out of scope.
            _ => return Err(Refusal::CssUnsupportedSelector),
        }
    }

    // A compound with no non-pseudo anchor (`:hover {}`, `:not(.c) {}`) is a bare
    // pseudo-only compound — the oracle prepends the hash, a different splice
    // shape refused in this slice.
    let Some(splice) = compute_splice(relative.selectors) else {
        return Err(Refusal::CssUnsupportedSelector);
    };
    debug_assert!(
        !predicates.is_empty(),
        "a splice anchor implies a non-pseudo predicate"
    );

    Ok(ScopedSelector {
        predicates,
        splice,
        display: complex.span.extract(source).to_string(),
    })
}

/// The splice site for a compound: the LAST non-pseudo simple selector (the
/// oracle's backward walk that skips trailing pseudo). A bare `*` **replaces**
/// its span; every other anchor **appends** after its end. `None` when the
/// compound is pseudo-only (no anchor).
fn compute_splice(simples: &[SimpleSelector<'_>]) -> Option<Splice> {
    for simple in simples.iter().rev() {
        match simple {
            SimpleSelector::PseudoClass { .. } | SimpleSelector::PseudoElement { .. } => continue,
            SimpleSelector::Universal {
                namespace: None,
                span,
            } => {
                return Some(Splice {
                    at: span.start,
                    remove_to: span.end,
                });
            }
            other => {
                let end = other.span().end;
                return Some(Splice {
                    at: end,
                    remove_to: end,
                });
            }
        }
    }
    None
}

/// The pseudo name: the span text with its leading `:`/`::` stripped, up to `(`
/// (a functional pseudo) or the end, lower-cased for the special-set test.
fn pseudo_name(raw: &str) -> String {
    let stripped = raw.trim_start_matches(':');
    let end = stripped.find('(').unwrap_or(stripped.len());
    stripped[..end].trim().to_ascii_lowercase()
}

/// Whether `flags` contains `ch` (the oracle's `flags?.includes(ch)`).
fn flags_has(flags: Option<&str>, ch: char) -> bool {
    flags.is_some_and(|f| f.contains(ch))
}

/// Refuse a selector name/value that carries a CSS escape: the oracle
/// un-escapes before comparing, a rule tsv declines to reproduce (escapes are
/// opaque to every scanner — see the CSS escape-opacity class), so a `\`-bearing
/// name refuses conservatively rather than risk a mis-scope.
fn refuse_if_escaped(text: &str) -> Result<(), Refusal> {
    if text.contains('\\') {
        return Err(Refusal::CssUnsupportedSelector);
    }
    Ok(())
}

/// Refuse a case-insensitive-comparison operand that is not pure ASCII: the
/// oracle case-folds with full-Unicode `.toLowerCase()`, tsv with ASCII-only
/// folding, which can disagree — so a non-ASCII operand refuses (safe
/// over-refusal). Posture-consistent with [`refuse_if_escaped`].
fn refuse_if_non_ascii(text: &str) -> Result<(), Refusal> {
    if !text.is_ascii() {
        return Err(Refusal::CssCaseInsensitiveNonAscii);
    }
    Ok(())
}

/// Record `reason`: in bail mode (`sink` is `None`) return it as an `Err` — the
/// `?` at the call site propagates it exactly as the original `return Err(…)`
/// did, so the compile path stays byte-identical; in collect mode push it and
/// return `Ok(())` so the caller continues to the next node/selector.
fn refuse(sink: &mut Option<&mut Vec<Refusal>>, reason: Refusal) -> Result<(), CompileError> {
    match sink {
        Some(collected) => {
            collected.push(reason);
            Ok(())
        }
        None => Err(unsupported(reason)),
    }
}

/// Whether `element` (a regular HTML element named `element_name`) satisfies the
/// compound's whole predicate list — the joint-AND match. Mirrors the oracle's
/// `relative_selector_might_apply_to_node` loop for the no-combinator case: every
/// predicate must hold on this ONE element.
///
/// A predicate whose match needs the oracle's `get_possible_values` bounded
/// static-eval over a dynamic/enumerable attribute value refuses
/// ([`Refusal::CssDynamicAttributeMatch`]) rather than risk a false verdict.
pub(crate) fn element_matches_selector(
    selector: &ScopedSelector,
    element: &Element<'_>,
    element_name: &str,
    source: &str,
) -> Result<bool, CompileError> {
    for predicate in &selector.predicates {
        if !predicate_matches(predicate, element, element_name, source)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The compound's source text — the `CssSelectorNoMatch` refusal message.
pub(crate) fn selector_display(selector: &ScopedSelector) -> &str {
    &selector.display
}

/// Whether one predicate holds on `element`.
fn predicate_matches(
    predicate: &Predicate,
    element: &Element<'_>,
    element_name: &str,
    source: &str,
) -> Result<bool, CompileError> {
    match predicate {
        Predicate::Universal => Ok(true),
        // The oracle's `TypeSelector`: tag-name case-insensitive equality via
        // full-Unicode `.toLowerCase()` (`*` and `SvelteElement` bypass, but `*`
        // is `Predicate::Universal` here and `<svelte:element>` refuses before
        // emission, so neither arises). A non-ASCII element name would need the
        // Unicode fold (the selector name is ASCII-guarded at analysis) — refuse.
        Predicate::Type(name) => {
            if !element_name.is_ascii() {
                return Err(unsupported(Refusal::CssCaseInsensitiveNonAscii));
            }
            Ok(element_name.eq_ignore_ascii_case(name))
        }
        Predicate::Class(name) => attribute_matches(
            element,
            "class",
            Some(name),
            Some(AttributeMatcher::Contains),
            false,
            false,
            source,
        ),
        Predicate::Id(name) => attribute_matches(
            element,
            "id",
            Some(name),
            Some(AttributeMatcher::Exact),
            false,
            false,
            source,
        ),
        Predicate::Attribute {
            name,
            matcher,
            value,
            case_insensitive,
        } => {
            // The `[open]` on `<details>`/`<dialog>` whitelist (`css-prune.js:20-23`)
            // matches unconditionally, no attribute needed.
            if is_attribute_whitelisted(element_name, name) {
                return Ok(true);
            }
            attribute_matches(
                element,
                name,
                value.as_deref(),
                *matcher,
                *case_insensitive,
                true,
                source,
            )
        }
    }
}

/// The oracle's `whitelist_attribute_selector` (`css-prune.js:20-23`): `[open]`
/// on `<details>`/`<dialog>` matches unconditionally.
fn is_attribute_whitelisted(element_name: &str, attr_name: &str) -> bool {
    let element_lower = element_name.to_ascii_lowercase();
    (element_lower == "details" || element_lower == "dialog")
        && attr_name.eq_ignore_ascii_case("open")
}

/// Port of the oracle's `attribute_matches` (`css-prune.js:713-822`) — does
/// `element` carry an attribute (or directive/spread) satisfying the selector's
/// `name`/`expected_value`/`operator`/`case_insensitive`? The over-approximation
/// is exact: a spread, a matching directive, or a presence test on a dynamic
/// value all report "could match," never "doesn't."
///
/// The one deliberate deviation: the oracle's `get_possible_values` bounded
/// static-eval over a dynamic/enumerable attribute value is **not ported**. A
/// single plain expression (`{x}`/`{o.p}`/`{f()}`) is `UNKNOWN` to the oracle →
/// assume match (reproduced exactly); anything the oracle might enumerate and
/// rule out (a literal/ternary/logical/array/object/template, or a mixed
/// text+expression value) refuses instead of guessing — a safe over-refusal,
/// never a false positive.
fn attribute_matches(
    element: &Element<'_>,
    name: &str,
    expected_value: Option<&str>,
    operator: Option<AttributeMatcher>,
    case_insensitive: bool,
    // Whether this is an *attribute* selector (`[a]`/`[a=b]`) vs. a class/id
    // selector routed here with an ASCII target name ("class"/"id"). Only an
    // attribute selector's target can Unicode-fold-match a non-ASCII element attr
    // name (Kelvin→k, …), so only then does a non-ASCII element attr name refuse;
    // for class/id the ASCII target is provably safe, so unrelated non-ASCII attr
    // names don't force a refusal.
    attribute_selector: bool,
    source: &str,
) -> Result<bool, CompileError> {
    let name_lower = name.to_ascii_lowercase();
    for node in element.attributes {
        match node {
            // A spread could carry any attribute — matches unconditionally.
            AttributeNode::SpreadAttribute(_) => return Ok(true),
            // A `bind:` whose logical name equals the selector's (case-sensitive,
            // the oracle's `attribute.name === name`).
            AttributeNode::BindDirective(bind) if bind.name_span.extract(source) == name => {
                return Ok(true);
            }
            // A `style:` directive matches any `[style…]` selector.
            AttributeNode::StyleDirective(_) if name_lower == "style" => return Ok(true),
            // A `class:` directive matches a `[class…]` selector: for `~=` iff the
            // directive name equals the expected class, else unconditionally.
            AttributeNode::ClassDirective(directive) if name_lower == "class" => {
                if matches!(operator, Some(AttributeMatcher::Contains)) {
                    if Some(directive.name_span.extract(source)) == expected_value {
                        return Ok(true);
                    }
                } else {
                    return Ok(true);
                }
            }
            AttributeNode::Attribute(attr) => {
                let attr_name = attr.name_span.extract(source);
                // The name compare is case-insensitive; ASCII folding can disagree
                // with the oracle's Unicode fold only when a non-ASCII char folds
                // across the ASCII boundary (Kelvin→k), which needs a non-ASCII
                // element attr name AND an attribute-selector target. Refuse it.
                if attribute_selector && !attr_name.is_ascii() {
                    return Err(unsupported(Refusal::CssCaseInsensitiveNonAscii));
                }
                if !attr_name.eq_ignore_ascii_case(&name_lower) {
                    continue;
                }
                // A boolean (valueless) attribute matches iff the selector is
                // presence-only (`operator === null`).
                let Some(values) = attr.value else {
                    return Ok(operator.is_none());
                };
                // A presence selector (`[a]`) matches whenever the attribute
                // exists, any value (including dynamic).
                let Some(expected) = expected_value else {
                    return Ok(true);
                };
                if let [AttributeValue::Text(text)] = values {
                    let data = text.data(source);
                    // A case-insensitive compare against a non-ASCII element value
                    // would need the oracle's Unicode fold — refuse (safe). A
                    // case-sensitive compare is a byte test and is fine.
                    if case_insensitive && !data.is_ascii() {
                        return Err(unsupported(Refusal::CssCaseInsensitiveNonAscii));
                    }
                    let matches = test_attribute(operator, expected, case_insensitive, &data);
                    // A non-matching static class/style may still match a later
                    // `class:`/`style:` directive — keep scanning.
                    if !matches && (name_lower == "class" || name_lower == "style") {
                        continue;
                    }
                    return Ok(matches);
                }
                // Dynamic / mixed value — the `get_possible_values` path.
                if let [AttributeValue::ExpressionTag(tag)] = values
                    && is_unknown_expression(&tag.expression)
                {
                    // A plain unresolvable expression is `UNKNOWN` to the oracle
                    // → assume match (exact).
                    return Ok(true);
                }
                return Err(unsupported(Refusal::CssDynamicAttributeMatch));
            }
            _ => {}
        }
    }
    Ok(false)
}

/// Whether the oracle's `gather_possible_values` yields `UNKNOWN` for a single
/// expression value — a plain identifier, member access, or call, which it
/// cannot bound (`css/utils.js`). These are exactly the shapes assume-match is
/// exact for; any enumerable shape (literal, ternary, logical, array, object,
/// template) is refused by the caller instead.
fn is_unknown_expression(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Identifier(_) | Expression::MemberExpression(_) | Expression::CallExpression(_)
    )
}

/// The oracle's `test_attribute` (`css-prune.js:679-703`): apply one attribute
/// operator, honoring case-insensitivity. `operator` is always `Some` when
/// reached (presence selectors return before this); the `None` arm mirrors `=`.
fn test_attribute(
    operator: Option<AttributeMatcher>,
    expected: &str,
    case_insensitive: bool,
    value: &str,
) -> bool {
    if case_insensitive {
        test_attribute_cs(
            operator,
            &expected.to_ascii_lowercase(),
            &value.to_ascii_lowercase(),
        )
    } else {
        test_attribute_cs(operator, expected, value)
    }
}

/// Case-sensitive operator application (the caller lower-cases both sides for the
/// case-insensitive path).
fn test_attribute_cs(operator: Option<AttributeMatcher>, expected: &str, value: &str) -> bool {
    match operator {
        None | Some(AttributeMatcher::Exact) => value == expected,
        // `~=`: whitespace-separated list contains the value. The oracle splits
        // on JS `/\s/` (`value.split(/\s/)`), which is NOT Rust's
        // `char::is_whitespace` — they diverge on NEL (U+0085, in Rust's set, not
        // JS's → over-split) and BOM (U+FEFF, in JS's `\s`, not Rust's →
        // under-split). `is_js_ws` matches JS `\s` exactly so the token set agrees.
        // (The class-value EMIT collapse — `collapse_attr_whitespace` — is a
        // *different* oracle operation, `regex_whitespaces_strict = /[ \t\n\r\f]+/`,
        // already matched there; the two need not and do not agree.)
        Some(AttributeMatcher::Contains) => value.split(is_js_ws).any(|token| token == expected),
        // `|=`: `${value}-`.startsWith(`${expected}-`).
        Some(AttributeMatcher::DashMatch) => {
            format!("{value}-").starts_with(&format!("{expected}-"))
        }
        Some(AttributeMatcher::Prefix) => value.starts_with(expected),
        Some(AttributeMatcher::Suffix) => value.ends_with(expected),
        Some(AttributeMatcher::Substring) => value.contains(expected),
    }
}

/// Whether `c` is JavaScript `\s` — the exact character class `RegExp`'s `/\s/`
/// matches, which is NOT Rust's `char::is_whitespace`: it drops NEL (U+0085) and
/// adds BOM (U+FEFF). Used only by the `~=` token split so it agrees with the
/// oracle's `value.split(/\s/)` byte-for-byte.
fn is_js_ws(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'   // tab
        | '\u{000A}' // LF
        | '\u{000B}' // VT
        | '\u{000C}' // FF
        | '\u{000D}' // CR
        | '\u{0020}' // space
        | '\u{00A0}' // NBSP
        | '\u{1680}' // OGHAM SPACE MARK
        | '\u{2000}'
            ..='\u{200A}' // EN QUAD … HAIR SPACE
        | '\u{2028}' // LINE SEPARATOR
        | '\u{2029}' // PARAGRAPH SEPARATOR
        | '\u{202F}' // NARROW NO-BREAK SPACE
        | '\u{205F}' // MEDIUM MATHEMATICAL SPACE
        | '\u{3000}' // IDEOGRAPHIC SPACE
        | '\u{FEFF}' // ZERO WIDTH NO-BREAK SPACE (BOM)
    )
}

/// The scoped CSS: the author's style text verbatim (whitespace preserved) with
/// `.svelte-tsvhash` spliced in — appended after each anchor, or replacing a bare
/// `*` — matching the oracle's output byte-for-byte. All selectors matched (any
/// unmatched compound refuses before this), so every splice is emitted.
pub(crate) fn splice_scoped_css(style: &Style<'_>, source: &str, scope: &ScopeInfo) -> String {
    let content_start = style.content_span.start;
    let content = style.content_span.extract(source);
    let mut splices: Vec<Splice> = scope.selectors.iter().map(|s| s.splice).collect();
    splices.sort_unstable_by_key(|s| s.at);
    let mut out = String::with_capacity(content.len() + 16 * splices.len());
    let mut prev = 0usize;
    for splice in &splices {
        let at = (splice.at - content_start) as usize;
        out.push_str(&content[prev..at]);
        out.push('.');
        out.push_str(SCOPE_HASH_CLASS);
        prev = (splice.remove_to - content_start) as usize;
    }
    out.push_str(&content[prev..]);
    out
}
