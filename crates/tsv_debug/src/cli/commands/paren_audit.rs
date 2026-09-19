//! Paren-authoring independence audit — a redundant paren carries no authoring signal, so
//! the two spellings of one document must reach one fixed point.
//!
//! Two site classes, each a rule that reads something the parens change:
//! **same-operator logical re-association** (§Why this exists) and the **relational
//! `<`…`>` chain** (§The relational chain). They share the verdict, the containment and
//! the zero tolerance; only the splice differs.
//!
//! ## Why this exists
//!
//! `a ?? b ?? c` and `a ?? (b ?? c)` are **one document**. Prettier says so
//! structurally: its parse postprocess (`../prettier/src/language-js/parse/postprocess/index.js`,
//! `onLeave(LogicalExpression)` → `rebalanceLogicalTree`) rewrites `a op (b op c)` into
//! `(a op b) op c` whenever `node.operator === node.right.operator`, recursively, for both
//! the `babel` and `typescript` parsers — so by the time its printer runs, the two
//! authorings ARE the same tree and print byte-identically.
//!
//! tsv cannot rebalance: acorn keeps the right-nested shape and tsv's wire must match it
//! (the drop-in AST contract, priority #1). So every rule that prettier asks of the
//! rebalanced tree, tsv must ask through a **rebalanced view** of its own
//! (`BinaryExpression::rebalanced_right`, named for prettier's `isUnbalancedLogicalTree`).
//! That is a per-rule obligation spread over six callers, and **a rule left on the raw
//! `binary.right` is invisible on every paren-free authoring** — which is every authoring
//! a formatted corpus contains, since tsv's own output normalizes the parens away.
//!
//! bug539 was exactly that: one half of `should_inline_logical_expression` reading
//! `binary.right`. It passed all ~4,300 fixtures and the whole 9,305-file corpus; only a
//! hand-written paren-nested twin exposed it. This audit is the standing instrument for
//! that class — nothing else enumerates the twin.
//!
//! ## The mutation (a pure two-character insertion)
//!
//! For every wire `LogicalExpression` `N` whose **left** is a `LogicalExpression` with the
//! **same operator** — i.e. a same-operator chain of three or more operands, which is what
//! a left-associative parse of `a op b op c` produces — insert `(` at `N.left.right.start`
//! and `)` at `N.end`. On `a op b op c` that yields `a op (b op c)`: the exact inverse of
//! prettier's rebalance, and the twin the class hides behind.
//!
//! Three facts make the splice sound, and each is a property of the acorn wire rather than
//! an assumption about the text:
//!
//! - **Both offsets are token boundaries.** `N.left.right.start` sits after the operator
//!   token (so the `(` can never glue onto a preceding identifier — the base is formatted,
//!   so whitespace separates them), and `N.end` is acorn's `lastTokEnd` for the whole node.
//! - **`N.end` is past every closing paren the node holds**, so a group opened inside
//!   `[open, close)` also closes inside it. The one unbalanced case is the mirror — a
//!   *parenthesized* `N.left.right`, whose own `(` sits before `open` (acorn's node span
//!   excludes it). The stray `)` then pairs with the inserted `(`, and the original `(`
//!   pairs with the inserted `)`, which re-parses as the same re-association one level out.
//!   So the variant always parses, and always parses as intended.
//! - **`&&` / `||` / `??` re-associate.** All three short-circuit left to right, so
//!   `(a op b) op c` and `a op (b op c)` evaluate the same operands in the same order to the
//!   same value. `LogicalExpression` carries exactly those three operators, which is why
//!   this keys on the node type and not on an operator list: a `BinaryExpression` (`+`, `&`)
//!   is NOT re-associable and prettier does not rebalance it.
//!
//! ## The relational chain
//!
//! A `>` whose left operand is a `<` gets a paren pair around that operand whenever the
//! `<`…`>` region would re-lex as a type-argument list once printed
//! (`docs/conformance_prettier_ts.md` §Relational chain type-argument parens). That rule
//! reads **bytes** — which head the region opens with, and where its scan stops — so a
//! redundant paren is exactly what can move its answer while leaving the program alone. Two
//! splices per such node, each a pure insertion around a whole operand:
//!
//! - **the chain** — `a < X > c` against `(a < X) > c`, the shell the printer strips, which
//!   moves where the region's scan stops;
//! - **the operand** — `a < X > c` against `a < (X) > c`, which moves which HEAD the
//!   lookahead dispatches on.
//!
//! The second is where a byte-reading head test is most exposed: a `(` head arm that grades
//! nothing commits every parenthesized operand, so `a < (arr[b - 1]) > c` would take a pair
//! its own paren-free twin does not — and no other standing gate can see that, since each
//! authoring is idempotent on its own and only a twin shows the two fixed points. Nothing
//! re-associates here, but that does not make every twin probeable: what the splice
//! preserves, and what it does not, is stated on
//! [`ParenAuditCommand::twin_exclusion_reason`].
//!
//! ## Which invocation carries the relational floor
//!
//! Both classes share the total vacuity floor (`check_graded_nonzero` over every site), which
//! holds on any corpus: a logical chain is ordinary code. The relational rows get a **second,
//! per-class floor** — and it is **opt-in**, spelled `--require-relational`, because a vacuity
//! floor is a claim about the SEED SET rather than about the audit. Nobody writes `a < b > c`
//! in real code: over the `../corpora` snapshot both relational rows are legitimately **0**,
//! and that zero is the corpus's shape, not a class that stopped enumerating. The class lives
//! in the fixture tree (`tests/fixtures/typescript/expressions/binary/relational_*`), so the
//! **fixture-tree gate** — `deno task paren:audit`, which `deno task check` runs — is the one
//! invocation that passes the flag, and the **real-code leg** of `audit:corpus`
//! (`benches/js/corpus_audit.ts`, publish Step 3c) deliberately does not: it grades every
//! class it finds and holds only the total floor.
//!
//! The flag rather than a `paths`-contains-`tests/fixtures` sniff: an explicit switch is
//! greppable and cannot silently mean the wrong thing on a subtree run. A **narrowed** run
//! under the flag fails its floor, like a ratchet's refusal of a narrowed `:update` — the flag
//! asserts the run covers the relational corpus, so a subtree run drops it.
//!
//! ## The verdict, and why it is zero-tolerance
//!
//! `format(variant)` must equal the base `F` — byte for byte, at every site. Unlike the
//! whitespace [`authoring_audit`](super::authoring_audit), which has a sanctioned dual-stable
//! remainder (a newline the author wrote is layout *intent*), a redundant paren carries no
//! intent that survives either formatter: prettier erases it at parse and tsv's printer
//! re-derives parens from the tree. There is no authoring signal here to preserve, so
//! **every divergence is a bug** and the audit is a hard gate rather than a ratchet.
//!
//! ## Blind spots
//!
//! - **Only the paren-INSERTING direction.** Sites are enumerated from `F`, tsv's own fixed
//!   point, which normalizes the parens away — so a chain reaches this audit paren-free and
//!   is probed toward the nested twin. Were tsv's fixed point ever to *retain* a redundant
//!   paren, that shape's paren-free twin would go unprobed here; it is also a prettier
//!   divergence, which `corpus:compare:format` grades.
//! - **One re-association step per node.** A four-operand chain contributes one site per
//!   same-operator node, each re-associating one level; the fully right-nested composition of
//!   those steps is not enumerated. One step is what turns `binary.right` from a leaf into a
//!   `LogicalExpression`, which is the whole signal.
//! - **Same-operator only** for the logical class, because that is precisely what prettier
//!   rebalances. `a || (b && c)` is a different tree in both formatters and its parens are
//!   load-bearing.
//! - **The relational class probes `<` under `>` only** — the one join the rule is keyed
//!   on. A `<` under any other operator, and a `>` over any other operand, take no pair, so
//!   a shell there is the general redundant-paren question the TODO below names.
//! - **A seed bearing a format-ignore directive is skipped** (`source_has_ignore_directive`):
//!   a frozen region reproduces the inserted parens verbatim, which is correct behavior and
//!   would read as a divergence.
//! - **A site whose `(` would land after a forward-binding block comment is excluded**
//!   ([`splits_forward_binding_comment`]) — the splice would re-bind a JSDoc cast or a bundler
//!   annotation, making the twin a different document. Counted in the report, not dropped
//!   silently.
//! - **A relational site whose twin tsv does not read as the same document is excluded**
//!   ([`ParenAuditCommand::twin_exclusion_reason`], which states why a twin can fail to be
//!   one). Counted in the report, not dropped silently. The divergence itself is a parse
//!   question, but no parse gate sees these twins — the audit synthesizes them, and no
//!   fixture or corpus file holds them — so the count printed here is where they surface.
//!   It is not pinned: it is a function of whichever seed set the invocation was handed, so
//!   it cannot be held the way a ratchet's snapshot is.
//! - **CSS has no expression grammar**, so `.css` seeds hold no sites; the audit's subject
//!   filter is the Svelte + TypeScript families.
//!
//! ## Containment
//!
//! Each file's work runs under `catch_unwind` with the default panic hook suppressed
//! (`audit::panic_hook`), the same bracket the pristine sweep and the injection audits
//! use: a panic anywhere in a file — the base format, the wire walk, a variant format — is
//! recorded as a PANICKED entry (path + message, an exact count with a bounded sample) and
//! the walk continues, rather than one adversarial file killing a corpus run with its
//! findings unprinted. A panic FAILS the run: a crash on a seed is never a pass, and a
//! gate that reported it green would launder the loudest possible finding. Catching needs
//! the corpus profile's `panic = "unwind"`; a stack overflow is not a panic and still
//! aborts (the sized-stack contract in `tsv_cli::cli::stack` is what bounds that).
//!
// TODO: the general redundant-paren class — wrap ANY expression in parens and require one
// fixed point — is a strictly larger instrument this one is the first slice of. It needs a
// `preserve_parens` reparse to enumerate the removal direction, and its findings would mix
// the deliberate paren retentions (a JSDoc cast's shell, a required operand pair) with real
// bugs, so it wants its own triage rather than this gate's zero tolerance.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::Path;

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::audit::excerpt::{first_line_diff, line_context};
use crate::audit::panic_hook::SuppressedPanicHook;
use crate::audit::properties::{
    BaseFixedPoint, Utf16ToByte, base_fixed_point, source_has_ignore_directive, tsv_parse_to_value,
};
use crate::audit::repro::{ReproCase, write_repro_case};
use crate::audit::tally::CappedPaths;
use crate::audit::vacuity::{check_graded_nonzero, check_required_nonzero};
use crate::cli::CliError;

use super::profile::{is_input_invalid_fixture, is_svelte, is_ts_family, resolve_seed_files_named};

/// Audit whether a chain formats identically to its redundantly-parenthesized twin.
///
/// Two classes. A same-operator LOGICAL chain: prettier rebalances `a op (b op c)` into
/// `(a op b) op c` at parse time, so the two authorings are one document. A RELATIONAL
/// `<`…`>` chain: the rule that parenthesizes its `<` operand reads bytes, so a shell
/// around either the chain or the operand can move its answer. tsv must reach one fixed
/// point from every spelling. Pure Rust — one format per site, no sidecar. Defaults to
/// `tests/fixtures` when no paths are given; Svelte and TypeScript-family files (a `.css`
/// seed holds no expressions).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "paren_audit")]
pub struct ParenAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// list the relational sites the run declined to probe, with the reason and the twin's
    /// own text — the exclusion count is otherwise unauditable
    #[argh(switch)]
    list_excluded: bool,

    /// write a byte-exact repro (base / variant / ftry / ftry2) per finding into this dir
    #[argh(option)]
    dump_dir: Option<String>,

    /// cap the number of files audited (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// cap the number of findings reported (default 20)
    #[argh(option, default = "20")]
    examples: usize,

    /// require the relational `<`…`>` classes to have graded a site — the seed-set
    /// vacuity floor, for an invocation whose corpus holds the class (the fixture-tree
    /// gate). Real code writes no `a < b > c`, so its run drops this; a narrowed run
    /// drops it too
    #[argh(switch)]
    require_relational: bool,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// The three operators `LogicalExpression` carries — and exactly the three prettier
/// rebalances. Named rather than kept as the wire's string so the report's coverage
/// breakdown is a closed set: an operator with zero sites is a corpus gap the table shows.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum LogicalOp {
    And,
    Or,
    Nullish,
}

impl LogicalOp {
    fn from_wire(op: &str) -> Option<Self> {
        match op {
            "&&" => Some(Self::And),
            "||" => Some(Self::Or),
            "??" => Some(Self::Nullish),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::And => "&&",
            Self::Or => "||",
            Self::Nullish => "??",
        }
    }
}

/// What a site's spliced pair is a twin OF — the audit's classes, one row each in the
/// report. A closed set, so a class with zero sites reads as a corpus gap rather than a
/// pass.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum SiteKind {
    /// Same-operator logical re-association: `a op b op c` against `a op (b op c)`.
    Logical(LogicalOp),
    /// A shell around a relational `<`…`>` CHAIN: `a < X > c` against `(a < X) > c`.
    RelationalChain,
    /// A shell around the `<` OPERAND of such a chain: `a < X > c` against `a < (X) > c`.
    /// The direction the region test is most exposed to: a shell there changes which head
    /// arm the type-argument lookahead dispatches on, where the chain's own shell only
    /// changes where the scan stops.
    RelationalOperand,
}

impl SiteKind {
    fn label(self) -> &'static str {
        match self {
            Self::Logical(op) => op.label(),
            Self::RelationalChain => "< > chain",
            Self::RelationalOperand => "< > operand",
        }
    }

    const ALL: [Self; 5] = [
        Self::Logical(LogicalOp::And),
        Self::Logical(LogicalOp::Or),
        Self::Logical(LogicalOp::Nullish),
        Self::RelationalChain,
        Self::RelationalOperand,
    ];

    /// Whether this class is a relational one — the two rows whose own vacuity floor the
    /// run checks, so the class cannot quietly stop enumerating.
    fn is_relational(self) -> bool {
        !matches!(self, Self::Logical(_))
    }
}

/// One site in the formatted base: insert `(` at `open`, `)` at `close`.
#[derive(Clone, Copy, Debug)]
struct Site {
    open: usize,
    close: usize,
    kind: SiteKind,
}

/// A relational site the run declined to probe, kept so the exclusion can be AUDITED
/// rather than only counted: a class that stops being the same document on both spellings
/// is indistinguishable, from the count alone, from a fresh tsv-only over-rejection.
#[derive(Debug, Clone)]
struct ExcludedSite {
    path: String,
    line: usize,
    column: usize,
    kind: &'static str,
    /// Why the twin is not the same document — which is the whole question an auditor asks
    /// of the count.
    reason: &'static str,
    /// The twin's own text, so the spelling can be handed to another parser as-is.
    twin: String,
}

/// A file's enumeration: the sites to probe, and counts of the ones excluded as unsound.
/// Each exclusion is counted rather than dropped silently — an audit that quietly stops
/// probing a class reads exactly like one that finds nothing in it.
#[derive(Default)]
struct Sites {
    probed: Vec<Site>,
    comment_bound: usize,
    parse_divergent: usize,
    excluded: Vec<ExcludedSite>,
}

/// Collect every same-operator re-association site in a wire AST.
///
/// The walk is over the wire `Value` rather than any language's internal AST because the
/// shape it looks for is identical in all of them: a `.ts` module and a `.svelte`
/// component's `<script>`, template expression, block head, attribute and `{@const}` all
/// serialize their expressions as the same acorn nodes at absolute file offsets. One walk
/// therefore covers both subject languages with no per-language enumerator.
fn collect_sites(node: &Value, f: &str, map: &Utf16ToByte, out: &mut Sites) {
    match node {
        Value::Array(items) => {
            for item in items {
                collect_sites(item, f, map, out);
            }
        }
        Value::Object(fields) => {
            if let Some(site) = logical_site_at(fields, map) {
                push_site(site, f, out);
            }
            for site in relational_sites_at(fields, map).into_iter().flatten() {
                push_site(site, f, out);
            }
            for (key, value) in fields {
                // `loc` is the only object-valued field that carries no nodes; skipping it
                // is a cost saving, not a correctness one (it holds no `type`).
                if key != "loc" {
                    collect_sites(value, f, map, out);
                }
            }
        }
        _ => {}
    }
}

/// Whether a `(` inserted at `at` would land between a **forward-binding block comment** and
/// the token it binds — and so change what that comment annotates.
///
/// Two comment kinds bind forward: a JSDoc cast (`/** @type {T} */ (x)` — the comment plus the
/// parens ARE the cast) and a bundler annotation (`/* @__PURE__ */ f()`). Splicing a `(` after
/// one hands it a different subtree: `err && /** @type {any} */ (err).name && …` becomes
/// `err && /** @type {any} */ ((err).name && …)`, where the cast now annotates the whole
/// chain. That is a different document, so requiring the two to format alike would be asking
/// the formatter to erase the distinction — the finding would be about this instrument, not
/// about tsv. (Both of the corpus's only two divergences were exactly this.) The paren-vs-comment
/// binding question is `binding_audit`'s subject, where a migrating paren IS the finding.
///
/// A **line** comment is deliberately still in scope: it runs to the end of its line, so
/// anything it precedes sits on the next line and it is that line's trailing comment — it binds
/// backward and cannot be a cast or an annotation. The one line comment this coarse test cannot
/// tell from a block close is one whose own text ends in `*/` (`// see /* … */`); that site is
/// over-excluded, which is the safe direction and is why the count is reported.
///
/// The trim is Rust's `char::is_whitespace`, wider than JS's `WhiteSpace` production. Wrong in
/// the safe direction too: a wider class only skips more sites, where a narrower one would probe
/// a splice this is meant to exclude.
fn splits_forward_binding_comment(f: &str, at: usize) -> bool {
    f[..at].trim_end().ends_with("*/")
}

/// Record one site, unless splicing it would re-bind a forward-binding comment — the one
/// unsound splice, excluded rather than reported (see [`splits_forward_binding_comment`]).
fn push_site(site: Site, f: &str, out: &mut Sites) {
    if splits_forward_binding_comment(f, site.open) {
        out.comment_bound += 1;
    } else {
        out.probed.push(site);
    }
}

/// The two shells a relational `<`…`>` chain is probed with, when this node is one: a
/// `BinaryExpression` `>` whose left operand is a `BinaryExpression` `<`.
///
/// Both splices are pure paren insertions around a whole operand, with none of the logical
/// splice's re-association argument to make (nothing moves). Whether tsv's own parser reads
/// each twin as the same document is a separate question, answered per site by
/// [`ParenAuditCommand::twin_exclusion_reason`]. The pair is what tsv's own
/// relational-chain rule synthesizes and retains
/// (`conformance_prettier_ts.md` §Relational chain type-argument parens), and that rule
/// reads BYTES — which head the `<` region opens with, where the region's scan stops — so
/// both authorings have to reach one fixed point or the rule is keyed on the spelling
/// rather than on the program.
fn relational_sites_at(
    fields: &serde_json::Map<String, Value>,
    map: &Utf16ToByte,
) -> [Option<Site>; 2] {
    let none = [None, None];
    let binary_op = |node: &serde_json::Map<String, Value>| -> Option<String> {
        (node.get("type")?.as_str()? == "BinaryExpression")
            .then(|| node.get("operator")?.as_str().map(str::to_string))
            .flatten()
    };
    if binary_op(fields).as_deref() != Some(">") {
        return none;
    }
    let Some(left) = fields.get("left").and_then(Value::as_object) else {
        return none;
    };
    if binary_op(left).as_deref() != Some("<") {
        return none;
    }
    let span = |node: &serde_json::Map<String, Value>| -> Option<Site> {
        let open = map.byte(node.get("start")?.as_u64()? as usize)?;
        let close = map.byte(node.get("end")?.as_u64()? as usize)?;
        (open < close).then_some(Site {
            open,
            close,
            kind: SiteKind::RelationalChain,
        })
    };
    let chain = span(left);
    let operand = left
        .get("right")
        .and_then(Value::as_object)
        .and_then(span)
        .map(|site| Site {
            kind: SiteKind::RelationalOperand,
            ..site
        });
    [chain, operand]
}

/// The logical site this node is, if it is one: a `LogicalExpression` whose left operand
/// is a `LogicalExpression` with the same operator.
fn logical_site_at(fields: &serde_json::Map<String, Value>, map: &Utf16ToByte) -> Option<Site> {
    if fields.get("type")?.as_str()? != "LogicalExpression" {
        return None;
    }
    let op = LogicalOp::from_wire(fields.get("operator")?.as_str()?)?;
    let left = fields.get("left")?.as_object()?;
    if left.get("type")?.as_str()? != "LogicalExpression"
        || LogicalOp::from_wire(left.get("operator")?.as_str()?)? != op
    {
        return None;
    }
    // The inner right operand's start is where the redundant group opens; the OUTER node's
    // end is where it closes — never the outer right operand's end, which for a
    // parenthesized operand stops before its own `)`.
    let open = map.byte(left.get("right")?.as_object()?.get("start")?.as_u64()? as usize)?;
    let close = map.byte(fields.get("end")?.as_u64()? as usize)?;
    (open < close).then_some(Site {
        open,
        close,
        kind: SiteKind::Logical(op),
    })
}

/// Drop every positional key from a wire AST, in place — what is left is the tree alone,
/// which is what "the same document" means for a splice that inserts two bytes.
fn strip_positions(node: &mut Value) {
    match node {
        Value::Array(items) => items.iter_mut().for_each(strip_positions),
        Value::Object(fields) => {
            fields.retain(|key, _| !matches!(key.as_str(), "start" | "end" | "loc" | "range"));
            fields.values_mut().for_each(strip_positions);
        }
        _ => {}
    }
}

/// The 1-based line and column of a byte offset, for naming an excluded site.
fn line_column_at(f: &str, offset: usize) -> (usize, usize) {
    let head = &f[..offset.min(f.len())];
    let line = head.matches('\n').count() + 1;
    let column = head.rfind('\n').map_or(offset, |nl| offset - nl - 1) + 1;
    (line, column)
}

/// The excluded twin's own line, trimmed — enough to hand the spelling to another parser
/// without reopening the file, and bounded so a corpus run's JSON stays readable.
fn excluded_twin_text(f: &str, site: &Site) -> String {
    let twin = splice(f, site);
    let start = twin[..site.open].rfind('\n').map_or(0, |nl| nl + 1);
    let end = twin[start..].find('\n').map_or(twin.len(), |nl| start + nl);
    let line = twin[start..end].trim();
    line.chars().take(200).collect()
}

/// Splice one site's redundant parens into the formatted base.
fn splice(f: &str, site: &Site) -> String {
    let mut out = String::with_capacity(f.len() + 2);
    out.push_str(&f[..site.open]);
    out.push('(');
    out.push_str(&f[site.open..site.close]);
    out.push(')');
    out.push_str(&f[site.close..]);
    out
}

/// What one site's probe found.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Verdict {
    /// The twin formats back to the base — the property holds.
    Converge,
    /// The twin formats to a second stable output: two fixed points for one document.
    Diverge,
    /// The twin's output is not even a fixed point of its own — F1 broken on the twin.
    NonIdempotent,
    /// The twin does not parse. The splice's soundness argument says this cannot happen,
    /// so it is a finding about the instrument (or the parser), never a pass.
    VariantParseError,
}

impl Verdict {
    fn key(self) -> &'static str {
        match self {
            Self::Converge => "converge",
            Self::Diverge => "diverge",
            Self::NonIdempotent => "diverge_non_idempotent",
            Self::VariantParseError => "variant_parse_error",
        }
    }

    /// Everything but [`Self::Converge`] fails the gate — see the module docs on why this
    /// audit has no sanctioned dual-stable remainder.
    fn is_finding(self) -> bool {
        self != Self::Converge
    }

    const ALL: [Self; 4] = [
        Self::Converge,
        Self::Diverge,
        Self::NonIdempotent,
        Self::VariantParseError,
    ];
}

/// One reported finding, carrying enough to reproduce it by hand.
struct Finding {
    path: String,
    offset: usize,
    kind: SiteKind,
    verdict: Verdict,
    context: String,
    /// The first line at which `format(variant)` departs from the base, as
    /// `(line number, base line, variant line)`. `None` when the twin failed to parse, and
    /// when the two are line-for-line equal yet not byte-equal (they differ only in a
    /// trailing newline) — there is no line to show either way.
    first_diff: Option<(usize, String, String)>,
}

#[derive(Default)]
struct Report {
    files_scanned: usize,
    files_parse_error: usize,
    /// Files that could not be read — never scanned.
    files_read_error: usize,
    files_output_reparse_error: usize,
    files_ignore_directive: usize,
    /// Seeds whose own format is not a fixed point: excluded from the re-association
    /// analysis (there is nothing for the twin to converge ON) and a failure of the run.
    /// A [`CappedPaths`] rather than a count beside a `Vec`, so the number the report
    /// prints cannot drift from the list under it and the list stays bounded on a corpus
    /// that has many.
    base_non_idempotent: CappedPaths,
    /// Files whose work PANICKED (`path: message`) — caught per file so the walk finishes,
    /// counted exactly with a bounded sample, and a failure of the run. See the module
    /// docs' §Containment. Counts recorded before the panic (sites, verdicts) stay in the
    /// report; each was complete when it was taken.
    panics: CappedPaths,
    sites: usize,
    sites_comment_bound: usize,
    /// Relational sites whose twin tsv rejects or reads as a different program — see
    /// [`ParenAuditCommand::twin_exclusion_reason`].
    sites_parse_divergent: usize,
    /// Those sites themselves, so the count can be checked rather than trusted.
    excluded_sites: Vec<ExcludedSite>,
    counts: BTreeMap<Verdict, usize>,
    kind_counts: BTreeMap<(SiteKind, Verdict), usize>,
    /// Sites enumerated per class, so a class that stops producing them fails its own
    /// vacuity floor instead of riding the others' counts.
    kind_sites: BTreeMap<SiteKind, usize>,
    findings: Vec<Finding>,
    /// Sequence number for `--dump-dir` case directories, so two findings in one file do not
    /// collide on a name.
    dump_seq: usize,
}

impl Report {
    fn count(&self, v: Verdict) -> usize {
        self.counts.get(&v).copied().unwrap_or(0)
    }

    fn kind_count(&self, kind: SiteKind, v: Verdict) -> usize {
        self.kind_counts.get(&(kind, v)).copied().unwrap_or(0)
    }

    /// Sites enumerated across the relational classes — the seed-set vacuity floor's
    /// denominator, read only by a run that asked for it.
    fn relational_sites(&self) -> usize {
        self.kind_sites
            .iter()
            .filter(|(kind, _)| kind.is_relational())
            .map(|(_, n)| n)
            .sum()
    }

    fn findings_total(&self) -> usize {
        Verdict::ALL
            .iter()
            .filter(|v| v.is_finding())
            .map(|v| self.count(*v))
            .sum()
    }

    /// Whether this run FAILS — the one question the exit code and the report's ✓ are two
    /// readings of, so the two cannot disagree. A base-non-idempotent file fails the run
    /// while producing no site finding at all, and that is exactly the case a ✓ keyed on the
    /// findings alone prints a pass over (`../prettier/tests/format` holds six such files).
    /// A panicking file is the same shape one step louder.
    fn failed(&self) -> bool {
        self.findings_total() + self.base_non_idempotent.count() + self.panics.count() > 0
    }

    /// Whether every vacuity floor THIS run is held to was met — the total always, the
    /// relational one only when the invocation asked for it.
    ///
    /// Stated once because the report's ✓ and the exit code are two readings of it, and a ✓
    /// keyed on a floor the run was never held to is the same doctrine break as one keyed on
    /// too few: over real code the relational rows are legitimately zero, and a ✓ withheld
    /// there reads as a failure the run did not have.
    fn floors_met(&self, require_relational: bool) -> bool {
        self.sites > 0 && (!require_relational || self.relational_sites() > 0)
    }
}

impl ParenAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        // A scan over nothing must not read as a pass — resolution fails loud on an empty
        // subject set, naming it rather than the raw walk.
        let files = resolve_seed_files_named(
            &self.paths,
            self.limit,
            ".svelte / TypeScript-family files",
            |p| is_svelte(p) || is_ts_family(p),
        )?;

        let mut report = Report::default();
        {
            // Suppressed for the walk only: a caught panic is recorded below, and the
            // default hook's per-file backtrace would bury the report. Restored on drop.
            let _hook = SuppressedPanicHook::install();
            for path in &files {
                let scanned = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    self.scan_file(path, &mut report);
                }));
                if let Err(payload) = scanned {
                    report.panics.push_panic(path, payload.as_ref());
                }
            }
        }

        if self.json {
            print_json(&report, self.require_relational);
        } else {
            print_human(
                &report,
                self.examples,
                self.require_relational,
                self.list_excluded,
            );
        }

        // A base-non-idempotent file is excluded from the re-association analysis (its fixed
        // point is undefined, so "formats back to the base" is meaningless) — but excluding
        // it is not a reason to pass the run, or a whole-file reflow could sit here green.
        if report.failed() {
            return Err(CliError::Failed);
        }
        // The floor UNDER the non-empty resolution: this audit's product is per-SITE, so a
        // corpus of files that hold no same-operator chain renders exactly the tables a
        // clean run does. The resolution's non-empty file list cannot see that.
        check_graded_nonzero(report.sites, "paren-authoring sites")?;
        // A second floor, per CLASS — and an OPT-IN one, because it is a claim about the
        // SEED SET rather than about the audit. Logical chains are everywhere, so the total
        // above holds on any corpus; a relational `a < b > c` chain is written in the fixture
        // tree and essentially nowhere else, so over real code zero relational sites is the
        // correct reading. Only the invocation whose corpus holds the class asks for it.
        if self.require_relational {
            check_required_nonzero(
                report.relational_sites(),
                "relational chain sites",
                "--require-relational",
            )?;
        }
        Ok(())
    }

    /// Why the redundantly-parenthesized twin of `site` is not the SAME DOCUMENT to tsv's own
    /// parser, or `None` when it is and the site may be probed — the probe's precondition,
    /// since a formatter cannot be asked to reach one fixed point from two programs.
    ///
    /// The relational splice is tree-preserving **in the language**: parenthesizing a whole
    /// operand changes no program. It is **not** guaranteed to be tree-preserving under tsv's
    /// `Parse`, so the twin may fail to parse there, or parse as a different tree.
    ///
    /// Checked only for the relational classes, and only because a paren around a `<`
    /// operand genuinely can move tsv's PARSE: the type-argument lookahead's `(` head arm
    /// grades the shell's CONTENT, so a twin whose content opens a type-argument list opens a
    /// region its paren-free twin does not. Where that content parses as a type, the twin is
    /// a DIFFERENT TREE — `p < (readonly.a) > (t, u)`, a call with type arguments to tsv and
    /// a comparison chain to acorn. Where it spells a parameter list, or a body tsc's error
    /// recovery carries to the `>`, tsv REJECTS the twin, following tsc —
    /// `x < (a = b) > (t, u)`, `x < (a << b) > (t, u)`, both the comparison chain to acorn.
    /// Either way it is a PARSER divergence against tsv's parse oracle rather than a
    /// formatting question, so this audit excludes the site and counts it, the way it does a
    /// comment-bound one.
    ///
    /// The two reasons are not interchangeable to an auditor: `rejected` means tsv's parser
    /// refuses a spelling the bare one accepts, which is a parse-oracle question that may
    /// be a deliberate divergence OR a fresh over-rejection; `different tree` means both
    /// parse and the shell moved the reading. Naming them is what lets the excluded count
    /// be graded against another parser instead of trusted, which `--list-excluded` and the
    /// `--json` `excluded_sites` array are for.
    ///
    /// The logical class asks nothing of the kind: its splice re-associates a tree acorn
    /// builds the same either way, so a twin that fails to parse there stays the loud
    /// `VariantParseError` finding it always was.
    fn twin_exclusion_reason(
        f: &str,
        site: &Site,
        base: &Value,
        parser: ParserType,
    ) -> Option<&'static str> {
        let Some(mut twin) = tsv_parse_to_value(&splice(f, site), parser) else {
            return Some("rejected");
        };
        let mut base = base.clone();
        strip_positions(&mut twin);
        strip_positions(&mut base);
        if twin == base {
            None
        } else {
            Some("different tree")
        }
    }

    fn scan_file(&self, path: &Path, report: &mut Report) {
        if is_input_invalid_fixture(path) {
            return;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            report.files_read_error += 1;
            return;
        };
        // A frozen region reproduces the inserted parens verbatim — correct behavior that
        // would read as a divergence. Same coarse pre-scan the injection audits exempt on.
        if source_has_ignore_directive(&source) {
            report.files_ignore_directive += 1;
            return;
        }
        let parser = ParserType::from_extension(&path.to_string_lossy());
        report.files_scanned += 1;
        let f = match base_fixed_point(&source, parser) {
            BaseFixedPoint::Ok(f) => f,
            BaseFixedPoint::ParseError => {
                report.files_parse_error += 1;
                return;
            }
            BaseFixedPoint::NonIdempotent => {
                report.base_non_idempotent.push(path.display().to_string());
                return;
            }
        };
        let Some(wire) = tsv_parse_to_value(&f, parser) else {
            // The base formatted but its own output does not reparse — `roundtrip_audit`'s
            // property, gated there. Counted apart from a seed the parser rejects: those are
            // ordinary (a `tsv_rejects` fixture, a prettier-rejects input), this is a bug
            // somewhere, and folding the two would hide it in the ordinary count.
            report.files_output_reparse_error += 1;
            return;
        };
        let map = Utf16ToByte::new(&f);
        let mut sites = Sites::default();
        collect_sites(&wire, &f, &map, &mut sites);
        let mut excluded = Vec::new();
        sites.probed.retain(|site| {
            if !site.kind.is_relational() {
                return true;
            }
            let Some(reason) = Self::twin_exclusion_reason(&f, site, &wire, parser) else {
                return true;
            };
            sites.parse_divergent += 1;
            let (line, column) = line_column_at(&f, site.open);
            excluded.push(ExcludedSite {
                path: path.display().to_string(),
                line,
                column,
                kind: site.kind.label(),
                reason,
                twin: excluded_twin_text(&f, site),
            });
            false
        });
        sites.excluded = excluded;
        report.sites += sites.probed.len();
        report.sites_comment_bound += sites.comment_bound;
        report.sites_parse_divergent += sites.parse_divergent;
        report.excluded_sites.append(&mut sites.excluded);
        for site in &sites.probed {
            *report.kind_sites.entry(site.kind).or_default() += 1;
        }
        for site in &sites.probed {
            self.probe(path, &f, site, parser, report);
        }
    }

    fn probe(&self, path: &Path, f: &str, site: &Site, parser: ParserType, report: &mut Report) {
        let variant = splice(f, site);
        let formatted = format_source(&variant, parser).ok();
        let (verdict, first_diff) = match &formatted {
            None => (Verdict::VariantParseError, None),
            Some(ftry) if ftry == f => (Verdict::Converge, None),
            Some(ftry) => {
                let stable = format_source(ftry, parser).is_ok_and(|x| &x == ftry);
                let verdict = if stable {
                    Verdict::Diverge
                } else {
                    Verdict::NonIdempotent
                };
                (verdict, describe_first_diff(f, ftry))
            }
        };
        *report.counts.entry(verdict).or_default() += 1;
        *report.kind_counts.entry((site.kind, verdict)).or_default() += 1;
        if verdict.is_finding() {
            if let Some(dir) = &self.dump_dir {
                report.dump_seq += 1;
                write_repro_case(
                    dir,
                    &ReproCase {
                        seq: report.dump_seq,
                        tag: verdict.key(),
                        mutation: "splice one site's redundant parens",
                        src_path: path,
                        base: f,
                        variant: &variant,
                        ftry: formatted.as_deref().unwrap_or(""),
                        parser,
                    },
                );
            }
            report.findings.push(Finding {
                path: path.display().to_string(),
                offset: site.open,
                kind: site.kind,
                verdict,
                context: line_context(f, site.open),
                first_diff,
            });
        }
    }
}

/// [`first_line_diff`] in this report's own terms: owned lines, with the run-out side named.
fn describe_first_diff(base: &str, variant: &str) -> Option<(usize, String, String)> {
    first_line_diff(base, variant).map(|(line, b, v)| {
        let spell = |s: Option<&str>| s.unwrap_or("<eof>").to_string();
        (line, spell(b), spell(v))
    })
}

fn print_human(report: &Report, examples: usize, require_relational: bool, list_excluded: bool) {
    println!("Paren-authoring independence audit (logical re-association + relational chain)");
    println!(
        "  files: {} scanned, {} parse-error, {} read-error, {} output-reparse-error, {} ignore-directive (skipped), {} base-non-idempotent",
        report.files_scanned,
        report.files_parse_error,
        report.files_read_error,
        report.files_output_reparse_error,
        report.files_ignore_directive,
        report.base_non_idempotent.count(),
    );
    println!(
        "  sites probed: {} ({} excluded — a `(` there would re-bind a forward-binding comment; \
{} relational excluded — tsv rejects the twin or parses it as a different program, a \
parse-oracle question, not this gate's)",
        report.sites, report.sites_comment_bound, report.sites_parse_divergent,
    );
    if list_excluded && !report.excluded_sites.is_empty() {
        println!();
        println!("  relational sites excluded (`--list-excluded`):");
        for e in &report.excluded_sites {
            println!(
                "    {}:{}:{}  [{}] {}\n        {}",
                e.path, e.line, e.column, e.kind, e.reason, e.twin
            );
        }
    }
    println!();
    println!("  verdicts:");
    println!(
        "    converge:                  {}",
        report.count(Verdict::Converge)
    );
    println!(
        "    DIVERGE (dual-stable):     {}",
        report.count(Verdict::Diverge)
    );
    println!(
        "    DIVERGE (non-idempotent):  {}",
        report.count(Verdict::NonIdempotent)
    );
    println!(
        "    variant parse error:       {}",
        report.count(Verdict::VariantParseError)
    );

    println!();
    println!("  by site class (a zero row is a corpus gap, not a pass):");
    print!("    {:<14}", "class");
    for v in Verdict::ALL {
        print!("{:>24}", v.key());
    }
    println!();
    for kind in SiteKind::ALL {
        print!("    {:<14}", kind.label());
        for v in Verdict::ALL {
            print!("{:>24}", report.kind_count(kind, v));
        }
        println!();
    }

    if !report.base_non_idempotent.is_empty() {
        println!();
        println!("  ✗ base-non-idempotent files (F1 broken — excluded from the re-association");
        println!("    analysis, but a failure in their own right):");
        for line in report.base_non_idempotent.sample_lines("    ") {
            println!("{line}");
        }
    }

    if !report.panics.is_empty() {
        println!();
        println!(
            "  ⚠ {} file(s) PANICKED (caught per file so the walk finished; a failure of the run):",
            report.panics.count()
        );
        for line in report.panics.sample_lines("    ") {
            println!("{line}");
        }
    }

    if report.findings.is_empty() {
        // Only claim the property when something actually carried it AND nothing else failed
        // the run: a run that graded no site is about to fail the vacuity floor, and one
        // holding a base-non-idempotent file has already printed the ✗ list above and exits
        // 1. A ✓ over either reads as a pass the run is not entitled to.
        //
        // Every floor the run is HELD TO is read here, which is what `floors_met` states: a
        // ✓ keyed on the total alone would print over a `--require-relational` run that then
        // exits 1, and one that read the relational rows without the flag would withhold the
        // ✓ from a real-code run that legitimately holds no relational chain and exits 0.
        if report.floors_met(require_relational) && !report.failed() {
            println!();
            println!("  ✓ every chain formats identically to its redundantly-parenthesized twin");
        }
        return;
    }

    println!();
    println!("  ✗ findings (a redundant paren changed the output):");
    for finding in report.findings.iter().take(examples) {
        println!();
        println!(
            "    {}:{}  {}  [{}]",
            finding.path,
            finding.offset,
            finding.kind.label(),
            finding.verdict.key(),
        );
        println!("      «{}»", finding.context);
        if let Some((line, base, variant)) = &finding.first_diff {
            println!("      first diff at line {line}:");
            println!("        base:    {}", base.trim_end());
            println!("        variant: {}", variant.trim_end());
        }
    }
    let shown = report.findings.len().min(examples);
    if report.findings.len() > shown {
        println!();
        println!(
            "    … {} more (raise --examples)",
            report.findings.len() - shown
        );
    }
}

fn print_json(report: &Report, require_relational: bool) {
    let counts: serde_json::Map<String, Value> = Verdict::ALL
        .iter()
        .map(|v| (v.key().to_string(), serde_json::json!(report.count(*v))))
        .collect();
    let kind_counts: serde_json::Map<String, Value> = SiteKind::ALL
        .iter()
        .map(|kind| {
            let per: serde_json::Map<String, Value> = Verdict::ALL
                .iter()
                .map(|v| {
                    (
                        v.key().to_string(),
                        serde_json::json!(report.kind_count(*kind, *v)),
                    )
                })
                .collect();
            (kind.label().to_string(), Value::Object(per))
        })
        .collect();
    let findings: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "path": f.path,
                "offset": f.offset,
                "class": f.kind.label(),
                "verdict": f.verdict.key(),
                "context": f.context,
                "first_diff": f.first_diff.as_ref().map(|(line, base, variant)| serde_json::json!({
                    "line": line,
                    "base": base,
                    "variant": variant,
                })),
            })
        })
        .collect();
    let out = serde_json::json!({
        "files_scanned": report.files_scanned,
        "files_parse_error": report.files_parse_error,
        "files_read_error": report.files_read_error,
        "files_output_reparse_error": report.files_output_reparse_error,
        "files_ignore_directive": report.files_ignore_directive,
        "files_base_non_idempotent": report.base_non_idempotent.count(),
        "files_panicked": report.panics.count(),
        "sites": report.sites,
        "sites_comment_bound": report.sites_comment_bound,
        "sites_parse_divergent": report.sites_parse_divergent,
        "excluded_sites": report
            .excluded_sites
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "line": e.line,
                    "column": e.column,
                    "kind": e.kind,
                    "reason": e.reason,
                    "twin": e.twin,
                })
            })
            .collect::<Vec<_>>(),
        "counts": counts,
        "kind_counts": kind_counts,
        "relational_sites": report.relational_sites(),
        "require_relational": require_relational,
        "findings": findings,
        "base_non_idempotent_sample": report.base_non_idempotent.sample(),
        "panicked_sample": report.panics.sample(),
    });
    super::print_json_pretty(&out);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A caught panic is a failure of the run even with zero site findings — the same
    /// shape as a base-non-idempotent file, one step louder.
    #[test]
    fn a_panicking_file_fails_the_run() {
        let mut report = Report {
            sites: 1,
            ..Report::default()
        };
        assert!(!report.failed());
        report.panics.push("seed.ts: boom".to_string());
        assert!(report.failed());
    }

    /// The relational floor is a claim about the SEED SET, so it is read only by a run that
    /// asked for it: a real-code corpus holds logical chains and no relational one, and
    /// withholding the ✓ there reads as a failure the run does not have.
    #[test]
    fn the_relational_floor_is_read_only_when_required() {
        let logical_only = Report {
            sites: 2530,
            ..Report::default()
        };
        assert_eq!(logical_only.relational_sites(), 0);
        assert!(logical_only.floors_met(false));
        assert!(!logical_only.floors_met(true));

        // The total floor stays unconditional — a run that graded nothing meets neither.
        let empty = Report::default();
        assert!(!empty.floors_met(false));
        assert!(!empty.floors_met(true));
    }

    /// Enumerate the sites of a TypeScript snippet, as the audit does.
    fn sites_of(source: &str) -> Vec<Site> {
        let wire = tsv_parse_to_value(source, ParserType::TypeScript).expect("parses");
        let map = Utf16ToByte::new(source);
        let mut out = Sites::default();
        collect_sites(&wire, source, &map, &mut out);
        out.probed
    }

    /// The variants a source's sites produce, in enumeration order.
    fn variants_of(source: &str) -> Vec<String> {
        sites_of(source).iter().map(|s| splice(source, s)).collect()
    }

    #[test]
    fn three_operand_chain_is_one_site() {
        assert_eq!(
            variants_of("const x = a ?? b ?? c;"),
            ["const x = a ?? (b ?? c);"]
        );
    }

    #[test]
    fn two_operand_chain_is_no_site() {
        assert!(sites_of("const x = a ?? b;").is_empty());
    }

    /// A four-operand chain is `((a op b) op c) op d` — one site per same-operator node, each
    /// re-associating one level. The fully right-nested `a && (b && c && d)` is a composition
    /// of two steps and is deliberately not enumerated: one step is what turns `binary.right`
    /// from a leaf into a `LogicalExpression`, which is the whole signal.
    #[test]
    fn each_chain_level_is_its_own_site() {
        let mut got = variants_of("const x = a && b && c && d;");
        got.sort();
        assert_eq!(
            got,
            [
                "const x = a && (b && c) && d;",
                "const x = a && b && (c && d);",
            ]
        );
    }

    /// A mixed-operator chain is not re-associable and prettier does not rebalance it.
    #[test]
    fn mixed_operators_are_no_site() {
        assert!(sites_of("const x = a || b && c;").is_empty());
        assert!(sites_of("const x = (a ?? b) || c;").is_empty());
    }

    /// `+` is a `BinaryExpression`: associative for neither strings nor floats, and never
    /// rebalanced. Keying on the node type is what excludes it.
    #[test]
    fn arithmetic_is_no_site() {
        assert!(sites_of("const x = a + b + c;").is_empty());
    }

    /// The close offset is the OUTER node's end, so it lands past a parenthesized right
    /// operand's own `)` rather than inside it.
    #[test]
    fn parenthesized_right_operand_closes_outside_its_paren() {
        assert_eq!(
            variants_of("const x = a ?? b ?? (c && d);"),
            ["const x = a ?? (b ?? (c && d));"]
        );
    }

    /// A parenthesized inner right operand leaves a stray `)` inside the spliced range; the
    /// inserted parens re-pair around it and the variant still parses as the intended
    /// re-association.
    #[test]
    fn parenthesized_inner_operand_still_parses_as_intended() {
        let source = "const x = a || (b && c) || d;";
        let variants = variants_of(source);
        assert_eq!(variants, ["const x = a || ((b && c) || d);"]);
        assert!(tsv_parse_to_value(&variants[0], ParserType::TypeScript).is_some());
    }

    /// A JSDoc cast before the operand makes the splice unsound — the `(` would re-bind the
    /// cast to the whole chain — so the site is excluded, not probed.
    #[test]
    fn cast_bound_operand_is_excluded() {
        assert!(sites_of("const x = a && /** @type {any} */ (b).c && d;").is_empty());
        // …and the same chain without the cast IS probed, so the exclusion is keyed on the
        // comment rather than on the shape around it.
        assert_eq!(sites_of("const x = a && (b).c && d;").len(), 1);
    }

    /// A line comment binds backward — it cannot be a cast or an annotation — so an operand
    /// it precedes stays in scope.
    #[test]
    fn line_comment_before_operand_stays_in_scope() {
        assert_eq!(sites_of("const x =\n\ta && // c\n\tb && c;").len(), 1);
    }

    /// A base-non-idempotent file fails the run while producing no site finding — the case
    /// the report's ✓ has to read from the same question the exit code does, or it prints a
    /// pass over a run that exits 1. `../prettier/tests/format` is a real corpus that lands
    /// here (six such files, zero site findings).
    #[test]
    fn a_base_non_idempotent_file_fails_the_run_with_no_findings() {
        let mut report = Report {
            sites: 12,
            ..Report::default()
        };
        report.base_non_idempotent.push("seed.svelte".to_string());
        assert!(report.findings.is_empty());
        assert_eq!(report.findings_total(), 0);
        assert!(report.failed());
    }

    /// The walk is over the wire, so a Svelte component's script, template expression and
    /// block head all enumerate through the same code path.
    #[test]
    fn svelte_expressions_enumerate() {
        let source =
            "<script>const x = a ?? b ?? c;</script>\n{#if p && q && r}<b>{u || v || w}</b>{/if}\n";
        let wire = tsv_parse_to_value(source, ParserType::Svelte).expect("parses");
        let map = Utf16ToByte::new(source);
        let mut out = Sites::default();
        collect_sites(&wire, source, &map, &mut out);
        let mut ops: Vec<&str> = out.probed.iter().map(|s| s.kind.label()).collect();
        ops.sort_unstable();
        assert_eq!(ops, ["&&", "??", "||"]);
    }

    /// A relational `<`…`>` chain contributes BOTH shells — around the chain and around the
    /// `<` operand — and a `<` under any other operator contributes neither, since the rule
    /// the class grades is keyed on that one join.
    #[test]
    fn a_relational_chain_enumerates_both_shells() {
        let mut kinds: Vec<&str> = sites_of("const x = (a < b) > c;")
            .iter()
            .map(|s| s.kind.label())
            .collect();
        kinds.sort_unstable();
        assert_eq!(kinds, ["< > chain", "< > operand"]);
        assert!(sites_of("const x = a < b;").is_empty());
        assert!(sites_of("const x = a > b > c;").is_empty());
    }

    /// The exclusion reason of every relational site in `source`, in enumeration order —
    /// `None` for a site the run probes.
    fn exclusions_of(source: &str) -> Vec<Option<&'static str>> {
        let wire = tsv_parse_to_value(source, ParserType::TypeScript).expect("parses");
        sites_of(source)
            .iter()
            .filter(|s| s.kind.is_relational())
            .map(|s| {
                ParenAuditCommand::twin_exclusion_reason(source, s, &wire, ParserType::TypeScript)
            })
            .collect()
    }

    /// The two exclusion reasons answer different questions, so the audit has to tell them
    /// apart: `rejected` is tsv refusing a spelling the bare one accepts, `different tree` is
    /// both spellings parsing and the shell moving the reading.
    #[test]
    fn the_two_exclusion_reasons_are_named_apart() {
        // A shell whose content opens a type-argument region the bare twin does not, and
        // whose body then fails to parse as a type: tsv rejects the twin.
        assert!(exclusions_of("const g1 = p < keyof.a > (t, u);").contains(&Some("rejected")));
        // A shell whose content parses as a type moves the reading instead.
        assert!(
            exclusions_of("const g2 = p < readonly.a > (t, u);").contains(&Some("different tree"))
        );
        // A chain whose twin neither moves nor rejects is probed, so the exclusion is keyed
        // on the twin rather than on the class.
        assert_eq!(exclusions_of("const g3 = p < b() > (t, u);"), [None, None]);
    }
}
