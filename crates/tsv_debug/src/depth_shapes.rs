//! The shapes `tests/depth_limit.rs` generates at the parsers' depth limits.
//!
//! Here rather than beside that test because `build_fanout_audit` grades the same catalog,
//! for the work a format does at small depths — one generator, so the two gates cannot come
//! to grade different catalogs.
//!
//! Every shape is a [`Shape`] — fixed text around a unit repeated `n` times, and a closer
//! repeated as often (`pre + unit×n + mid + close×n + post`) — so the gate searches each
//! one's accepted `n`, and the WASM leg (`scripts/depth_wasm.ts`) regenerates the same
//! sources from the spec lines `write_shape_limits` writes.
//!
//! Three families:
//!
//! - **named** shapes, one per construct the limits count;
//! - the **positions** ([`POSITIONS`]): every grammar position that holds a construct which
//!   can hold the position again, nested in itself — what keeps the wire bound
//!   (`check_wire`) seeing every recursive step the parsers take, not only the ones a
//!   shape was written for;
//! - a **generated sweep**: every ordered pair, and a seeded sample of triples, over the
//!   whole link alphabet ([`SEGMENTS`] — every chain link kind and every binary operator)
//!   behind each of [`HEADS`] ([`Scope::Full`]) or of [`CHECK_HEADS`] ([`Scope::Check`]),
//!   and the same links with comments in their gaps ([`commented_links`]). What a walk spends on a link depends on its neighbours — a
//!   call on a call, a `!` over a call, a tag beside a member, an `==` over an `==`, an
//!   operator over an `as` — each leaving the iterative walk for a recursive one, so the
//!   gate generates the combinations rather than trusting a list of the ones that do.
//!   Beside it, the **prefix alphabet** ([`PREFIXES`], [`prefix_sweep`]): every prefix
//!   operator nested over every other, bare and with comments in their gaps, over operands
//!   ending in each link, and as the operands of each binary operator — where the printer
//!   adds the pairs a format's reparse counts (`-(-a)`, `!(/* c */ a)`, `<T>(<U>a)`,
//!   `new A()`), so a pair site the parser does not charge fails the F1 leg — and nested
//!   over authored pairs, bare and commented ([`prefix_pair_sweep`]). And the
//!   **decorators** ([`decorator_sweep`]): a class nested in a decorator's argument, in
//!   each position a decorator takes, behind each callee and inside each payload.
//!   Which ones parse is the parser's answer: a combination it rejects (`a?.b`t``,
//!   `a ?? a || a`, `new -a`) is skipped, and counted.

use std::sync::LazyLock;

use tsv_lang::SPINE_LIMIT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ts,
    Css,
    Svelte,
}

impl Lang {
    pub fn name(self) -> &'static str {
        match self {
            Lang::Ts => "typescript",
            Lang::Css => "css",
            Lang::Svelte => "svelte",
        }
    }
}

/// Which limit refuses the shape one level past its accepted depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Nesting,
    Spine,
    /// A generated combination, which either limit may end (a right-associative `**`
    /// nests, every other link is a spine link).
    Either,
    /// A named shape either limit may end: a construct whose own node is a spine link
    /// beside its nesting (a tagged template's tag, a conditional's test, the
    /// [`POSITIONS`] whose wrapper takes one), whose spine can run out first.
    Any,
    /// A chain each of whose links the printer parenthesizes (`(a == b) == c`), which
    /// either limit may end: the pairs, charged as levels of the output's nesting, or the
    /// links with that nesting charged on top.
    PairedChain,
}

/// How a shape's accepted depth relates to its wire, beyond the bound every shape meets:
/// at its accepted depth, no path through its wire nests deeper than the nesting limit
/// or holds more left-spine links than the spine limit, with a few levels of document
/// frame in hand (`check_wire` in `tests/depth_limit.rs`) — the parse charges at least
/// what the walks recurse through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    /// Calibrated on the nesting: the wire's depth (left-spine links aside) must also sit
    /// AT the limit, so the count does not overcharge the shape either.
    Nesting,
    /// Calibrated on the spine: its path's spine links, at the units the shape's link
    /// kinds count (a ratio, units over links), must sit at the limit.
    Spine(u32, u32),
    /// The bound alone: the count may charge more than the wire records — a pair the
    /// printer adds, a CSS value's functions (a string on the wire), a step charged above
    /// its wire nodes, or a combined shape whose two counts the calibration does not
    /// separate.
    Bounded,
}

pub struct Shape {
    pub name: String,
    pub lang: Lang,
    pub limit: Limit,
    pub wire: Wire,
    pub pre: String,
    pub unit: String,
    pub mid: String,
    pub close: String,
    pub post: String,
}

impl Shape {
    /// The source at depth `n`.
    pub fn make(&self, n: usize) -> String {
        let mut out = String::with_capacity(
            self.pre.len()
                + (self.unit.len() + self.close.len()) * n
                + self.mid.len()
                + self.post.len(),
        );
        out.push_str(&self.pre);
        for _ in 0..n {
            out.push_str(&self.unit);
        }
        out.push_str(&self.mid);
        for _ in 0..n {
            out.push_str(&self.close);
        }
        out.push_str(&self.post);
        out
    }

    /// Whether the shape belongs to the generated sweep.
    pub fn generated(&self) -> bool {
        self.limit == Limit::Either
    }
}

/// The links the generated sweep combines: every link kind of an expression chain, each in
/// one spelling, and every binary and logical operator, each with its right operand.
pub const SEGMENTS: &[&str] = &[
    // chain links
    ".b",
    "()",
    "!",
    "[0]",
    "?.b",
    "?.()",
    "<T>()",
    "`t`",
    " as T",
    " satisfies T",
    "(x)",
    // a call whose argument is a multi-line template, which a chain prints whole
    "(`\nx\n`)",
    ".o(`\nx\n`)",
    // binary and logical operators, every precedence class
    " ?? a",
    " || a",
    " && a",
    " | a",
    " ^ a",
    " & a",
    " == a",
    " != a",
    " === a",
    " !== a",
    " < a",
    " > a",
    " <= a",
    " >= a",
    " instanceof a",
    " in a",
    " << a",
    " >> a",
    " >>> a",
    " + a",
    " - a",
    " * a",
    " / a",
    " % a",
    " ** a",
];

/// The heads the generated sweep starts from: a bare name, a member (behind which a call
/// is a member chain's, and a call on that call the curried call printed apart), a call,
/// `this`, a parenthesized name, and a `new` (behind which a member is its callee's).
pub const HEADS: &[&str] = &["a", "a.b", "a()", "this", "(a)", "new A"];

/// The heads `check`'s sweep starts from ([`Scope::Check`]): a bare name and a call, the two
/// whose neighbours change a link's walk the most (a call on a call is the curried call
/// printed apart; a member over a call may print it whole).
pub const CHECK_HEADS: &[&str] = &["a", "a()"];

/// The prefix operators the generated sweep nests ([`prefix_sweep`]): every unary and
/// keyword operator, `await` and `yield` (in an async generator's body), an angle-bracket
/// assertion, `new` and the prefix updates — each spelled so that it lexes apart from the
/// one after it.
pub const PREFIXES: &[&str] = &[
    "- ", "+ ", "!", "~", "typeof ", "void ", "delete ", "await ", "yield ", "<T>", "new ", "++",
    "--",
];

/// The comments the prefix sweep puts in an operator's gap.
const PREFIX_COMMENTS: &[&str] = &["/*c*/", "// c\n"];

/// The links `check`'s prefix sweep ends its operands in ([`Scope::Check`]); the full sweep
/// takes every one of [`SEGMENTS`].
const CHECK_PREFIX_LINKS: &[&str] = &[".b", "()", "!", "`t`", " as T"];

/// The binary operators `check`'s prefix sweep takes prefixed operands of
/// ([`Scope::Check`]); the full sweep takes every operator in [`SEGMENTS`].
const CHECK_PREFIX_OPERATORS: &[&str] = &[" + a", " == a", " ** a", " ?? a"];

/// How many triples the full sweep samples, behind seeded heads; the pairs are exhaustive.
pub const SAMPLED_TRIPLES: usize = 1_500;

/// How many triples `check`'s sweep samples ([`Scope::Check`]).
pub const CHECK_TRIPLES: usize = 300;

/// The sample's seed: fixed, so a run grades the same triples every time.
const TRIPLE_SEED: u64 = 0x7473_765f_6465_7074;

/// How much of the generated sweep a run grades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every pair behind [`CHECK_HEADS`], [`CHECK_TRIPLES`] seeded triples, the commented
    /// links and `check`'s sample of the prefix alphabet — what `deno task test:depth`
    /// grades inside `check`.
    Check,
    /// Every pair behind every one of [`HEADS`], [`SAMPLED_TRIPLES`] seeded triples, the
    /// commented links and the whole prefix alphabet — what the shipped-profile run (`deno task test:depth:napi`, with
    /// `TSV_DEPTH_SWEEP=full`) and the WASM leg grade.
    Full,
}

impl Scope {
    /// The scope a run asks for: `TSV_DEPTH_SWEEP=full` names the full sweep, anything else
    /// `check`'s.
    pub fn from_env() -> Scope {
        if std::env::var("TSV_DEPTH_SWEEP").is_ok_and(|v| v == "full") {
            Scope::Full
        } else {
            Scope::Check
        }
    }
}

/// The shapes a scope grades: the named ones, then the generated sweep
/// (`x = <head><segments>×n;`).
pub fn shapes(scope: Scope) -> &'static [Shape] {
    static CHECK: LazyLock<Vec<Shape>> = LazyLock::new(|| build_shapes(Scope::Check));
    static FULL: LazyLock<Vec<Shape>> = LazyLock::new(|| build_shapes(Scope::Full));
    match scope {
        Scope::Check => &CHECK,
        Scope::Full => &FULL,
    }
}

/// A generated combination: `x = <head><unit>×n;`.
fn generated(unit: String, head: &str) -> Shape {
    Shape {
        name: format!("ts_gen `{head}` + `{}`", unit.replace('\n', "\\n")),
        lang: Lang::Ts,
        limit: Limit::Either,
        wire: Wire::Bounded,
        pre: format!("x = {head}"),
        unit,
        mid: String::new(),
        close: String::new(),
        post: ";\n".to_owned(),
    }
}

fn build_shapes(scope: Scope) -> Vec<Shape> {
    let (heads, triples) = match scope {
        Scope::Check => (CHECK_HEADS, CHECK_TRIPLES),
        Scope::Full => (HEADS, SAMPLED_TRIPLES),
    };
    let mut shapes = named_shapes();
    for &head in heads {
        for &first in SEGMENTS {
            for &second in SEGMENTS {
                shapes.push(generated([first, second].concat(), head));
            }
        }
    }
    let mut state = TRIPLE_SEED;
    let mut next = |bound: usize| {
        // xorshift64*: deterministic, and dependency-free
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % bound
    };
    for _ in 0..triples {
        let head = heads[next(heads.len())];
        let triple = [
            SEGMENTS[next(SEGMENTS.len())],
            SEGMENTS[next(SEGMENTS.len())],
            SEGMENTS[next(SEGMENTS.len())],
        ];
        shapes.push(generated(triple.concat(), head));
    }
    commented_links(&mut shapes);
    prefix_sweep(&mut shapes, scope == Scope::Full);
    if scope == Scope::Full {
        prefix_pair_sweep(&mut shapes);
    }
    decorator_sweep(&mut shapes, scope == Scope::Full);
    shapes
}

/// A generated prefix combination, in an async generator's body so that `await` and
/// `yield` are operators: `x = <lead><unit>×n<mid>;`.
fn generated_prefix(lead: &str, unit: String, mid: &str) -> Shape {
    Shape {
        name: format!(
            "ts_gen_prefix `{lead}` + `{}` `{mid}`",
            unit.replace('\n', "\\n")
        ),
        lang: Lang::Ts,
        limit: Limit::Either,
        wire: Wire::Bounded,
        pre: format!("async function* g() {{\n\tx = {lead}"),
        unit,
        mid: mid.to_owned(),
        close: String::new(),
        post: ";\n}\n".to_owned(),
    }
}

/// The forms an author writes a prefix operator's operand in with a pair of their own,
/// as (before, after) the nested operand: a bare pair, a comment inside it on either side,
/// and a comment between the operator and the pair — the pairs the printer may keep,
/// strip or write again, which the parse charges the difference of.
const PREFIX_PAIR_FORMS: &[(&str, &str)] = &[
    ("(", ")"),
    ("(/*c*/", ")"),
    ("(", "/*c*/)"),
    ("/*c*/(", ")"),
];

/// The operators the prefix pair sweep puts inside an authored pair, after the operator
/// outside it ([`prefix_pair_sweep`]).
const PREFIX_PAIR_INNER: &[&str] = &["", "- ", "!", "<T>", "await "];

/// What the prefix pair sweep follows an authored pair with ([`prefix_pair_sweep`]):
/// nothing, a member after a `!`, an instantiation, and a member chain of two calls, which
/// the member-chain printer lays out in several candidates around the pair as its base —
/// links a printer takes a path of its own for behind a pair it may keep.
const PREFIX_PAIR_LINKS: &[&str] = &["", "!.b", "<T>", ".b().c()"];

/// The prefix operators nested over their authored pairs ([`PREFIX_PAIR_FORMS`]), bare and
/// with a second operator inside the pair ([`PREFIX_PAIR_INNER`]), each pair followed by
/// each of [`PREFIX_PAIR_LINKS`] — and, in an operator's place, the pair as a binary
/// operator's right operand: `x = <op>(<inner>×n a)<link>×n` — the full sweep's alone.
///
/// TODO: K305, K306 — two forms are left out, each a printer bug of its own that no depth
/// reaches: they format at a single level to a form whose own format differs (F1). A block
/// comment opening a prefix operator's bare pair before a link (K305: `-(/*c*/a)!.b` →
/// `-/*c*/ a!.b` → `-(/*c*/ a!.b)`, the same with `<T>` and a member chain), and one closing a binary operand's
/// pair (K306: `a == (a /*c*/)` → `a == a /*c*/` → `a == a; /*c*/`).
fn prefix_pair_sweep(shapes: &mut Vec<Shape>) {
    for &op in PREFIXES.iter().chain(&["a == "]) {
        for &(before, after) in PREFIX_PAIR_FORMS {
            for &inner in PREFIX_PAIR_INNER {
                for &link in PREFIX_PAIR_LINKS {
                    let binary = op == "a == ";
                    let unary_comment_link =
                        !binary && before == "(/*c*/" && inner.is_empty() && !link.is_empty();
                    let binary_trailing_comment = binary && after == "/*c*/)" && link.is_empty();
                    if unary_comment_link || binary_trailing_comment {
                        continue;
                    }
                    shapes.push(Shape {
                        name: format!("ts_gen_prefix_pair `{op}{before}{inner}…{after}{link}`"),
                        close: [after, link].concat(),
                        ..generated_prefix("", [op, before, inner].concat(), "a")
                    });
                }
            }
        }
    }
}

/// The prefix alphabet ([`PREFIXES`]): every ordered pair of operators nested `n` times,
/// each operator with a comment in its gap, and each operator over an operand ending in a
/// link and as the operand of a binary operator — `full` over every link and operator and
/// with a comment between every pair, else `check`'s sample.
///
/// A line comment after `yield` ends its operand (`yield` is a restricted production), so
/// that combination nests nothing and is left out.
fn prefix_sweep(shapes: &mut Vec<Shape>, full: bool) {
    let comment_fits = |op: &str, comment: &str| !(op == "yield " && comment.contains('\n'));
    for &first in PREFIXES {
        for &second in PREFIXES {
            shapes.push(generated_prefix("", [first, second].concat(), "a"));
        }
    }
    for &op in PREFIXES {
        for &comment in PREFIX_COMMENTS.iter().filter(|c| comment_fits(op, c)) {
            shapes.push(generated_prefix("", [op, comment].concat(), "a"));
            if full {
                for &second in PREFIXES {
                    shapes.push(generated_prefix("", [op, comment, second].concat(), "a"));
                }
            }
        }
    }
    let (links, operators) = if full {
        (SEGMENTS, SEGMENTS)
    } else {
        (CHECK_PREFIX_LINKS, CHECK_PREFIX_OPERATORS)
    };
    for &op in PREFIXES {
        for &link in links {
            shapes.push(generated_prefix("", op.to_owned(), &format!("a{link}")));
        }
        // a binary operator's right operand, prefixed: `a + -a + -a`
        for &operator in operators {
            if let Some(symbol) = operator.strip_suffix(" a")
                && symbol.starts_with(' ')
            {
                shapes.push(generated_prefix("a", format!("{symbol} {op}a"), ""));
            }
        }
    }
}

/// The callees the decorator sweep calls ([`decorator_sweep`]): a name, a member chain, an
/// instantiation, a non-null over either (which the printer parenthesizes, `@(a!(…))`),
/// a parenthesized callee, and none — the parenthesized decorator `@(…)` itself.
const DECORATOR_CALLEES: &[&str] = &["a", "a.b.c", "a<T>", "a!", "a!.b", "a.b!", "(a)", ""];

/// The payloads the decorator sweep wraps its nested class in, as (before, after): bare, a
/// spread, an assertion, a non-null, a member, an earlier argument, a comment, a tag and an
/// angle-bracket assertion.
const DECORATOR_PAYLOADS: &[(&str, &str)] = &[
    ("", ""),
    ("...", ""),
    ("", " as T"),
    ("", " satisfies T"),
    ("", "!"),
    ("", ".b"),
    ("x, ", ""),
    ("/*c*/", ""),
    ("", "`t`"),
    ("<T>", ""),
];

/// The callees and payloads `check`'s decorator sweep takes ([`Scope::Check`]).
const CHECK_DECORATOR_CALLEES: &[&str] = &["a", "a.b.c", "a!", ""];
const CHECK_DECORATOR_PAYLOADS: &[(&str, &str)] =
    &[("", ""), ("...", ""), ("", " as T"), ("", "!"), ("", ".b")];

/// The decorators: a class nested in a decorator's argument, `n` deep, in each position a
/// decorator takes — on a class expression, on a class member (a field, a static block, an
/// index signature), and on a method's parameter — behind every callee and inside every
/// payload ([`DECORATOR_CALLEES`], [`DECORATOR_PAYLOADS`]; `full`, else `check`'s sample).
/// Each repetition puts the class, its member or parameter, the decorator and its call on
/// the path.
fn decorator_sweep(shapes: &mut Vec<Shape>, full: bool) {
    let (callees, payloads) = if full {
        (DECORATOR_CALLEES, DECORATOR_PAYLOADS)
    } else {
        (CHECK_DECORATOR_CALLEES, CHECK_DECORATOR_PAYLOADS)
    };
    for &callee in callees {
        for &(before, after) in payloads {
            // the parenthesized decorator holds an expression, not an argument list
            if callee.is_empty() && (before == "..." || before == "x, ") {
                continue;
            }
            let decorator = format!("@{callee}({before}");
            for (position, unit, close) in [
                ("class", decorator.clone(), format!("{after}) class {{}}")),
                (
                    "member",
                    format!("class {{ {decorator}"),
                    format!("{after}) y }}"),
                ),
                (
                    "parameter",
                    format!("class {{ m({decorator}"),
                    format!("{after}) p) {{}} }}"),
                ),
                (
                    "static block",
                    format!("class {{ {decorator}"),
                    format!("{after}) static {{}} }}"),
                ),
                (
                    "index signature",
                    format!("class {{ {decorator}"),
                    format!("{after}) [k: string]: T }}"),
                ),
            ] {
                shapes.push(Shape {
                    name: format!("ts_gen_decorator {position} `{unit}…{close}`"),
                    lang: Lang::Ts,
                    limit: Limit::Either,
                    wire: Wire::Bounded,
                    pre: "x = ".to_owned(),
                    unit,
                    mid: "class {}".to_owned(),
                    close,
                    post: ";\n".to_owned(),
                });
            }
        }
    }
}

/// The commented links: a comment in every gap a link opens, where a printer may take a
/// path of its own for the comment's sake (a block before a `!` once re-entered the
/// expression printer per link). Every segment with a block comment after it, and with one
/// on either side; every ordered pair joined by a block; and a line comment joining a pair
/// wherever the grammar lets the next link continue past the line break — before an
/// operator, a member, a call, a template — behind a representative first link.
fn commented_links(shapes: &mut Vec<Shape>) {
    for &segment in SEGMENTS {
        shapes.push(generated(format!("{segment}/*c*/"), "a"));
        shapes.push(generated(format!("/*c*/{segment}/*c*/"), "a"));
    }
    for &first in SEGMENTS {
        for &second in SEGMENTS {
            shapes.push(generated(format!("{first}/*c*/{second}"), "a"));
        }
    }
    // a `!`, an `as` or a `satisfies` after a line break starts a new statement, which
    // would end no chain at all (as does any link after a type, which ends at the break)
    let continues = |segment: &str| {
        !segment.starts_with('!')
            && !segment.starts_with(" as ")
            && !segment.starts_with(" satisfies ")
    };
    for &first in &[".b", "()", "!", "`t`", " + a", " == a", " * a / a"] {
        for &second in SEGMENTS.iter().filter(|s| continues(s)) {
            shapes.push(generated(format!("{first} // c\n{second}"), "a"));
        }
    }
}

/// The spine units of a link whose walk re-enters the expression printer — the parser's
/// `LINK_REENTRY`, restated for the flat shapes whose wire it calibrates.
const REENTRY: u32 = 10;

/// The spine units of a member access, a call or an instantiation the member-chain
/// printer flattens — the parser's `LINK_SUBSCRIPT`, restated for the
/// flat shapes whose wire it calibrates.
const SUBSCRIPT: u32 = 1;

/// The spine units of a non-null assertion the member-chain printer flattens — the
/// parser's `LINK_NON_NULL`, restated for the flat shapes whose wire it calibrates.
const NON_NULL: u32 = 2;

pub fn rep(s: &str, n: usize) -> String {
    s.repeat(n)
}

/// A flat chain of `n` terms joined by `op`.
pub fn chain(head: &str, op: &str, n: usize) -> String {
    let mut out = String::from(head);
    for _ in 1..n {
        out.push_str(op);
        out.push_str(head);
    }
    out
}

/// A named shape: `pre + unit×n + mid + close×n + post`.
#[expect(clippy::too_many_arguments)]
fn shape(
    name: &str,
    lang: Lang,
    limit: Limit,
    wire: Wire,
    pre: &str,
    unit: &str,
    mid: &str,
    close: &str,
    post: &str,
) -> Shape {
    Shape {
        name: name.to_owned(),
        lang,
        limit,
        wire,
        pre: pre.to_owned(),
        unit: unit.to_owned(),
        mid: mid.to_owned(),
        close: close.to_owned(),
        post: post.to_owned(),
    }
}

/// A named TypeScript nesting shape recorded by the wire: `pre + unit×n + mid + close×n + post`.
fn ts_nesting(name: &str, pre: &str, unit: &str, mid: &str, close: &str, post: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Nesting,
        Wire::Nesting,
        pre,
        unit,
        mid,
        close,
        post,
    )
}

/// A named flat TypeScript chain: `pre + unit×n + post`, its links counting `units` each.
fn ts_flat(name: &str, units: u32, pre: &str, unit: &str, post: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Spine,
        Wire::Spine(units, 1),
        pre,
        unit,
        "",
        "",
        post,
    )
}

/// A named chain whose every link the printer parenthesizes ([`Limit::PairedChain`]):
/// `pre + unit×n + post`.
fn ts_paired(name: &str, pre: &str, unit: &str, post: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::PairedChain,
        Wire::Bounded,
        pre,
        unit,
        "",
        "",
        post,
    )
}

/// A named TypeScript prefix operator nested over itself, `x = <unit>×n<mid>;`, whose
/// printer pairs the wire does not record.
fn ts_prefix(name: &str, unit: &str, mid: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Nesting,
        Wire::Bounded,
        "x = ",
        unit,
        mid,
        "",
        ";\n",
    )
}

/// A named TypeScript leaf at the deepest nesting, `x = [×n<leaf>]×n;`, for a pair the
/// printer adds at the leaf alone.
fn ts_leaf(name: &str, leaf: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Nesting,
        Wire::Bounded,
        "x = ",
        "[",
        leaf,
        "]",
        ";\n",
    )
}

/// A named TypeScript type at the deepest tuple nesting, `type T = [×n<leaf>]×n;`, for a
/// pair the type printer adds at the leaf alone.
fn ts_type_leaf(name: &str, leaf: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Nesting,
        Wire::Bounded,
        "type T = ",
        "[",
        leaf,
        "]",
        ";\n",
    )
}

/// A named TypeScript nesting shape each level of which the printer adds a pair to,
/// `x = <unit>×n<mid><close>×n;`.
fn ts_paired_nesting(name: &str, unit: &str, mid: &str, close: &str) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Nesting,
        Wire::Bounded,
        "x = ",
        unit,
        mid,
        close,
        ";\n",
    )
}

/// A named spine shape the wire bound does not separate.
fn ts_spine_unrecorded(
    name: &str,
    pre: &str,
    unit: &str,
    mid: &str,
    close: &str,
    post: &str,
) -> Shape {
    shape(
        name,
        Lang::Ts,
        Limit::Spine,
        Wire::Bounded,
        pre,
        unit,
        mid,
        close,
        post,
    )
}

fn named_shapes() -> Vec<Shape> {
    let mut shapes = construct_shapes();
    shapes.extend(position_shapes());
    shapes
}

fn construct_shapes() -> Vec<Shape> {
    let half_spine = SPINE_LIMIT as usize / 2;
    let half_chain = chain("x", " + ", half_spine);
    let half_tail = format!("{};\n", rep(" + a", half_spine));
    let deepest = 250;
    let deep = 128;
    vec![
        // TypeScript: the expression cycle
        ts_nesting("ts_paren", "x = ", "(", "1", ")", ";\n"),
        ts_nesting("ts_array", "x = ", "[", "1", "]", ";\n"),
        ts_nesting("ts_object", "x = ", "{a: ", "1", "}", ";\n"),
        ts_nesting(
            "ts_object_method",
            "x = ",
            "{m() { return ",
            "1",
            "}}",
            ";\n",
        ),
        ts_nesting("ts_arrow_block", "x = ", "() => {", "1", "}", ";\n"),
        ts_nesting("ts_arrow_concise", "x = ", "() => ", "1", "", ";\n"),
        // a concise or block body is one count however the arrow's head is spelled
        ts_nesting("ts_arrow_bare_concise", "x = ", "a => ", "1", "", ";\n"),
        ts_nesting(
            "ts_arrow_async_bare_concise",
            "x = ",
            "async a => ",
            "1",
            "",
            ";\n",
        ),
        ts_nesting(
            "ts_arrow_async_concise",
            "x = ",
            "async () => ",
            "1",
            "",
            ";\n",
        ),
        ts_nesting(
            "ts_arrow_async_block",
            "x = ",
            "async () => {",
            "1",
            "}",
            ";\n",
        ),
        ts_nesting("ts_member_call", "", "a.f(", "x", ")", ";\n"),
        ts_nesting("ts_call", "", "f(", "x", ")", ";\n"),
        ts_nesting("ts_computed", "", "a[", "x", "]", ";\n"),
        ts_nesting("ts_binary_paren", "x = ", "1 + (", "1", ")", ";\n"),
        // a chain whose left operand is a paren holding the next: the shape of tsc's
        // `parsingDeepParenthensizedExpression.ts`
        shape(
            "ts_left_paren_chain",
            Lang::Ts,
            Limit::Nesting,
            Wire::Bounded,
            "x = ",
            "(",
            "1",
            " + 1)",
            ";\n",
        ),
        // each `**` nests its right operand, and the printer parenthesizes it
        // (`a ** (a ** a)`), a level of the output's nesting the wire does not record
        shape(
            "ts_exponent",
            Lang::Ts,
            Limit::Nesting,
            Wire::Bounded,
            "x = a",
            " ** a",
            "",
            "",
            ";\n",
        ),
        ts_nesting("ts_assign", "", "a = ", "1", "", ";\n"),
        // each conditional's test is a spine link too, which the chain's spine pays before
        // its nesting runs out
        shape(
            "ts_ternary",
            Lang::Ts,
            Limit::Any,
            Wire::Bounded,
            "x = ",
            "a ? b : ",
            "c",
            "",
            ";\n",
        ),
        ts_nesting("ts_unary", "x = ", "!", "1", "", ";\n"),
        // prefix operators the printer parenthesizes over one another (`-(-1)`,
        // `<T>(<T>a)`, `!(/* c */ !a)`, `new new A()()`), each pair a level of the output's
        // nesting the wire does not record
        ts_prefix("ts_unary_minus", "- ", "1"),
        ts_prefix("ts_unary_plus", "+ ", "a"),
        ts_prefix("ts_unary_minus_update", "- ++", "a"),
        ts_prefix("ts_unary_block_comment", "!/*c*/", "a"),
        ts_prefix("ts_unary_line_comment", "!// c\n", "a"),
        ts_prefix("ts_typeof_block_comment", "typeof /*c*/", "a"),
        ts_prefix("ts_angle_assertion", "<T>", "a"),
        ts_prefix("ts_angle_assertion_unary", "<T>-", "a"),
        ts_prefix("ts_unary_angle_assertion", "!<T>", "a"),
        ts_prefix("ts_await_angle_assertion", "await <T>", "a"),
        ts_prefix("ts_new_argless", "new ", "A"),
        ts_prefix("ts_new_argless_comment", "new /*c*/", "A"),
        // the pairs the printer adds at a leaf, at the deepest nesting
        ts_leaf("ts_leaf_new_argless", "new A"),
        ts_leaf("ts_leaf_new_instantiation", "new a.b<T><U>()"),
        ts_leaf("ts_leaf_numeric_member", "1..toString()"),
        ts_leaf("ts_leaf_arrow_parameter", "a => 1"),
        ts_leaf("ts_leaf_async_arrow_parameter", "async a => 1"),
        ts_leaf("ts_leaf_arrow_conditional", "() => a ? b : c"),
        ts_leaf("ts_leaf_spread", "...a || b"),
        ts_leaf("ts_leaf_template_sequence", "`${a, b}`"),
        ts_leaf("ts_leaf_computed_sequence", "a[b, c]"),
        ts_leaf("ts_leaf_instantiations", "f<T><U><V>"),
        ts_leaf("ts_leaf_assignment", "a = b"),
        ts_leaf("ts_leaf_unary_comment", "typeof /*c*/ a"),
        ts_leaf("ts_leaf_called_function", "function () {}()"),
        ts_leaf("ts_leaf_tagged_function", "function () {}`t`"),
        ts_leaf("ts_leaf_function_dividend", "function () {} / 2"),
        ts_leaf("ts_leaf_unary_in", "!a in b"),
        ts_leaf("ts_leaf_logical_await", "a && await b"),
        ts_leaf("ts_leaf_relational_pair", "a < b > c"),
        ts_leaf("ts_leaf_async_assertion", "async as T"),
        ts_leaf("ts_leaf_arrow_decorated_class", "() => @d class {}"),
        ts_leaf("ts_leaf_ternary_branches", "a ? b ?? c : d ?? e"),
        ts_leaf("ts_leaf_ternary_test", "a ?? b ? c : d"),
        ts_leaf("ts_leaf_ternary_assertion_test", "a as T ? b : c"),
        ts_leaf("ts_leaf_arrow_function_return", "(x): (y) => T => x"),
        ts_leaf("ts_leaf_return_binary", "function () { return a && b; }"),
        // the pairs the type printer adds at a leaf, at the deepest tuple nesting
        ts_type_leaf("ts_type_leaf_type_query_array", "typeof a[]"),
        ts_type_leaf("ts_type_leaf_negative_literal_array", "-1[]"),
        ts_type_leaf("ts_type_leaf_optional_union", "A | B?"),
        ts_type_leaf("ts_type_leaf_intersection_member", "A & B | C"),
        ts_type_leaf("ts_type_leaf_readonly_operand", "keyof readonly A[]"),
        ts_type_leaf(
            "ts_type_leaf_constraint",
            "<U extends A extends B ? C : D>() => U",
        ),
        ts_type_leaf(
            "ts_type_leaf_conditional_branch",
            "A extends B ? C extends D ? E : F : G",
        ),
        ts_type_leaf("ts_type_leaf_tuple_union", "A | B, C"),
        // type operators the printer parenthesizes over one another, and a union member
        // it parenthesizes inside each author's pair
        shape(
            "ts_type_readonly",
            Lang::Ts,
            Limit::Nesting,
            Wire::Bounded,
            "type T = ",
            "readonly ",
            "A",
            "[]",
            ";\n",
        ),
        shape(
            "ts_type_member_mix",
            Lang::Ts,
            Limit::Nesting,
            Wire::Bounded,
            "type T = ",
            "A & (B | C & (",
            "D",
            "))",
            ";\n",
        ),
        // sequences and spreads the printer parenthesizes inside one another
        ts_paired_nesting("ts_template_sequence", "`${a, ", "b", "}`"),
        ts_paired_nesting("ts_computed_sequence", "a[b, ", "c", "]"),
        ts_paired_nesting("ts_spread_logical", "[...a || ", "a", "]"),
        ts_nesting("ts_typeof", "x = ", "typeof ", "a", "", ";\n"),
        ts_nesting("ts_new", "x = ", "new ", "A", "()", ";\n"),
        // an argument is its list's level alone, the `new` charging only its callee
        ts_nesting("ts_new_argument", "x = ", "new A(", "a", ")", ";\n"),
        // a parameter property's binding is a level of its own, and its decorators sit on
        // that binding; a plain parameter's decorators sit on the parameter itself
        ts_nesting(
            "ts_parameter_property_default",
            "x = ",
            "class { constructor(private p = ",
            "0",
            ") {} }",
            ";\n",
        ),
        ts_nesting(
            "ts_parameter_property_decorator",
            "x = ",
            "class { constructor(@a(",
            "class {}",
            ") private p) {} }",
            ";\n",
        ),
        ts_nesting(
            "ts_parameter_decorator",
            "x = ",
            "class { constructor(@a(",
            "class {}",
            ") p) {} }",
            ";\n",
        ),
        // an index signature's parameter: its `Identifier` and its annotation
        ts_nesting(
            "ts_index_signature_parameter",
            "type X = ",
            "{ [k: ",
            "B",
            "]: B }",
            ";\n",
        ),
        ts_nesting(
            "ts_class_heritage",
            "x = ",
            "class extends ",
            "A",
            " {}",
            ";\n",
        ),
        // a class heritage that is a call, each `extends` a level and its argument list
        // another
        ts_nesting(
            "ts_class_heritage_call",
            "x = ",
            "class extends f(",
            "A",
            ") {}",
            ";\n",
        ),
        ts_nesting("ts_template", "x = ", "`${", "1", "}`", ";\n"),
        ts_nesting("ts_pattern_array", "const ", "[", "a", "]", " = x;\n"),
        ts_nesting("ts_pattern_object", "const ", "{a: ", "a", "}", " = x;\n"),
        // TypeScript: statements
        ts_nesting("ts_block", "", "{", "x;", "}", "\n"),
        ts_nesting("ts_if", "", "if (a) ", "x;", "", "\n"),
        ts_nesting(
            "ts_else_if",
            "if (a) {}",
            " else if (a) {}",
            " else {}",
            "",
            "\n",
        ),
        ts_nesting("ts_label", "", "a: ", "x;", "", "\n"),
        ts_nesting("ts_function", "", "function f() {", "x;", "}", "\n"),
        ts_nesting("ts_class", "", "class A { m() {", "x;", "} }", "\n"),
        ts_nesting("ts_switch", "", "switch (a) { case 1: ", "x;", "}", "\n"),
        ts_nesting("ts_try", "", "try {", "x;", "} finally {}", "\n"),
        ts_nesting("ts_namespace", "", "namespace A {", "x;", "}", "\n"),
        ts_nesting("ts_namespace_dotted", "namespace A", ".A", " {}", "", "\n"),
        // TypeScript: types
        ts_nesting("ts_type_reference", "type T = ", "Array<", "1", ">", ";\n"),
        ts_nesting("ts_type_function", "type T = ", "() => ", "1", "", ";\n"),
        ts_nesting("ts_type_literal", "type T = ", "{ a: ", "1", " }", ";\n"),
        ts_nesting("ts_type_tuple", "type T = ", "[", "1", "]", ";\n"),
        // each `keyof` over another the printer parenthesizes (`keyof (keyof A)`), a level
        // of the output's nesting the wire does not record
        shape(
            "ts_type_operator",
            Lang::Ts,
            Limit::Nesting,
            Wire::Bounded,
            "type T = ",
            "keyof ",
            "A",
            "",
            ";\n",
        ),
        ts_nesting(
            "ts_type_conditional",
            "type T = ",
            "A extends B ? C : ",
            "D",
            "",
            ";\n",
        ),
        ts_nesting("ts_type_union_paren", "type T = ", "A | (", "A", ")", ";\n"),
        ts_nesting("ts_annotation_arrow", "x = ", "(a: A) => ", "1", "", ";\n"),
        // TypeScript: left spines
        ts_flat("ts_binary_flat", 1, "x = 1", " + 1", ";\n"),
        ts_flat("ts_logical_flat", 1, "x = a", " || a", ";\n"),
        ts_flat("ts_call_chain_flat", SUBSCRIPT, "x = a", ".b()", ";\n"),
        ts_flat("ts_member_flat", SUBSCRIPT, "x = a", ".b", ";\n"),
        ts_flat("ts_computed_flat", SUBSCRIPT, "x = a", "[0]", ";\n"),
        ts_flat("ts_call_flat", REENTRY, "x = a", "()", ";\n"),
        ts_flat("ts_optional_call_flat", REENTRY, "x = a", "?.()", ";\n"),
        ts_flat("ts_non_null_flat", NON_NULL, "x = a", "!", ";\n"),
        // a block comment in each `!`'s gap, which the printer takes on a path of its own
        ts_flat(
            "ts_non_null_block_comment_flat",
            NON_NULL,
            "x = a",
            "!/*c*/",
            ";\n",
        ),
        ts_flat(
            "ts_new_callee_non_null_comment",
            NON_NULL,
            "x = new a",
            "!/*c*/",
            "();\n",
        ),
        // the same run as a template literal's interpolation, plain and tagged: a level of
        // doc nesting per link, which the template printer flattens onto one line whole.
        // The count charges the tag as a tagged link, where the wire holds it as one level
        // of nesting below the floor the spine's calibration reads past, so the tagged one
        // is bounded, not calibrated
        ts_flat(
            "ts_template_non_null_comment",
            NON_NULL,
            "x = `${a",
            "!/*c*/",
            "}`;\n",
        ),
        ts_spine_unrecorded(
            "ts_tagged_template_non_null_comment",
            "x = t`${a",
            "!/*c*/",
            "",
            "",
            "}`;\n",
        ),
        ts_flat("ts_as_flat", 8, "x = a", " as T", ";\n"),
        ts_flat("ts_tagged_flat", 14, "x = a", "``", ";\n"),
        ts_flat("ts_new_callee_flat", SUBSCRIPT, "x = new a", ".b", "();\n"),
        ts_flat("ts_decorator_flat", SUBSCRIPT, "@a", ".b", "\nclass A {}\n"),
        ts_flat("ts_type_array_flat", 6, "type T = A", "[]", ";\n"),
        ts_flat("ts_type_indexed_flat", 6, "type T = A", "[K]", ";\n"),
        ts_flat("ts_qualified_flat", 1, "type T = A", ".B", ";\n"),
        ts_flat("ts_import_equals_flat", 1, "import x = A", ".B", ";\n"),
        // the operator chains the binary-chain printer does not flatten, each link of which
        // it walks by re-entering the expression printer — and parenthesizes, each pair a
        // level of the output's nesting
        ts_paired("ts_equality_flat", "x = a", " == a", ";\n"),
        // an instantiation instantiated again, which the printer parenthesizes
        // (`((f<T>)<T>)<T>`)
        ts_paired("ts_instantiation_flat", "x = f", "<T>", ";\n"),
        ts_paired("ts_shift_flat", "x = a", " << a", ";\n"),
        ts_paired("ts_modulo_flat", "x = a", " % a", ";\n"),
        ts_paired("ts_multiplicative_alternation", "x = a", " * a / a", ";\n"),
        ts_paired("ts_relational_alternation", "x = a", " < a > a", ";\n"),
        ts_paired("ts_logical_mixed", "x = a", " && a || a", ";\n"),
        // an assertion alternating with an operator: the operator's left operand owes a
        // pair, and the assertion's operand is the next operator's chain
        ts_paired(
            "ts_assertion_operator_alternation",
            "x = a",
            " + a as T",
            ";\n",
        ),
        ts_paired(
            "ts_satisfies_operator_alternation",
            "x = a",
            " == a satisfies T",
            ";\n",
        ),
        // runs of `!` and instantiations over a call, each run's call a curried one
        ts_flat("ts_non_null_runs_1", REENTRY, "x = a", "()!", ";\n"),
        ts_flat("ts_non_null_runs_2", REENTRY, "x = a", "()!!", ";\n"),
        ts_flat("ts_non_null_runs_3", REENTRY, "x = a", "()!!!", ";\n"),
        ts_flat("ts_non_null_runs_4", REENTRY, "x = a", "()!!!!", ";\n"),
        ts_spine_unrecorded("ts_instantiation_runs_1", "x = a", "()<T>", "", "", ";\n"),
        ts_spine_unrecorded("ts_instantiation_runs_2", "x = a", "()!<T>", "", "", ";\n"),
        ts_spine_unrecorded("ts_instantiation_runs_3", "x = a", "()!!<T>", "", "", ";\n"),
        ts_spine_unrecorded(
            "ts_instantiation_runs_4",
            "x = a",
            "()!!!<T>",
            "",
            "",
            ";\n",
        ),
        // the `for` header's init, where the printer parenthesizes every `in` binary, so
        // each `in` link is walked by re-entering the expression printer — in the header's
        // own parens, in a declaration's initializer, and in a function nested in either
        ts_paired("ts_for_init_in", "for ((a", " in a", ");;);\n"),
        ts_paired(
            "ts_for_declaration_in",
            "for (let x = (a",
            " in a",
            ");;);\n",
        ),
        ts_paired(
            "ts_for_init_function_in",
            "for (x = function () { return a",
            " in a",
            "; };;);\n",
        ),
        // a chain as a call's argument, a member over a curried call, and calls whose
        // argument is a multi-line template (printed whole), with and without a `!`
        ts_spine_unrecorded("ts_argument_chain", "f(a", " + a", "", "", ");\n"),
        ts_spine_unrecorded(
            "ts_member_over_curried_call",
            "x = a",
            ".p()(y)",
            "",
            "",
            ";\n",
        ),
        ts_spine_unrecorded(
            "ts_template_argument_calls",
            "x = a",
            ".o(`\nx\n`)",
            "",
            "",
            ";\n",
        ),
        ts_spine_unrecorded(
            "ts_template_argument_non_null",
            "x = a",
            ".o(`\nx\n`)!",
            "",
            "",
            ";\n",
        ),
        ts_spine_unrecorded(
            "ts_template_argument_curried",
            "x = a",
            "(`\nx\n`)",
            "",
            "",
            ";\n",
        ),
        // TypeScript: a chain whose leaf holds a chain — one root-to-leaf path through both
        shape(
            "ts_spine_in_spine_leaf",
            Lang::Ts,
            Limit::Spine,
            Wire::Spine(1, 1),
            "x = (a",
            " + a",
            ")",
            " + a",
            ";\n",
        ),
        // nesting and spine on one path share the spine's budget: a chain of half the limit,
        // with as much nesting as the rest of the budget leaves above it, inside its leaf, or
        // in markup around it
        ts_spine_unrecorded(
            "ts_nesting_around_spine",
            "",
            "a.f(",
            &half_chain,
            ")",
            ";\n",
        ),
        ts_spine_unrecorded(
            "ts_spine_around_nesting",
            "x = ",
            "a.f(",
            "a",
            ")",
            &half_tail,
        ),
        ts_spine_unrecorded(
            "ts_spine_around_computed",
            "x = ",
            "a[",
            "a",
            "]",
            &half_tail,
        ),
        ts_spine_unrecorded(
            "ts_arrow_around_spine",
            "x = ",
            "() => {",
            &half_chain,
            "}",
            ";\n",
        ),
        // both counts at once: the deepest nesting either side of a chain whose length is
        // searched, the case the nesting weight on the spine exists for
        ts_spine_unrecorded(
            "ts_spine_around_deepest_leaf",
            &format!("x = {}a{}", rep("a.f(", deepest), rep(")", deepest)),
            " + a",
            "",
            "",
            ";\n",
        ),
        ts_spine_unrecorded(
            "ts_deepest_nesting_around_spine",
            &format!("{}x", rep("a.f(", deepest)),
            " + x",
            "",
            "",
            &format!("{};\n", rep(")", deepest)),
        ),
        ts_spine_unrecorded(
            "ts_spine_around_deep_leaf",
            &format!("x = {}a{}", rep("a.f(", deep), rep(")", deep)),
            " + a",
            "",
            "",
            ";\n",
        ),
        // CSS
        shape(
            "css_rule",
            Lang::Css,
            Limit::Nesting,
            Wire::Nesting,
            "",
            "a { ",
            "color: red;",
            "}",
            "\n",
        ),
        shape(
            "css_atrule",
            Lang::Css,
            Limit::Nesting,
            Wire::Nesting,
            "",
            "@media print { ",
            "a { color: red; }",
            "}",
            "\n",
        ),
        shape(
            "css_selector",
            Lang::Css,
            Limit::Nesting,
            Wire::Nesting,
            "",
            ":is(",
            "a",
            ")",
            " { color: red; }\n",
        ),
        shape(
            "css_value_paren",
            Lang::Css,
            Limit::Nesting,
            Wire::Bounded,
            "a { width: ",
            "calc(",
            "1px",
            ")",
            "; }\n",
        ),
        // the value parser reads an unquoted `url(`'s interior as nested groups, though the
        // lexer's url-token ends at its first `)`: every spelling of it, and the positions a
        // value takes
        css_value(
            "css_url_interior_paren",
            "a { b: url(",
            "(",
            "x",
            ")",
            "); }\n",
        ),
        css_value(
            "css_url_upper_interior_paren",
            "a { b: URL(",
            "(",
            "x",
            ")",
            "); }\n",
        ),
        css_value("css_url_nested_url", "a { b: ", "url(", "x", ")", "; }\n"),
        css_value(
            "css_url_nested_function",
            "a { b: url(",
            "f(",
            "x",
            ")",
            "); }\n",
        ),
        css_value(
            "css_url_comma_list",
            "a { b: x, url(",
            "(",
            "x",
            ")",
            "), y; }\n",
        ),
        css_value(
            "css_url_in_var",
            "a { b: var(--c, url(",
            "(",
            "x",
            ")",
            ")); }\n",
        ),
        css_value(
            "css_url_custom_property",
            "a { --b: url(",
            "(",
            "x",
            ")",
            "); }\n",
        ),
        css_value(
            "css_url_keyframes",
            "@keyframes k { from { b: url(",
            "(",
            "x",
            ")",
            "); } }\n",
        ),
        shape(
            "svelte_style_url_interior_paren",
            Lang::Svelte,
            Limit::Nesting,
            Wire::Bounded,
            "<style>\n\ta { b: url(",
            "(",
            "x",
            ")",
            "); }\n</style>\n",
        ),
        // Svelte
        svelte("svelte_element", "", "<div>", "x", "</div>", "\n"),
        svelte("svelte_component", "", "<A>", "x", "</A>", "\n"),
        svelte("svelte_if", "", "{#if a}", "x", "{/if}", "\n"),
        svelte(
            "svelte_else_if",
            "{#if a}x",
            "{:else if a}x",
            "{/if}",
            "",
            "\n",
        ),
        svelte("svelte_each", "", "{#each a as b}", "x", "{/each}", "\n"),
        svelte("svelte_expression", "<p>{", "[", "1", "]", "}</p>\n"),
        // a `{@const}` holds its binding and init two wire levels below the tag
        svelte(
            "svelte_const_init",
            "{#if a}{@const b = ",
            "[",
            "1",
            "]",
            "}{/if}\n",
        ),
        svelte(
            "svelte_const_annotation",
            "<script lang=\"ts\"></script>\n{#if a}{@const b: ",
            "A<",
            "B",
            ">",
            " = 1}{/if}\n",
        ),
        // the Svelte wire drops grouping parens, which the parse still recurses through
        shape(
            "svelte_expression_paren",
            Lang::Svelte,
            Limit::Nesting,
            Wire::Bounded,
            "<p>{",
            "(",
            "1",
            ")",
            "}</p>\n",
        ),
        svelte(
            "svelte_attribute_expression",
            "<p a={",
            "[",
            "1",
            "]",
            "}></p>\n",
        ),
        svelte(
            "svelte_script",
            "<script>\n\tx = ",
            "[",
            "1",
            "]",
            ";\n</script>\n",
        ),
        svelte(
            "svelte_style",
            "<style>\n\t",
            "a { ",
            "color: red;",
            "}",
            "\n</style>\n",
        ),
        // the document-wide budget: the markup's nesting and an island's share one count
        svelte(
            "svelte_elements_around_expression",
            &format!("{}{{", rep("<div>", 100)),
            "[",
            "1",
            "]",
            &format!("}}{}\n", rep("</div>", 100)),
        ),
        // the pair the Svelte printer writes around an assignment in a template island
        // (`{(a = b)}`), at the markup's deepest nesting
        shape(
            "svelte_elements_around_assignment",
            Lang::Svelte,
            Limit::Nesting,
            Wire::Bounded,
            "",
            "<div>",
            "{a = b}",
            "</div>",
            "\n",
        ),
        shape(
            "svelte_elements_around_spine",
            Lang::Svelte,
            Limit::Spine,
            Wire::Bounded,
            "",
            "<div>",
            &format!("{{{half_chain}}}"),
            "</div>",
            "\n",
        ),
        // an operator chain the binary-chain printer does not flatten, in a template island
        shape(
            "svelte_expression_equality",
            Lang::Svelte,
            Limit::PairedChain,
            Wire::Bounded,
            "<p>{a",
            " == a",
            "",
            "",
            "}</p>\n",
        ),
        shape(
            "svelte_attribute_assertion_alternation",
            Lang::Svelte,
            Limit::PairedChain,
            Wire::Bounded,
            "<script lang=\"ts\"></script>\n<p a={a",
            " + a as T",
            "",
            "",
            "}></p>\n",
        ),
    ]
}

/// A named CSS value shape: its nesting is the value parser's, which the wire does not
/// record (a value is a string on the wire).
fn css_value(name: &str, pre: &str, unit: &str, mid: &str, close: &str, post: &str) -> Shape {
    shape(
        name,
        Lang::Css,
        Limit::Nesting,
        Wire::Bounded,
        pre,
        unit,
        mid,
        close,
        post,
    )
}

/// A named Svelte nesting shape recorded by the wire.
fn svelte(name: &str, pre: &str, unit: &str, mid: &str, close: &str, post: &str) -> Shape {
    shape(
        name,
        Lang::Svelte,
        Limit::Nesting,
        Wire::Nesting,
        pre,
        unit,
        mid,
        close,
        post,
    )
}

/// Every grammar position that holds a construct which can hold the position again — an
/// expression, a statement, a type, a pattern, a class member, a selector, a block —
/// each nested in itself `n` deep (`pre + unit×n + mid + close×n + post`), so the gate's
/// wire bound sees every recursive step the parsers take ([`Wire::Bounded`]: the count
/// must charge at least the wire levels each repetition puts on the path).
const POSITIONS: &[(Lang, &str, &str, &str, &str, &str, &str)] = &[
    // expressions
    (
        Lang::Ts,
        "pos_new_argument",
        "x = ",
        "new A(",
        "a",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_new_computed_callee",
        "x = ",
        "new a[",
        "a",
        "]()",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_optional_computed",
        "x = ",
        "a?.[",
        "b",
        "]",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_optional_call",
        "x = ",
        "a.b.c?.(",
        "b",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_tagged_template_expression",
        "x = ",
        "t`${",
        "a",
        "}`",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_decorated_class_statement",
        "",
        "@a(",
        "class {}",
        ") class A {}",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_conditional_alternate_paren",
        "x = ",
        "a ? b : (",
        "c",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_spread_argument",
        "x = ",
        "f(...",
        "a",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_spread_element",
        "x = ",
        "[...",
        "a",
        "]",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_spread_property",
        "x = ",
        "{...",
        "a",
        "}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_computed_key",
        "x = ",
        "{[",
        "a",
        "]: 1}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_getter",
        "x = ",
        "{get a() { return ",
        "a",
        "; }}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_setter",
        "",
        "x = {set a(v) { ",
        "a;",
        " }}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_generator_method",
        "x = ",
        "{*m() { yield ",
        "a",
        "; }}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_async_method",
        "x = ",
        "{async m() { await ",
        "a",
        "; }}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_default",
        "x = ",
        "(a = ",
        "0",
        ") => 0",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_pattern_default",
        "x = ",
        "({a} = ",
        "0",
        ") => 0",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_pattern_default",
        "x = ",
        "({a = ",
        "0",
        "}) => 0",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_function_body",
        "x = ",
        "function () { return ",
        "a",
        "; }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_function_default",
        "x = ",
        "function (a = ",
        "0",
        ") {}",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_async_arrow",
        "x = ",
        "async () => ",
        "a",
        "",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_async_arrow_parameter",
        "x = ",
        "async (a = ",
        "0",
        ") => 0",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_generic_arrow",
        "x = ",
        "<T>() => ",
        "a",
        "",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_class_property",
        "x = ",
        "class { a = ",
        "b",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_class_computed_key",
        "x = ",
        "class { [",
        "a",
        "]() {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_class_static_block",
        "",
        "x = class { static { ",
        "a;",
        " } }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_class_getter",
        "x = ",
        "class { get a() { return ",
        "a",
        "; } }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_class_expression_method",
        "x = ",
        "f(class { m() { return ",
        "a",
        "; } })",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_class_constructor_default",
        "x = ",
        "class { constructor(a = ",
        "0",
        ") {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_property_default",
        "x = ",
        "class { constructor(private p = ",
        "0",
        ") {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_property_readonly",
        "x = ",
        "class { constructor(readonly p = ",
        "0",
        ") {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_property_override",
        "x = ",
        "class extends B { constructor(override p = ",
        "0",
        ") {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_static_block_decorator",
        "x = ",
        "class { @a(",
        "class {}",
        ") static {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_index_signature_decorator",
        "x = ",
        "class { @a(",
        "class {}",
        ") [k: string]: T }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_property_decorator",
        "x = ",
        "class { constructor(@a(",
        "class {}",
        ") private p) {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_property_decorator_default",
        "x = ",
        "class { constructor(@a private p = ",
        "0",
        ") {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_parameter_property_decorator_with_default",
        "x = ",
        "class { constructor(@a(",
        "class {}",
        ") public readonly p = 0) {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_yield",
        "x = ",
        "function* () { yield ",
        "a",
        "; }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_yield_delegate",
        "x = ",
        "function* () { yield* ",
        "a",
        "; }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_conditional_consequent",
        "x = ",
        "a ? ",
        "a",
        " : b",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_conditional_test",
        "x = ",
        "(",
        "a",
        " ? b : c)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_compound_assignment",
        "",
        "a += ",
        "1",
        "",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_logical_assignment",
        "",
        "a ??= ",
        "1",
        "",
        ";\n",
    ),
    (Lang::Ts, "pos_sequence", "x = ", "(a, ", "b", ")", ";\n"),
    (
        Lang::Ts,
        "pos_sequence_first",
        "x = ",
        "(",
        "a",
        ", b)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_non_null_paren",
        "x = ",
        "(",
        "a",
        ")!",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_import_call",
        "x = ",
        "import(",
        "a",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_import_call_options",
        "x = ",
        "import(a, ",
        "b",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_instantiation",
        "x = ",
        "f<A<",
        "B",
        ">>",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_paren_arrow_body",
        "x = ",
        "() => (",
        "a",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_array_pattern_default",
        "",
        "[a = ",
        "0",
        "] = x",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_array_pattern_rest",
        "",
        "[...",
        "a",
        "]",
        " = x;\n",
    ),
    (
        Lang::Ts,
        "pos_pattern_rest_parameter",
        "x = ",
        "(...{ a = ",
        "0",
        " }) => 0",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_pattern_rest_parameter_array",
        "x = ",
        "(...[{ a = ",
        "0",
        " }]) => 0",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_method_rest_parameter_pattern",
        "x = ",
        "class { m(...{ a = ",
        "0",
        " }) {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_pattern_shorthand_default",
        "x = ",
        "({ a = ",
        "0",
        " } = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_pattern_nested_shorthand_default",
        "x = ",
        "({ a: { b = ",
        "0",
        " } } = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_pattern_array_shorthand_default",
        "x = ",
        "([{ a = ",
        "0",
        " }] = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_pattern_rest_shorthand_default",
        "x = ",
        "([...{ a = ",
        "0",
        " }] = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_pattern_computed_key",
        "x = ",
        "({ [",
        "0",
        "]: a } = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_pattern_member_target",
        "x = ",
        "([a[",
        "0",
        "]] = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_computed_target",
        "x = ",
        "(a[",
        "0",
        "] = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_call_target",
        "x = ",
        "(a(",
        "0",
        ").b = x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_compound_assignment_target",
        "x = ",
        "(a[",
        "0",
        "] += x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_compound_assignment_literal_target",
        "x = ",
        "({ [",
        "0",
        "]: a } += x)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_assignment_statement_pattern",
        "x = ",
        "() => { ({ [",
        "0",
        "]: a } = x); }",
        ";\n",
    ),
    // statements
    (Lang::Ts, "pos_else", "", "if (a) x; else ", "x;", "", "\n"),
    (Lang::Ts, "pos_while", "", "while (a) ", "x;", "", "\n"),
    (
        Lang::Ts,
        "pos_do_while",
        "",
        "do ",
        "x;",
        " while (a);",
        "\n",
    ),
    (Lang::Ts, "pos_for", "", "for (;;) ", "x;", "", "\n"),
    (Lang::Ts, "pos_for_in", "", "for (a in b) ", "x;", "", "\n"),
    (Lang::Ts, "pos_for_of", "", "for (a of b) ", "x;", "", "\n"),
    (
        Lang::Ts,
        "pos_for_await",
        "async function f() {",
        "for await (a of b) ",
        "x;",
        "",
        "}\n",
    ),
    (
        Lang::Ts,
        "pos_catch",
        "",
        "try {} catch { ",
        "x;",
        " }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_catch_binding",
        "",
        "try {} catch (e) { ",
        "x;",
        " }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_catch_pattern_shorthand_default",
        "x = ",
        "() => { try {} catch ({ a = ",
        "0",
        " }) {} }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_declarator_pattern_shorthand_default",
        "x = ",
        "() => { let { a = ",
        "0",
        " } = x; }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_declaration_pattern",
        "x = ",
        "() => { for (let { a = ",
        "0",
        " } = x; ; ); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_in_declaration_pattern",
        "x = ",
        "() => { for (let { a = ",
        "0",
        " } in x); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_of_declaration_pattern",
        "x = ",
        "() => { for (const { a = ",
        "0",
        " } of x); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_of_declaration_computed_key",
        "x = ",
        "() => { for (const { [",
        "0",
        "]: a } of x); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_await_declaration_pattern",
        "x = ",
        "async () => { for await (const { a = ",
        "0",
        " } of x); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_of_assignment_pattern",
        "x = ",
        "() => { for ({ a = ",
        "0",
        " } of x); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_for_init_assignment_pattern",
        "x = ",
        "() => { for ({ [",
        "0",
        "]: a } = x; ; ); }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_finally",
        "",
        "try {} finally { ",
        "x;",
        " }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_switch_default",
        "",
        "switch (a) { default: ",
        "x;",
        " }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_switch_test",
        "",
        "switch (a) { case (() => { ",
        "a;",
        " })(): }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_switch_discriminant",
        "",
        "switch ((() => { ",
        "a;",
        " })()) {}",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_if_test",
        "",
        "if ((() => { ",
        "a;",
        " })()) {}",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_while_test",
        "",
        "while ((() => { ",
        "a;",
        " })()) {}",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_variable_initializer",
        "",
        "let a = () => { ",
        "x;",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_return_argument",
        "",
        "function f() { return ",
        "a",
        "; }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_throw_argument",
        "",
        "function f() { throw ",
        "a",
        "; }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_export_namespace",
        "",
        "export namespace A { ",
        "x;",
        " }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_declare_namespace",
        "declare namespace A { ",
        "namespace A { ",
        "",
        "}",
        " }\n",
    ),
    (
        Lang::Ts,
        "pos_enum_initializer",
        "",
        "enum E { A = (() => { ",
        "x;",
        " })() }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_async_function",
        "",
        "async function f() { ",
        "x;",
        " }",
        "\n",
    ),
    (
        Lang::Ts,
        "pos_generator_function",
        "",
        "function* f() { ",
        "x;",
        " }",
        "\n",
    ),
    // types
    (
        Lang::Ts,
        "pos_type_tuple_named",
        "type X = ",
        "[a: ",
        "A",
        "]",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_tuple_optional",
        "type X = ",
        "[",
        "A",
        "?]",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_tuple_rest",
        "type X = ",
        "[...",
        "A",
        "]",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_intersection_paren",
        "type X = ",
        "A & (",
        "A",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_function_parameter",
        "type X = ",
        "(a: ",
        "A",
        ") => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_function_rest",
        "type X = ",
        "(...a: ",
        "A",
        "[]) => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_function_type_parameter",
        "type X = ",
        "<T extends ",
        "A",
        ">() => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_function_type_default",
        "type X = ",
        "<T = ",
        "A",
        ">() => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_constructor",
        "type X = ",
        "new () => ",
        "A",
        "",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_constructor_parameter",
        "type X = ",
        "new (a: ",
        "A",
        ") => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_signature",
        "type X = ",
        "{ m(): ",
        "A",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_parameter",
        "type X = ",
        "{ m(a: ",
        "A",
        "): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_generic_method_parameter",
        "type X = ",
        "{ m<T>(a: ",
        "A",
        "): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_rest_parameter",
        "type X = ",
        "{ m(...a: ",
        "A",
        "[]): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_pattern_parameter",
        "type X = ",
        "{ m({ a }: ",
        "A",
        "): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_setter_signature_parameter",
        "type X = ",
        "{ set a(v: ",
        "A",
        ") }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_call_signature_parameter",
        "type X = ",
        "{ (a: ",
        "A",
        "): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_construct_signature_parameter",
        "type X = ",
        "{ new (a: ",
        "A",
        "): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_pattern_shorthand_default",
        "type X = ",
        "{ m({ a = x as ",
        "A",
        " }): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_pattern_nested_shorthand_default",
        "type X = ",
        "{ m({ a: { b = x as ",
        "A",
        " } }): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_pattern_array_shorthand_default",
        "type X = ",
        "{ m([{ a = x as ",
        "A",
        " }]): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_pattern_default",
        "type X = ",
        "{ m([a = x as ",
        "A",
        "]): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_pattern_computed_key",
        "type X = ",
        "{ m({ [x as ",
        "A",
        "]: a }): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_rest_pattern",
        "type X = ",
        "{ m(...{ a = x as ",
        "A",
        " }): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_rest_pattern_array",
        "type X = ",
        "{ m(...[{ a = x as ",
        "A",
        " }]): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_method_rest_pattern_annotation",
        "type X = ",
        "{ m(...[a]: ",
        "A",
        "): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_call_signature_pattern_default",
        "type X = ",
        "{ ({ a = x as ",
        "A",
        " }): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_construct_signature_pattern_default",
        "type X = ",
        "{ new ({ a = x as ",
        "A",
        " }): B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_setter_signature_pattern_default",
        "type X = ",
        "{ set s({ a = x as ",
        "A",
        " }) }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_function_pattern_shorthand_default",
        "type X = ",
        "({ a = x as ",
        "A",
        " }) => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_function_rest_pattern",
        "type X = ",
        "(...{ a = x as ",
        "A",
        " }) => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_constructor_pattern_shorthand_default",
        "type X = ",
        "new ({ a = x as ",
        "A",
        " }) => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_constructor_rest_pattern",
        "type X = ",
        "new (...{ a = x as ",
        "A",
        " }) => B",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_interface_method_parameter",
        "interface I { m(a: ",
        "{ m(a: ",
        "B",
        "): B }",
        "): B }\n",
    ),
    (
        Lang::Ts,
        "pos_interface_call_signature_parameter",
        "interface I { (a: ",
        "{ (a: ",
        "B",
        "): B }",
        "): B }\n",
    ),
    (
        Lang::Ts,
        "pos_interface_construct_signature_parameter",
        "interface I { new (a: ",
        "{ new (a: ",
        "B",
        "): B }",
        "): B }\n",
    ),
    // a class member's or a function's signature holds no class or function again, so
    // these are their outer signature over the type-literal recursion
    (
        Lang::Ts,
        "pos_class_method_overload_parameter",
        "class A { m(a: ",
        "{ m(a: ",
        "B",
        "): B }",
        "): B; m() {} }\n",
    ),
    (
        Lang::Ts,
        "pos_class_abstract_method_parameter",
        "abstract class A { abstract m(a: ",
        "{ m(a: ",
        "B",
        "): B }",
        "): B; }\n",
    ),
    (
        Lang::Ts,
        "pos_function_overload_parameter",
        "function f(a: ",
        "{ m(a: ",
        "B",
        "): B }",
        "): B;\nfunction f() {}\n",
    ),
    (
        Lang::Ts,
        "pos_declare_function_parameter",
        "declare function f(a: ",
        "{ m(a: ",
        "B",
        "): B }",
        "): B;\n",
    ),
    (
        Lang::Ts,
        "pos_type_call_signature",
        "type X = ",
        "{ (): ",
        "A",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_construct_signature",
        "type X = ",
        "{ new (): ",
        "A",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_index_signature",
        "type X = ",
        "{ [k: string]: ",
        "A",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_index_signature_parameter",
        "type X = ",
        "{ [k: ",
        "B",
        "]: B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_readonly_index_signature_parameter",
        "type X = ",
        "{ readonly [k: ",
        "B",
        "]: B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_interface_index_signature_parameter",
        "interface I { [k: ",
        "{ [k: ",
        "B",
        "]: B }",
        "]: B }\n",
    ),
    // a class's index signature holds no class again, so the class forms are its outer
    // signature over the type-literal recursion
    (
        Lang::Ts,
        "pos_class_index_signature_parameter",
        "class A { [k: ",
        "{ [k: ",
        "B",
        "]: B }",
        "]: B }\n",
    ),
    (
        Lang::Ts,
        "pos_class_static_index_signature_parameter",
        "class A { static [k: ",
        "{ [k: ",
        "B",
        "]: B }",
        "]: B }\n",
    ),
    (
        Lang::Ts,
        "pos_class_readonly_index_signature_parameter",
        "class A { readonly [k: ",
        "{ [k: ",
        "B",
        "]: B }",
        "]: B }\n",
    ),
    (
        Lang::Ts,
        "pos_type_getter_signature",
        "type X = ",
        "{ get a(): ",
        "A",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_mapped",
        "type X = ",
        "{ [K in keyof ",
        "A",
        "]: B }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_mapped_value",
        "type X = ",
        "{ [K in A]: ",
        "B",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_mapped_as",
        "type X = ",
        "{ [K in A as ",
        "B",
        "]: C }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_conditional_true",
        "type X = ",
        "A extends B ? ",
        "C",
        " : D",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_conditional_check",
        "type X = ",
        "(",
        "A",
        " extends B ? C : D)",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_infer_constraint",
        "type X = ",
        "A extends [infer U extends ",
        "B",
        "] ? C : D",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_indexed_index",
        "type X = ",
        "A[",
        "B",
        "]",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_template_literal",
        "type X = ",
        "`a${",
        "B",
        "}`",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_paren",
        "type X = ",
        "(",
        "A",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_import_argument",
        "type X = ",
        "import('a').B<",
        "C",
        ">",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_query_argument",
        "type X = ",
        "typeof a<",
        "B",
        ">",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_predicate",
        "type X = ",
        "(a: A) => a is ",
        "B",
        "",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_union_member",
        "type X = ",
        "A | (",
        "B",
        ")",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_type_optional_member",
        "type X = ",
        "{ a?: ",
        "A",
        " }",
        ";\n",
    ),
    (
        Lang::Ts,
        "pos_interface_extends_argument",
        "interface I extends ",
        "A<",
        "B",
        ">",
        " {}\n",
    ),
    // Svelte
    (
        Lang::Svelte,
        "pos_svelte_await",
        "",
        "{#await a}",
        "x",
        "{/await}",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_await_then",
        "",
        "{#await a then b}",
        "x",
        "{/await}",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_key",
        "",
        "{#key a}",
        "x",
        "{/key}",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_snippet",
        "",
        "{#snippet s()}",
        "x",
        "{/snippet}",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_else",
        "",
        "{#if a}{:else}",
        "x",
        "{/if}",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_each_else",
        "",
        "{#each a as b}{:else}",
        "x",
        "{/each}",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_svelte_element",
        "",
        "<svelte:element this={a}>",
        "x",
        "</svelte:element>",
        "\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_attribute_value",
        "<p a=\"{",
        "[",
        "1",
        "]",
        "}\"></p>\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_directive",
        "<p on:a={",
        "[",
        "1",
        "]",
        "}></p>\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_html",
        "<p>{@html ",
        "[",
        "1",
        "]",
        "}</p>\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_const_init",
        "<script lang=\"ts\"></script>\n{#if a}{@const b = ",
        "[",
        "1",
        "]",
        "}{/if}\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_const_annotation",
        "<script lang=\"ts\"></script>\n{#if a}{@const b: ",
        "A<",
        "B",
        ">",
        " = 1}{/if}\n",
    ),
    (
        Lang::Svelte,
        "pos_svelte_const_pattern",
        "<script lang=\"ts\"></script>\n{#if a}{@const ",
        "[",
        "b",
        "]",
        " = 1}{/if}\n",
    ),
    // CSS
    (Lang::Css, "pos_css_not", "", ":not(", "a", ")", " {}\n"),
    (Lang::Css, "pos_css_has", "", ":has(", "a", ")", " {}\n"),
    (
        Lang::Css,
        "pos_css_nth_child_of",
        "",
        ":nth-child(2n of ",
        "a",
        ")",
        " {}\n",
    ),
    (
        Lang::Css,
        "pos_css_nested_rule",
        "",
        "a { & b { ",
        "c: d;",
        "} }",
        "\n",
    ),
    (
        Lang::Css,
        "pos_css_layer",
        "",
        "@layer a { ",
        "b { c: d; }",
        "}",
        "\n",
    ),
    (
        Lang::Css,
        "pos_css_container",
        "",
        "@container (a) { ",
        "b { c: d; }",
        "}",
        "\n",
    ),
    (
        Lang::Css,
        "pos_css_value_var",
        "a { b: ",
        "var(--c, ",
        "d",
        ")",
        "; }\n",
    ),
];

fn position_shapes() -> Vec<Shape> {
    POSITIONS
        .iter()
        .map(|&(lang, name, pre, unit, mid, close, post)| {
            shape(
                name,
                lang,
                Limit::Any,
                Wire::Bounded,
                pre,
                unit,
                mid,
                close,
                post,
            )
        })
        .collect()
}
