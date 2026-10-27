//! The work legs of `build_fanout_audit`: build multiplicity, doc nodes per byte, and the
//! growth of the work counters — each graded on the deterministic counts
//! `tsv_lang::fanout_probe` takes, over the depth-limit gate's shapes, the known-bug shapes
//! and the flat runs, and held to a ratchet of the shapes that fail today.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use tsv_cli::cli::input::ParserType;
use tsv_lang::fanout_probe::{self, BUDGET_EXCEEDED, Budget, Tally};

use super::doc_node_count;
use crate::audit::panic_hook::panic_message;
use crate::audit::ratchet::SnapshotKey;
use crate::cli::commands::profile::lang_token;
use crate::depth_shapes::{Lang, Limit, Scope, Shape, Wire, shapes};

/// The most times any one AST node may be built in one format. The corpus's worst file
/// builds one node 9 times; a subtree rebuilt per layout candidate reaches hundreds at
/// ten levels.
pub(super) const MAX_BUILDS_PER_NODE: u32 = 32;

/// The most builds per distinct built node, over a whole format. The corpus's worst file
/// builds 1.73 per node (its 99th percentile 1.51).
pub(super) const MAX_BUILDS_RATIO: u64 = 4;

/// Doc nodes per source byte allowed at the graded depth, and the constant beside it (the
/// prelude and a small input's fixed frame). The corpus's densest file builds 0.69 nodes
/// per byte.
pub(super) const MAX_NODES_PER_BYTE: usize = 8;
pub(super) const NODES_SLACK: usize = 2_000;

/// The depth the multiplicity and density legs grade a nesting shape at.
pub(super) const GRADED_DEPTH: usize = 10;

/// The depths a nesting shape's work is swept over, and the growth exponent its counters
/// may reach across the sweep's last doubling: a walk or scan over each level's subtree
/// from each level is quadratic, which passes; anything faster compounds.
pub(super) const NESTING_SWEEP: [usize; 3] = [8, 16, 32];
pub(super) const MAX_NESTING_EXPONENT: f64 = 2.0;

/// The link counts a chain shape is swept over: past the nesting depths, so a quadratic too
/// small to clear [`WORK_FLOOR`] at 32 links (under 4 units per link squared) clears it at the
/// top, and the last doubling sits nearer the asymptote. A count the parser refuses ends the
/// sweep, as a depth does.
pub(super) const CHAIN_SWEEP: [usize; 5] = [8, 16, 32, 64, 128];

/// The run lengths a flat shape is swept over, and the growth exponent allowed across the
/// last doubling: a sibling run's work must stay linear, with room for a log factor.
pub(super) const FLAT_SWEEP: [usize; 3] = [1_000, 2_000, 4_000];
pub(super) const MAX_FLAT_EXPONENT: f64 = 1.3;

/// How far over its limit a measured exponent may sit. A quadratic's lower-order terms put
/// its exponent over one doubling a little above 2 — the nested templates' walks and the
/// mixed-precedence chains' scans measure 2.02–2.11 over 16→32 and converge on 2 further
/// out (4.02–4.07× per doubling at 64→128) — where a cubic measures 3 and a rebuild per
/// layout candidate far more.
const EXPONENT_TOLERANCE: f64 = 0.25;

/// A counter below this at the sweep's top point is too small to fail on its exponent: a
/// constant's worth of work is not a growth curve.
const WORK_FLOOR: u64 = 4_096;

/// What one format may do before the probe stops it. Far above anything a linear format of
/// these inputs does (a few thousand builds, tens of thousands of visits), far below what
/// takes the machine's memory or minutes.
const BUDGET: Budget = Budget {
    builds: 250_000,
    walks: 20_000_000,
    scans: 20_000_000,
    predicates: 20_000_000,
    nodes: NODE_BUDGET,
};

/// The most doc nodes one format may allocate, in this leg and the doc-node growth leg alike:
/// the bound on a doc's memory whatever printer builds it (a `DocNode` is 24 bytes, so this
/// many is under a hundred megabytes of node store). The largest doc a shape no bug is known
/// in builds is about 52,000 nodes (a flat Svelte block run at 4,000); the largest a known
/// bug's sweep reaches without being stopped is about 8,000 (K285's tag run at 4,000). A real
/// file at the corpus's densest (0.69 nodes per byte) reaches it only past 5 MB, ten times
/// the snapshot's largest file.
pub(super) const NODE_BUDGET: u64 = 4_000_000;

/// How a shape's input grows: nested `n` deep, `n` links of one chain, `n` siblings in a
/// row, or not at all (an unminimized reproducer, graded as written on the multiplicity and
/// density legs alone).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Growth {
    Nesting,
    /// A depth shape that grows a chain's links, not its nesting: swept over
    /// [`CHAIN_SWEEP`], graded on the flat exponent, since a link is a sibling of the one
    /// before it on the printer's side and its work must stay linear.
    Chain,
    Flat,
    Fixed,
}

/// One input the legs grade: a shape, how it grows, and — for a shape written for a known
/// bug — the bug it reproduces.
pub(super) struct Input {
    pub(super) shape: Shape,
    pub(super) growth: Growth,
    /// Whether the parser must accept it (a hand-written shape); a generated depth shape the
    /// parser rejects is skipped.
    pub(super) required: bool,
}

/// The leg a finding fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Leg {
    Multiplicity,
    Density,
    Work,
    Panic,
}

impl Leg {
    fn name(self) -> &'static str {
        match self {
            Leg::Multiplicity => "multiplicity",
            Leg::Density => "density",
            Leg::Work => "work",
            Leg::Panic => "panic",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [Leg::Multiplicity, Leg::Density, Leg::Work, Leg::Panic]
            .into_iter()
            .find(|leg| leg.name() == s)
    }
}

/// A ratchet key: the leg and the shape that fails it, with the known bug that explains it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct FanoutKey {
    pub(super) leg: Leg,
    pub(super) shape: String,
    /// The bug number ([`known_cause`], or a [`KnownFamily`]'s); `-` when none is known,
    /// which never pins.
    pub(super) cause: String,
}

impl SnapshotKey for FanoutKey {
    fn to_line(&self) -> String {
        format!("{}\t{}\t{}", self.leg.name(), self.shape, self.cause)
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let leg = Leg::parse(cols.next()?)?;
        let shape = cols.next()?.to_string();
        let cause = cols.next()?.to_string();
        cols.next().is_none().then_some(Self { leg, shape, cause })
    }

    /// A finding pins only with a known bug beside it — a NEW blowup fails until someone
    /// names it ([`KNOWN_CAUSES`], [`KNOWN_FAMILIES`]) — and a panic never does.
    fn is_pinnable(&self) -> bool {
        self.leg != Leg::Panic && self.cause != NO_CAUSE
    }
}

const NO_CAUSE: &str = "-";

/// The bugs the known failures reproduce, by shape name — a pattern ending in `*` matches a
/// prefix, any other the whole name. A shape written for a bug carries its number in its
/// name; any other shape that fails is listed by its full name. Each later fix removes its
/// entries (and the ratchet's lines with them).
const KNOWN_CAUSES: &[(&str, &str)] = &[("k285_*", "K285"), ("k289_*", "K289")];

fn known_cause(shape: &str) -> &'static str {
    KNOWN_CAUSES
        .iter()
        .find(|(pattern, _)| {
            pattern
                .strip_suffix('*')
                .map_or(shape == *pattern, |prefix| shape.starts_with(prefix))
        })
        .map_or(NO_CAUSE, |&(_, cause)| cause)
}

/// A known bug the generated sweep reproduces in many shapes at once, pinned as one ratchet
/// line (its `label` in the shape column) instead of a line per member. It explains one thing
/// only: the work-leg exponent of one `counter`, on a shape of one `growth` whose name holds
/// one of its `needles`, up to `max_exponent`. Every other finding on a member — another
/// counter, a steeper exponent, a stopped run, a multiplicity, density or panic finding — keys
/// on the shape's own name and cause, and fails as NEW. A new bug of the same signature (the
/// same counter, quadratic, on a shape the needles name) is indistinguishable from the known
/// one and joins the family silently.
struct KnownFamily {
    label: &'static str,
    needles: &'static [&'static str],
    growth: Growth,
    counter: &'static str,
    max_exponent: f64,
    cause: &'static str,
}

const KNOWN_FAMILIES: &[KnownFamily] = &[
    // a chain link holding a `<` — a less-than, or the shift the type grammar re-reads as
    // two — sends the parser's type-argument lookahead over the rest of the chain, so the
    // scans grow as n^2 over the links
    KnownFamily {
        label: "chain links holding `<` or `<<` (scans, n^2)",
        needles: &[" < ", "<<"],
        growth: Growth::Chain,
        counter: "scans",
        max_exponent: MAX_NESTING_EXPONENT + EXPONENT_TOLERANCE,
        cause: "K73",
    },
];

/// The family that explains `counter` growing as n^`exponent` on `input`, if one does.
fn known_family(input: &Input, counter: &str, exponent: f64) -> Option<&'static KnownFamily> {
    KNOWN_FAMILIES.iter().find(|family| {
        family.growth == input.growth
            && family.counter == counter
            && exponent <= family.max_exponent
            && family
                .needles
                .iter()
                .any(|needle| input.shape.name.contains(needle))
    })
}

/// One format's counts.
pub(super) struct Measure {
    pub(super) bytes: usize,
    pub(super) doc_nodes: usize,
    pub(super) tally: Tally,
}

impl Measure {
    /// The counters the work leg grades, named.
    fn counters(&self) -> [(&'static str, u64); 5] {
        [
            ("doc_nodes", self.doc_nodes as u64),
            ("builds", self.tally.total_builds()),
            ("walks", self.tally.walks),
            ("scans", self.tally.scans),
            ("predicates", self.tally.predicates),
        ]
    }
}

/// How one armed format ended.
pub(super) enum Outcome {
    Measured(Measure),
    /// The parser rejected the input.
    Refused(String),
    /// The probe stopped it past its [`BUDGET`].
    OverBudget,
    /// The formatter panicked.
    Panicked(String),
}

/// Format `source` with the counters armed, under [`BUDGET`].
pub(super) fn measure(source: &str, parser: ParserType) -> Outcome {
    fanout_probe::arm(BUDGET);
    let result = catch_unwind(AssertUnwindSafe(|| doc_node_count(source, parser)));
    let tally = fanout_probe::disarm();
    match result {
        Ok(Ok(doc_nodes)) => Outcome::Measured(Measure {
            bytes: source.len(),
            doc_nodes,
            tally,
        }),
        Ok(Err(e)) => Outcome::Refused(e),
        Err(payload) => {
            let message = panic_message(&*payload);
            if message == BUDGET_EXCEEDED {
                Outcome::OverBudget
            } else {
                Outcome::Panicked(message.to_string())
            }
        }
    }
}

/// What the multiplicity and density legs read of one format, for the report.
pub(super) struct Point {
    pub(super) size: usize,
    pub(super) bytes: usize,
    pub(super) doc_nodes: usize,
    pub(super) built_nodes: usize,
    pub(super) total_builds: u64,
    pub(super) max_builds: u32,
}

/// One finding, with what the report says about it.
pub(super) struct Finding {
    pub(super) key: FanoutKey,
    pub(super) detail: String,
}

/// Grade one format on the multiplicity and density legs; `shape` names it in the findings.
pub(super) fn grade_point(
    shape: &str,
    size: usize,
    outcome: &Outcome,
    findings: &mut Vec<Finding>,
) -> Option<Point> {
    let mut fail = |leg: Leg, detail: String| {
        findings.push(Finding {
            key: FanoutKey {
                leg,
                shape: shape.to_string(),
                cause: known_cause(shape).to_string(),
            },
            detail,
        });
    };
    match outcome {
        Outcome::Measured(m) => {
            let built_nodes = m.tally.builds.len();
            let total_builds = m.tally.total_builds();
            let max_builds = m.tally.max_builds().map_or(0, |(_, n)| n);
            if max_builds > MAX_BUILDS_PER_NODE
                || total_builds > MAX_BUILDS_RATIO * built_nodes as u64
            {
                fail(
                    Leg::Multiplicity,
                    format!(
                        "n={size}: one node built {max_builds}× (limit {MAX_BUILDS_PER_NODE}), \
                         {total_builds} builds of {built_nodes} nodes (limit \
                         {MAX_BUILDS_RATIO}×)"
                    ),
                );
            }
            let node_limit = MAX_NODES_PER_BYTE * m.bytes + NODES_SLACK;
            if m.doc_nodes > node_limit {
                fail(
                    Leg::Density,
                    format!(
                        "n={size}: {} doc nodes for {} bytes (limit {node_limit})",
                        m.doc_nodes, m.bytes
                    ),
                );
            }
            Some(Point {
                size,
                bytes: m.bytes,
                doc_nodes: m.doc_nodes,
                built_nodes,
                total_builds,
                max_builds,
            })
        }
        Outcome::OverBudget => {
            let detail = format!("n={size}: stopped past the work budget");
            fail(Leg::Multiplicity, detail.clone());
            fail(Leg::Density, detail);
            None
        }
        Outcome::Panicked(message) => {
            fail(Leg::Panic, format!("n={size}: {message}"));
            None
        }
        Outcome::Refused(_) => None,
    }
}

/// The depth-limit gate's shapes for `scope` (its named shapes, its grammar positions and
/// its generated sweep), as nesting inputs — the same generator, so the two gates grade one
/// catalog.
pub(super) fn depth_inputs(scope: Scope) -> impl Iterator<Item = Input> {
    shapes(scope).iter().map(|s| Input {
        growth: depth_growth(s),
        shape: Shape {
            name: s.name.replace('\t', "\\t"),
            lang: s.lang,
            limit: s.limit,
            wire: s.wire,
            pre: s.pre.clone(),
            unit: s.unit.clone(),
            mid: s.mid.clone(),
            close: s.close.clone(),
            post: s.post.clone(),
        },
        required: false,
    })
}

/// How a depth shape grows: a shape a spine limit may end whose unit has no closer is a run
/// of chain links ([`Growth::Chain`]); any other nests.
fn depth_growth(shape: &Shape) -> Growth {
    let spine = matches!(shape.limit, Limit::Spine | Limit::Either);
    if spine && shape.close.is_empty() {
        Growth::Chain
    } else {
        Growth::Nesting
    }
}

fn hand(name: &str, lang: Lang, growth: Growth, parts: [&str; 5]) -> Input {
    let [pre, unit, mid, close, post] = parts;
    Input {
        shape: Shape {
            name: name.to_string(),
            lang,
            limit: Limit::Any,
            wire: Wire::Bounded,
            pre: pre.to_string(),
            unit: unit.to_string(),
            mid: mid.to_string(),
            close: close.to_string(),
            post: post.to_string(),
        },
        growth,
        required: true,
    }
}

/// The shapes written for the known blowups, one or more per bug, named by it.
pub(super) fn bug_inputs() -> Vec<Input> {
    use Growth::{Flat, Nesting};
    use Lang::{Svelte, Ts};
    vec![
        // a conditional type nested in a parenthesized check type
        hand(
            "k282_paren_conditional_type",
            Ts,
            Nesting,
            ["type T = ", "(", "A", " extends B ? C : D)", ";\n"],
        ),
        // the same nest behind a doubled shell
        hand(
            "k282_double_paren_conditional_type",
            Ts,
            Nesting,
            ["type T = ", "((", "A", " extends B ? C : D))", ";\n"],
        ),
        // the same nest with a block comment trailing the innermost check type
        hand(
            "k282_paren_conditional_type_block_comment",
            Ts,
            Nesting,
            ["type T = ", "(", "A /*c*/", " extends B ? C : D)", ";\n"],
        ),
        // the same nest with a line comment leading the innermost check type
        hand(
            "k282_paren_conditional_type_line_comment",
            Ts,
            Nesting,
            ["type T = ", "(", "// c\nA", " extends B ? C : D)", ";\n"],
        ),
        // a parenthesized conditional as an array element, nested in its own check type
        hand(
            "k282_array_conditional_type",
            Ts,
            Nesting,
            ["type T = ", "(", "A", " extends B ? C : D)[]", ";\n"],
        ),
        // a parenthesized conditional as an indexed access object, nested in its own check
        // type
        hand(
            "k282_indexed_conditional_type",
            Ts,
            Nesting,
            ["type T = ", "(", "A", " extends B ? C : D)[K]", ";\n"],
        ),
        // a parenthesized conditional as an intersection's first member, nested in its own
        // check type
        hand(
            "k282_intersection_conditional_type",
            Ts,
            Nesting,
            ["type T = ", "(", "A", " extends B ? C : D) & E", ";\n"],
        ),
        // an intersection as a parenthesized conditional's check type, nested in its own
        // first member
        hand(
            "k282_conditional_intersection_check",
            Ts,
            Nesting,
            ["type T = ", "(", "A", " & E extends B ? C : D)", ";\n"],
        ),
        // a conditional nested in a consequent
        hand(
            "k283_consequent_ternary",
            Ts,
            Nesting,
            ["x = ", "a ? ", "1", " : b", ";\n"],
        ),
        // a callback nested in a member chain's call
        hand(
            "k284_member_chain_callback",
            Ts,
            Nesting,
            ["x = ", "a.b().c(() => ", "a", ")", ";\n"],
        ),
        // a run of sibling Svelte tags
        hand(
            "k285_svelte_tag_run",
            Svelte,
            Flat,
            ["", "{a}", "", "", "\n"],
        ),
        // a run of statement-leading block comments
        hand(
            "k285_leading_block_comment_run",
            Ts,
            Flat,
            ["", "/* c */ ", "a;", "", "\n"],
        ),
        // a tag, a line comment, then a member call whose argument is a multi-line template
        hand(
            "k286_tag_comment_template_call",
            Ts,
            Nesting,
            ["x = a", "`t` // c\n.o(`\nx\n`)", "", "", ";\n"],
        ),
        // a non-null member after an authored pair, as a binary operand
        hand(
            "k289_nonnull_member_operand",
            Ts,
            Nesting,
            ["x = ", "a == (", "a", ")!.b", ";\n"],
        ),
        // a commented pair followed by an instantiation
        hand(
            "k289_commented_instantiation",
            Ts,
            Nesting,
            ["x = ", "(/*c*/(", "a", ")<T>)", ";\n"],
        ),
        // a run of angle-bracket casts over a commented template holding IIFE `if`s
        hand(
            "k289_cast_run_template",
            Ts,
            Nesting,
            [
                "x = `${(/*c*/",
                "<T>",
                "`${`${(/*c*/(() => { if ((() => { if (`${a}`) a; })) a; }))}`}`)}`",
                "",
                ";\n",
            ],
        ),
        // expand-last callbacks behind a commented pair in a template interpolation: the
        // interpolation asks whether the doc holds a deferred run, and it holds none
        hand(
            "k289_commented_callback_template",
            Ts,
            Nesting,
            ["x = `${(/*c*/ ", "f(() => ", "a", ")", ")}`;\n"],
        ),
        // expand-last callbacks in a template interpolation, the innermost body and the
        // interpolation each ending on a line comment: the interpolation reads the run still
        // pending at its end
        hand(
            "k289_line_comment_callback_template",
            Ts,
            Nesting,
            ["x = `${", "f(() => ", "a // c\n", ")", " // d\n}`;\n"],
        ),
        // expand-last callbacks as an `{#each}` expression with no `as`, ending on a block
        // comment: the head asks whether the expression can end on a comment
        hand(
            "k289_each_callback_comment",
            Svelte,
            Nesting,
            ["{#each ", "f(() => ", "a /*c*/", ")", "}x{/each}\n"],
        ),
        // expand-last callbacks behind a commented pair in an attribute value
        hand(
            "k289_attribute_commented_callback",
            Svelte,
            Nesting,
            ["<div a={(/*c*/ ", "f(() => ", "a", ")", ")}></div>\n"],
        ),
        // the unminimized reproducer: assignments nested in authored pairs, each closing on a
        // `!` or an `|| c` (found in a Svelte template, where it took over a gigabyte)
        hand(
            "k289_assign_or_nonnull_repro",
            Ts,
            Growth::Fixed,
            ["x = ", "", K289_ASSIGN_OR_NONNULL, "", ";\n"],
        ),
        // a block-bodied callback first argument with another argument after it
        hand(
            "k293_callback_first_arg",
            Ts,
            Nesting,
            ["", "f(() => { ", "a;", " }, a)", ";\n"],
        ),
    ]
}

/// The K289 reproducer's expression, as found (no smaller form of it reproduces yet).
const K289_ASSIGN_OR_NONNULL: &str = concat!(
    "(a = (((a = (a = ((a = (a = (a = (a = ((a = ((a = ((a = (a = ((a = ((a = ((a = (a = ",
    "((((a = (a = (a = ((a = ((((((a = ((a = (a = (a = (a = (a = (a = (a = ((((a = (a = ((a = ",
    "(a = (a = ((a = ((a = (a = (a = ((a = (a = (((a = ((a = ((a = (a = (((a = ((a = (((a = ",
    "((a = (a = (a = (a = (a = (((a = (a = (((((a = (((a = ((a = ((((((a = (a = (((((((a = (a ",
    "= (((a = (((a = (((a = (((a = (((a = (a = (((a = (a = ((a = (a = (a = (a = ((a = (((a = ",
    "(a = (a = (a = (a = ((a = ((((a = a) || c)!)!)!) || c)!) || c) || c) || c) || c) || ",
    "c)!)!) || c)!) || c) || c) || c) || c)!) || c) || c)!)!) || c) || c)!)!) || c)!)!) || ",
    "c)!)!) || c)!)!) || c)!)!) || c) || c)!)!)!)!)!)!) || c) || c)!)!)!)!)!) || c)!) || ",
    "c)!)!) || c)!)!)!)!) || c) || c)!)!) || c) || c) || c) || c) || c)!) || c)!)!) || c)!) ",
    "|| c)!)!) || c) || c)!) || c)!) || c)!)!) || c) || c)!) || c) || c) || c)!) || c)!) || ",
    "c) || c) || c)!) || c) || c)!)!)!) || c) || c) || c) || c) || c) || c) || c)!) || ",
    "c)!)!)!)!)!) || c)!) || c) || c) || c)!)!)!) || c) || c)!) || c)!) || c)!) || c) || c)!) ",
    "|| c)!) || c)!) || c) || c) || c) || c)!) || c) || c)!)!) || c",
);

/// Flat sibling runs no bug is known in: the work leg's linear baseline, one per kind of
/// list a printer walks.
pub(super) fn flat_inputs() -> Vec<Input> {
    use Growth::Flat;
    use Lang::{Css, Svelte, Ts};
    vec![
        hand("flat_ts_statements", Ts, Flat, ["", "a;\n", "", "", ""]),
        hand(
            "flat_ts_line_comment_statements",
            Ts,
            Flat,
            ["", "// c\na;\n", "", "", ""],
        ),
        hand("flat_ts_array", Ts, Flat, ["x = [", "a, ", "a", "", "];\n"]),
        hand(
            "flat_ts_object",
            Ts,
            Flat,
            ["x = {", "a: 1, ", "b: 1", "", "};\n"],
        ),
        hand(
            "flat_ts_call_args",
            Ts,
            Flat,
            ["f(", "a, ", "a", "", ");\n"],
        ),
        hand("flat_ts_binary", Ts, Flat, ["x = a", " + a", "", "", ";\n"]),
        hand(
            "flat_ts_union",
            Ts,
            Flat,
            ["type T = A", " | A", "", "", ";\n"],
        ),
        hand(
            "flat_ts_template",
            Ts,
            Flat,
            ["x = `", "${a}", "", "", "`;\n"],
        ),
        hand(
            "flat_ts_class_members",
            Ts,
            Flat,
            ["class A {\n", "a = 1;\n", "", "", "}\n"],
        ),
        hand(
            "flat_svelte_elements",
            Svelte,
            Flat,
            ["", "<p>a</p>\n", "", "", ""],
        ),
        hand(
            "flat_svelte_if_blocks",
            Svelte,
            Flat,
            ["", "{#if a}a{/if}\n", "", "", ""],
        ),
        hand(
            "flat_svelte_attributes",
            Svelte,
            Flat,
            ["<div ", "a=\"b\" ", "", "", "></div>\n"],
        ),
        hand(
            "flat_svelte_text",
            Svelte,
            Flat,
            ["<p>", "word ", "", "", "</p>\n"],
        ),
        hand(
            "flat_css_rules",
            Css,
            Flat,
            ["", "a { b: c; }\n", "", "", ""],
        ),
        hand(
            "flat_css_declarations",
            Css,
            Flat,
            ["a { ", "b: c; ", "", "", "}\n"],
        ),
        hand(
            "flat_css_selectors",
            Css,
            Flat,
            ["", "a, ", "b", "", " { c: d; }\n"],
        ),
    ]
}

pub(super) fn parser_of(lang: Lang) -> ParserType {
    match lang {
        Lang::Ts => ParserType::TypeScript,
        Lang::Css => ParserType::Css,
        Lang::Svelte => ParserType::Svelte,
    }
}

/// What the legs found for one input, for the report.
pub(super) struct InputResult {
    pub(super) name: String,
    pub(super) parser: ParserType,
    pub(super) growth: Growth,
    pub(super) graded: Option<Point>,
    /// The sweep's points (size, counters), as far as it got.
    pub(super) sweep: Vec<(usize, [(&'static str, u64); 5])>,
    /// The worst counter's growth exponent across the sweep, with its name.
    pub(super) exponent: Option<(&'static str, f64)>,
    /// Why the input was not graded at all: the parser rejected it.
    pub(super) skipped: Option<String>,
}

/// Grade one input on every leg.
pub(super) fn grade_input(input: &Input, findings: &mut Vec<Finding>) -> InputResult {
    let shape = &input.shape;
    let parser = parser_of(shape.lang);
    let (graded_size, sweep, limit): (usize, &[usize], f64) = match input.growth {
        Growth::Nesting => (GRADED_DEPTH, &NESTING_SWEEP, MAX_NESTING_EXPONENT),
        Growth::Chain => (GRADED_DEPTH, &CHAIN_SWEEP, MAX_FLAT_EXPONENT),
        Growth::Flat => (FLAT_SWEEP[0], &FLAT_SWEEP, MAX_FLAT_EXPONENT),
        Growth::Fixed => (0, &[], 0.0),
    };
    let mut result = InputResult {
        name: shape.name.clone(),
        parser,
        growth: input.growth,
        graded: None,
        sweep: Vec::new(),
        exponent: None,
        skipped: None,
    };
    let outcome = measure(&shape.make(graded_size), parser);
    if let Outcome::Refused(e) = &outcome {
        result.skipped = Some(e.lines().next().unwrap_or_default().to_string());
        return result;
    }
    result.graded = grade_point(&shape.name, graded_size, &outcome, findings);

    let graded_panicked = matches!(outcome, Outcome::Panicked(_));
    let own = |leg: Leg, detail: String| Finding {
        key: FanoutKey {
            leg,
            shape: shape.name.clone(),
            cause: known_cause(&shape.name).to_string(),
        },
        detail,
    };
    for &size in sweep {
        match measure(&shape.make(size), parser) {
            Outcome::Measured(m) => result.sweep.push((size, m.counters())),
            Outcome::Refused(_) => break,
            Outcome::OverBudget => {
                findings.push(own(
                    Leg::Work,
                    format!("n={size}: stopped past the work budget"),
                ));
                return result;
            }
            // a panic the graded point did not reach is a finding of its own; one it did
            // reach is already reported
            Outcome::Panicked(message) => {
                if !graded_panicked {
                    findings.push(own(Leg::Panic, format!("n={size}: {message}")));
                }
                return result;
            }
        }
    }
    // the last doubling: the nearest the sweep comes to the growth's asymptote
    let [.., lo, hi] = result.sweep.as_slice() else {
        return result;
    };
    if lo.0 == hi.0 {
        return result;
    }
    // counts and sizes are far below 2^52, so the conversions are exact
    #[expect(clippy::cast_precision_loss)]
    let span = (hi.0 as f64 / lo.0 as f64).ln();
    let mut worst: Option<(&'static str, f64, u64)> = None;
    for (&(name, lo_count), &(_, hi_count)) in lo.1.iter().zip(hi.1.iter()) {
        #[expect(clippy::cast_precision_loss)]
        let exponent = ((hi_count + 1) as f64 / (lo_count + 1) as f64).ln() / span;
        if worst.is_none_or(|(_, e, _)| exponent > e) {
            worst = Some((name, exponent, hi_count));
        }
        if exponent > limit + EXPONENT_TOLERANCE && hi_count >= WORK_FLOOR {
            let growth = format!(
                "{name} grows as n^{exponent:.2} over n={}..{} ({lo_count} → {hi_count}; \
                 limit n^{limit:.1})",
                lo.0, hi.0
            );
            match known_family(input, name, exponent) {
                // one line for the family; the detail names the member
                Some(family) => findings.push(Finding {
                    key: FanoutKey {
                        leg: Leg::Work,
                        shape: family.label.to_string(),
                        cause: family.cause.to_string(),
                    },
                    detail: format!("`{}`: {growth}", shape.name),
                }),
                None => findings.push(own(Leg::Work, growth)),
            }
        }
    }
    result.exponent = worst.map(|(name, e, _)| (name, e));
    result
}

/// The key set a run's findings make.
pub(super) fn found_keys(findings: &[Finding]) -> BTreeSet<FanoutKey> {
    findings.iter().map(|f| f.key.clone()).collect()
}

/// One key, for a human.
pub(super) fn render_key(key: &FanoutKey) -> String {
    format!("{} `{}` ({})", key.leg.name(), key.shape, key.cause)
}

pub(super) fn growth_token(growth: Growth) -> &'static str {
    match growth {
        Growth::Nesting => "nesting",
        Growth::Chain => "chain",
        Growth::Flat => "flat",
        Growth::Fixed => "fixed",
    }
}

pub(super) fn parser_token(parser: ParserType) -> &'static str {
    lang_token(parser)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, growth: Growth) -> Input {
        hand(name, Lang::Ts, growth, ["", "a", "", "", ""])
    }

    #[test]
    fn a_family_explains_one_counter_in_its_band_alone() {
        let member = input("ts_gen `a` + ` < a`", Growth::Chain);
        assert_eq!(
            known_family(&member, "scans", 2.0).map(|f| f.cause),
            Some("K73")
        );
        assert!(known_family(&member, "predicates", 2.0).is_none());
        assert!(known_family(&member, "scans", 2.5).is_none());
        let shift = input("ts_gen `a` + ` << a`", Growth::Chain);
        assert!(known_family(&shift, "scans", 2.0).is_some());
        let nested = input("ts_gen `a` + ` < a`", Growth::Nesting);
        assert!(known_family(&nested, "scans", 2.0).is_none());
        let other = input("ts_gen `a` + ` > a`", Growth::Chain);
        assert!(known_family(&other, "scans", 2.0).is_none());
        // the member's own name pins nothing: every other finding on it fails as NEW
        assert_eq!(known_cause(&member.shape.name), NO_CAUSE);
        assert_eq!(known_cause("k289_nonnull_member_operand"), "K289");
    }
}
