//! Authoring-independence audit (Svelte boundary whitespace).
//!
//! Stronger than the corpus idempotency sweep (which only checks `format(x)` is
//! stable): this probes whether the *same logical document*, authored with
//! different boundary whitespace, formats to **one** tsv fixed point. A formatter
//! can be idempotent yet authoring-*dependent* — two authorings settling on two
//! different stable outputs.
//!
//! ## What it mutates (safe by construction)
//!
//! Two site families, both confined to fragments that are **not**
//! whitespace-significant (`<pre>`/`<textarea>`, via `tsv_html::preserves_whitespace`),
//! and neither ever touching a blank-line run (2+ newlines — Tier-1 significant):
//!
//! 1. **Between siblings** — an existing ASCII-whitespace run separating two nodes (a
//!    whitespace-only `Text` node, or the leading/trailing whitespace of a content
//!    `Text` node adjacent to a sibling). Inter-node whitespace is render-**significant**
//!    (it collapses to one space, it does not vanish), so the toggle is space ↔ single
//!    newline only: never inserting whitespace where none exists, never removing it.
//!    Both forms collapse to the same single inter-node space, so the mutation is
//!    semantics-preserving; the element *expansion* it may trigger is the layout change
//!    under test.
//!
//! 2. **At a fragment's content boundary** — the whitespace between a fragment's opening
//!    tag and its first child, or between its last child and the closing tag. Svelte has
//!    two fragment families and **both** boundaries are probed: an element's/component's
//!    content, and a **block branch's** body (`{#if}` / `{:else}` / `{#each}` / its
//!    `{:else}` fallback / each `{#await}` phase / `{#key}` / `{#snippet}`). This run is
//!    render-**free** under Svelte 5 (start/end-of-content whitespace is removed at
//!    compile: `<p>foo<span> - bar</span></p>` renders `foo- bar`), so here the run may
//!    be *created and destroyed*, not just reshaped: each boundary is probed at all
//!    three forms it can take — **hug (zero whitespace) ↔ space ↔ newline**. This is the
//!    family that catches a formatter letting a render-free character pick the layout.
//!    Restricted to elements whose content already spans lines in the base, because that is
//!    where layout is at stake. Be honest about what this excludes: when the content fits on
//!    one line, tsv *preserves* an authored boundary space (`<span> text </span>` and
//!    `<span>text</span>` are both stable), so it is authoring-DEPENDENT there too — on a
//!    character the compiler removes. That is a deliberate, prettier-matching preservation
//!    choice, pinned by the `inline_boundary_whitespace` fixture and cataloged in
//!    conformance_prettier.md §Svelte: Inline content block-style; it costs nothing
//!    structurally, and probing it would bury the layout signal this audit exists to find
//!    under a wall of sanctioned noise. So: a clean run means no render-free character picks
//!    a LAYOUT — not that none survives in the output.
//!
//! The enumeration reuses the parser's own node structure + `preserves_whitespace`
//! rather than re-deriving significance (audit *policy* lives here in the caller;
//! the significance *query* is the shared predicate).
//!
//! ## Buckets (with `--prettier`)
//!
//! Per site, the 2×2 of "does tsv converge?" × "does prettier converge?":
//! - **(a) BUG** — tsv diverges where prettier converges (or tsv is non-idempotent).
//! - **(b) PIN** — tsv converges where prettier diverges (the `space_after_block`
//!   class — a `_prettier_divergence` worth pinning).
//! - **(c) ---** — both diverge (sanctioned, e.g. Tier-2 element expansion); record.
//!   Each (c) carries a latent design question: *should* tsv converge here anyway?
//! - **clean** — both converge.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_svelte::ast::internal::FragmentNode;

use crate::cli::CliError;
use crate::deno::{PrettierParser, run_prettier};

use super::profile::resolve_files;

/// Audit Svelte boundary-whitespace authoring-independence.
///
/// Mutates whitespace that cannot change the render — space↔newline between siblings,
/// and hug↔space↔newline at a tag's content boundary — and checks tsv formats every
/// authoring to one fixed point. Pure Rust for the convergence / idempotency verdict;
/// `--prettier` adds the (a)/(b)/(c) triage via the sidecar. Defaults to
/// `tests/fixtures` when no paths are given. Svelte (`.svelte`) only.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "authoring_audit")]
pub struct AuthoringAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// run the prettier triage (splits divergences into a/b/c buckets)
    #[argh(switch)]
    prettier: bool,

    /// show per-site detail for the interesting (non-clean) sites
    #[argh(switch)]
    verbose: bool,

    /// max boundary sites probed per file (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// cap the number of examples retained per bucket (default 40)
    #[argh(option, default = "40")]
    examples: usize,

    /// write byte-exact repro artifacts (base/variant/ftry/ftry2) for every hard
    /// finding (non-idempotent, and — with --prettier — bucket-a) into this dir
    #[argh(option)]
    dump_dir: Option<String>,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// The kind of boundary the toggle site sits on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum SiteKind {
    /// Whitespace-only `Text` node between two siblings.
    WsOnly,
    /// Leading whitespace of a content `Text` node (a previous sibling exists).
    ContentLeading,
    /// Trailing whitespace of a content `Text` node (a next sibling exists).
    ContentTrailing,
    /// Between an element's opening tag and its first child — render-free, so probed
    /// at all three forms (hug / space / newline).
    BoundaryLeading,
    /// Between an element's last child and its closing tag — likewise.
    BoundaryTrailing,
    /// Between a block's opening/branch tag (`{#if …}`, `{:else}`, `{:then …}`, …) and
    /// the first child of that branch — render-free, same three forms.
    BlockBoundaryLeading,
    /// Between a block branch's last child and its closing/next-branch tag — likewise.
    BlockBoundaryTrailing,
}

impl SiteKind {
    fn label(self) -> &'static str {
        match self {
            Self::WsOnly => "ws-only",
            Self::ContentLeading => "content-leading",
            Self::ContentTrailing => "content-trailing",
            Self::BoundaryLeading => "boundary-leading",
            Self::BoundaryTrailing => "boundary-trailing",
            Self::BlockBoundaryLeading => "block-boundary-leading",
            Self::BlockBoundaryTrailing => "block-boundary-trailing",
        }
    }

    /// The ordered kind list, for the per-kind report breakdown.
    const ALL: [Self; 7] = [
        Self::WsOnly,
        Self::ContentLeading,
        Self::ContentTrailing,
        Self::BoundaryLeading,
        Self::BoundaryTrailing,
        Self::BlockBoundaryLeading,
        Self::BlockBoundaryTrailing,
    ];
}

/// A single toggle site: the byte range (in the formatted base `F`) of the
/// whitespace run, the current form, and what to splice in to flip it.
#[derive(Clone, Debug)]
struct Site {
    start: usize,
    end: usize,
    kind: SiteKind,
    had_newline: bool,
    flipped: &'static str,
}

/// Flip a whitespace run space↔single-newline. `None` for runs that must not be
/// toggled: empty, or carrying a blank line (2+ newlines — Tier-1 significant).
fn flip_run(run: &str) -> Option<(bool, &'static str)> {
    if run.is_empty() {
        return None;
    }
    match run.matches('\n').count() {
        0 => Some((false, "\n")),
        1 => Some((true, " ")),
        _ => None,
    }
}

/// The per-language site enumerator — parse the formatted source and collect its
/// safe toggle sites. This is the only language-specific seam: the `Site` type and
/// the audit driver (format variant → compare → triage → report → dump) are
/// language-agnostic. `None` on parse failure.
///
// TODO: TS/CSS enumerators are a planned followup — add `ts_sites` / `css_sites`
// (a JS object `{`→first-prop newline and a CSS `:`→value newline are *significant*
// triggers, not fragment-sibling boundaries, so the safe-toggle set differs) and
// dispatch here by the file's `ParserType`. They serve as a passing baseline: the
// idempotency invariant predicts those embedded triggers are each self-stable.
fn svelte_sites(f: &str) -> Option<Vec<Site>> {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(f, &arena).ok()?;
    let mut sites = Vec::new();
    collect_sites(root.fragment.nodes, f, false, &mut sites);
    Some(sites)
}

/// Recurse the fragment tree collecting safe toggle sites. `ws_sig` is set once
/// inside a `<pre>`/`<textarea>` subtree (whitespace is then literal — never
/// toggled).
fn collect_sites(nodes: &[FragmentNode<'_>], src: &str, ws_sig: bool, out: &mut Vec<Site>) {
    let len = nodes.len();
    for (i, node) in nodes.iter().enumerate() {
        if !ws_sig && let FragmentNode::Text(_) = node {
            let sp = node.span();
            let (s, e) = (sp.start_usize(), sp.end_usize());
            let raw = &src[s..e];
            if raw.trim_ascii().is_empty() {
                // Whitespace-only node: a site only when it sits *between* two
                // siblings (an edge run is a parent-boundary, skipped in v1).
                if i > 0
                    && i + 1 < len
                    && let Some((had_nl, flip)) = flip_run(raw)
                {
                    out.push(Site {
                        start: s,
                        end: e,
                        kind: SiteKind::WsOnly,
                        had_newline: had_nl,
                        flipped: flip,
                    });
                }
            } else {
                // Content text: its leading run is a boundary iff a previous
                // sibling exists; its trailing run iff a next sibling exists.
                let lead_len = raw.len() - raw.trim_start_matches(is_ascii_ws).len();
                if i > 0
                    && lead_len > 0
                    && let Some((had_nl, flip)) = flip_run(&raw[..lead_len])
                {
                    out.push(Site {
                        start: s,
                        end: s + lead_len,
                        kind: SiteKind::ContentLeading,
                        had_newline: had_nl,
                        flipped: flip,
                    });
                }
                let trail_len = raw.len() - raw.trim_end_matches(is_ascii_ws).len();
                if i + 1 < len
                    && trail_len > 0
                    && let Some((had_nl, flip)) = flip_run(&raw[raw.len() - trail_len..])
                {
                    out.push(Site {
                        start: e - trail_len,
                        end: e,
                        kind: SiteKind::ContentTrailing,
                        had_newline: had_nl,
                        flipped: flip,
                    });
                }
            }
        }
        recurse_children(node, src, ws_sig, out);
    }
}

fn is_ascii_ws(c: char) -> bool {
    c.is_ascii_whitespace()
}

/// The three forms an element's content boundary can take. All render identically
/// under Svelte 5 (the run is removed at compile), so all three are legal authorings
/// of one document — and none of them may select the layout.
const BOUNDARY_FORMS: [&str; 3] = ["", " ", "\n"];

/// Collect the content-boundary sites of one fragment — an element's content or a block
/// branch's body, which are the same thing structurally and equally render-free: the
/// (possibly empty) whitespace run between the opening tag and the first child, and
/// between the last child and the closing tag. Each is emitted once per *other* form in
/// [`BOUNDARY_FORMS`], so a hugged boundary gets probed with a space and a newline spliced
/// in, and vice versa. `kinds` is the (leading, trailing) pair to label them with.
///
/// Caller must have excluded whitespace-significant subtrees (`<pre>`/`<textarea>`),
/// where this run is literal content and the mutation would change the render.
fn collect_boundary_sites(
    nodes: &[FragmentNode<'_>],
    src: &str,
    kinds: (SiteKind, SiteKind),
    out: &mut Vec<Site>,
) {
    let (Some(first), Some(last)) = (nodes.first(), nodes.last()) else {
        return;
    };
    // An all-whitespace fragment is an EMPTY element, not content with a boundary. (Its
    // lone space — `<span> </span>` — is a deliberate, prettier-matching preservation.)
    let has_content = nodes
        .iter()
        .any(|n| !matches!(n, FragmentNode::Text(t) if t.raw(src).trim_ascii().is_empty()));
    if !has_content {
        return;
    }
    let content_start = first.span().start_usize();
    let content_end = last.span().end_usize();
    // Probe only where LAYOUT is at stake — content that already spans lines. See the
    // module doc: for content that fits on one line, both formatters deliberately keep an
    // authored boundary space, so probing it would only pile up sanctioned divergences.
    if !src[content_start..content_end].contains('\n') {
        return;
    }

    // The leading run lives inside the first child when that child is text; otherwise the
    // boundary is a hug and the run is the empty span at the content's start. Same, mirrored,
    // for the trailing run. The two can't overlap: the fragment has non-whitespace content
    // between them.
    let lead = match first {
        FragmentNode::Text(t) => {
            let raw = t.raw(src);
            raw.len() - raw.trim_start_matches(is_ascii_ws).len()
        }
        _ => 0,
    };
    push_boundary_forms(content_start, content_start + lead, src, kinds.0, out);

    let trail = match last {
        FragmentNode::Text(t) => {
            let raw = t.raw(src);
            raw.len() - raw.trim_end_matches(is_ascii_ws).len()
        }
        _ => 0,
    };
    push_boundary_forms(content_end - trail, content_end, src, kinds.1, out);
}

/// The (leading, trailing) kind pair for an element's content boundary.
const ELEMENT_BOUNDARY: (SiteKind, SiteKind) =
    (SiteKind::BoundaryLeading, SiteKind::BoundaryTrailing);

/// The (leading, trailing) kind pair for a block branch's body boundary.
const BLOCK_BOUNDARY: (SiteKind, SiteKind) = (
    SiteKind::BlockBoundaryLeading,
    SiteKind::BlockBoundaryTrailing,
);

/// Emit one site per alternative form of the boundary run `src[start..end]` (which may be
/// empty — a hugged boundary). A run carrying a blank line is Tier-1 significant and is
/// left alone, like everywhere else in this audit.
fn push_boundary_forms(start: usize, end: usize, src: &str, kind: SiteKind, out: &mut Vec<Site>) {
    let run = &src[start..end];
    let base = match run.matches('\n').count() {
        0 if run.is_empty() => "",
        0 => " ",
        1 => "\n",
        _ => return,
    };
    for form in BOUNDARY_FORMS {
        if form != base {
            out.push(Site {
                start,
                end,
                kind,
                had_newline: base == "\n",
                flipped: form,
            });
        }
    }
}

/// A block branch (`{#if}` consequent, `{:else}`, `{#each}` body / fallback, an
/// `{#await}` phase, `{#key}`, `{#snippet}`) is a fragment exactly like an element's
/// content, and its boundary is equally render-free — so it gets both site families.
/// Inside `<pre>`/`<textarea>` the run is literal, so only the inter-sibling family applies.
fn collect_block_fragment(
    fragment: &tsv_svelte::ast::internal::Fragment<'_>,
    src: &str,
    ws_sig: bool,
    out: &mut Vec<Site>,
) {
    if !ws_sig {
        collect_boundary_sites(fragment.nodes, src, BLOCK_BOUNDARY, out);
    }
    collect_sites(fragment.nodes, src, ws_sig, out);
}

/// Is this alternate the `{:else if …}` form — one nested `IfBlock` that spans the rest
/// of the chain, closing tag included — rather than a real `{:else}` fragment?
fn is_elseif_chain(alternate: &tsv_svelte::ast::internal::Fragment<'_>) -> bool {
    matches!(alternate.nodes, [FragmentNode::IfBlock(b)] if b.elseif)
}

/// Descend into a node's child fragments, propagating (and entering) whitespace
/// significance.
fn recurse_children(node: &FragmentNode<'_>, src: &str, ws_sig: bool, out: &mut Vec<Site>) {
    match node {
        FragmentNode::Element(el) => {
            let tag = el.name_span.extract(src).to_ascii_lowercase();
            let child_ws_sig = ws_sig || tsv_html::preserves_whitespace(&tag);
            // The content boundary is render-free only outside `<pre>`/`<textarea>`, where
            // it is literal content and the dangle is mandatory.
            if !child_ws_sig {
                collect_boundary_sites(el.fragment.nodes, src, ELEMENT_BOUNDARY, out);
            }
            collect_sites(el.fragment.nodes, src, child_ws_sig, out);
        }
        FragmentNode::SpecialElement(el) => {
            // No special element preserves whitespace (svelte:*, slot, title).
            // `<svelte:element this={tag}>` could resolve to <pre> at runtime, but
            // the tag is dynamic and unknowable statically; treat as non-pre
            // (the conservative miss is a `<svelte:element this="pre">` literal,
            // vanishingly rare and not worth a special case here).
            if !ws_sig {
                collect_boundary_sites(el.fragment.nodes, src, ELEMENT_BOUNDARY, out);
            }
            collect_sites(el.fragment.nodes, src, ws_sig, out);
        }
        FragmentNode::IfBlock(b) => {
            collect_block_fragment(&b.consequent, src, ws_sig, out);
            if let Some(alt) = &b.alternate {
                // An `{:else if}` chain is an alternate holding one nested `IfBlock` whose
                // span runs from `{:else if` through the closing `{/if}` — so that fragment
                // has no boundary of its own to probe (splicing at its edges would land
                // outside the block). Recurse only; the nested block's own branches are
                // probed when we reach it.
                if is_elseif_chain(alt) {
                    collect_sites(alt.nodes, src, ws_sig, out);
                } else {
                    collect_block_fragment(alt, src, ws_sig, out);
                }
            }
        }
        FragmentNode::EachBlock(b) => {
            collect_block_fragment(&b.body, src, ws_sig, out);
            if let Some(fb) = &b.fallback {
                collect_block_fragment(fb, src, ws_sig, out);
            }
        }
        FragmentNode::AwaitBlock(b) => {
            for frag in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                collect_block_fragment(frag, src, ws_sig, out);
            }
        }
        FragmentNode::KeyBlock(b) => collect_block_fragment(&b.fragment, src, ws_sig, out),
        FragmentNode::SnippetBlock(b) => collect_block_fragment(&b.body, src, ws_sig, out),
        // Leaf fragment nodes (no child fragment): text, comments, expression /
        // html / const / declaration / debug / render tags.
        _ => {}
    }
}

/// Per-site outcome from the tsv (and optional prettier) passes.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // flat result record; the bools are independent verdicts
struct Outcome {
    path: String,
    offset: usize,
    kind: SiteKind,
    had_newline: bool,
    tsv_converge: bool,
    /// Whether the *diverged* variant is itself a fixed point (only meaningful
    /// when `!tsv_converge`). `false` ⇒ hard non-idempotency.
    tsv_self_stable: bool,
    /// `Some(true)` = prettier maps both authorings to one output; `Some(false)`
    /// = prettier diverges; `None` = not triaged or prettier errored.
    prettier_converge: Option<bool>,
    prettier_error: bool,
    context: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Bucket {
    CleanBoth,
    BugA,
    NonIdempotent,
    PinB,
    SanctionedC,
    /// tsv converge/diverge known, prettier not triaged (pure-Rust mode).
    ConvergeUntriaged,
    DivergeUntriaged,
    PrettierError,
}

impl Outcome {
    fn bucket(&self) -> Bucket {
        if !self.tsv_converge && !self.tsv_self_stable {
            return Bucket::NonIdempotent;
        }
        match self.prettier_converge {
            None if self.prettier_error => Bucket::PrettierError,
            None if self.tsv_converge => Bucket::ConvergeUntriaged,
            None => Bucket::DivergeUntriaged,
            Some(p_conv) => match (self.tsv_converge, p_conv) {
                (true, true) => Bucket::CleanBoth,
                (true, false) => Bucket::PinB,
                (false, true) => Bucket::BugA,
                (false, false) => Bucket::SanctionedC,
            },
        }
    }
}

/// The aggregate result of a run.
#[derive(Default)]
struct Report {
    files_scanned: usize,
    files_parse_error: usize,
    files_base_non_idempotent: usize,
    sites: usize,
    variant_parse_errors: usize,
    counts: BTreeMap<Bucket, usize>,
    /// Per-site-kind bucket counts — the work-list view: which boundary family a
    /// divergence lives in.
    kind_counts: BTreeMap<(SiteKind, Bucket), usize>,
    examples: BTreeMap<Bucket, Vec<Outcome>>,
    base_non_idempotent_paths: Vec<String>,
    dump_seq: usize,
}

impl Report {
    fn count(&self, b: Bucket) -> usize {
        self.counts.get(&b).copied().unwrap_or(0)
    }

    fn kind_count(&self, k: SiteKind, b: Bucket) -> usize {
        self.kind_counts.get(&(k, b)).copied().unwrap_or(0)
    }
}

/// Write a byte-exact repro of a hard finding: the base `F`, the flipped
/// `variant`, tsv's `ftry`, and `ftry2` (= format(ftry), to expose a 2-cycle).
fn dump_case(dir: &str, seq: usize, tag: &str, src_path: &str, f: &str, variant: &str, ftry: &str) {
    let slug: String = Path::new(src_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("case")
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let case_dir = Path::new(dir).join(format!("{seq:03}_{tag}_{slug}"));
    if std::fs::create_dir_all(&case_dir).is_err() {
        return;
    }
    let ftry2 = format_source(ftry, ParserType::Svelte).unwrap_or_default();
    let note = format!(
        "source: {src_path}\nbucket: {tag}\nbase F (a fixed point) -> flip one boundary -> variant -> format = ftry\nftry == F?      {}\nftry idempotent? {}\n",
        ftry == f,
        ftry == ftry2,
    );
    let _ = std::fs::write(case_dir.join("base.svelte"), f);
    let _ = std::fs::write(case_dir.join("variant.svelte"), variant);
    let _ = std::fs::write(case_dir.join("ftry.svelte"), ftry);
    let _ = std::fs::write(case_dir.join("ftry2.svelte"), ftry2);
    let _ = std::fs::write(case_dir.join("note.txt"), note);
}

/// Stable display / JSON key for a bucket (the map is keyed by the `Bucket` enum;
/// this is only for human labels and machine-readable output).
fn bucket_key(b: Bucket) -> &'static str {
    match b {
        Bucket::CleanBoth => "clean",
        Bucket::BugA => "a_bug",
        Bucket::NonIdempotent => "a_non_idempotent",
        Bucket::PinB => "b_pin",
        Bucket::SanctionedC => "c_sanctioned",
        Bucket::ConvergeUntriaged => "converge",
        Bucket::DivergeUntriaged => "diverge_dual_stable",
        Bucket::PrettierError => "prettier_error",
    }
}

/// Is a bucket "interesting" enough to retain an example / show in verbose?
fn interesting(b: Bucket) -> bool {
    matches!(
        b,
        Bucket::BugA
            | Bucket::NonIdempotent
            | Bucket::PinB
            | Bucket::SanctionedC
            | Bucket::DivergeUntriaged
    )
}

impl AuthoringAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths.clone()
        };
        let files = match resolve_files(&paths) {
            Ok(f) => f.into_iter().filter(|p| is_svelte(p)).collect::<Vec<_>>(),
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };

        let report = if self.prettier {
            let rt = super::create_runtime();
            rt.block_on(self.scan_with_prettier(&files))
        } else {
            self.scan_pure(&files)
        };

        if self.json {
            print_json(&report);
        } else {
            print_human(&report, self.verbose, self.prettier);
        }

        // Exit non-zero on any hard finding (non-idempotency, or — when triaged —
        // an (a) bug). (c)/(b)/untriaged divergences are not gate failures here.
        let hard = report.count(Bucket::BugA) + report.count(Bucket::NonIdempotent);
        if hard > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }

    /// Pure-Rust pass: convergence + self-stability only (no prettier).
    fn scan_pure(&self, files: &[PathBuf]) -> Report {
        let mut report = Report::default();
        for path in files {
            let Some((f, sites)) = self.prepare_file(path, &mut report) else {
                continue;
            };
            for site in &sites {
                let Some(outcome) = self.tsv_outcome(path, &f, site, &mut report) else {
                    continue;
                };
                self.record(&mut report, outcome);
            }
        }
        report
    }

    /// Prettier-triaged pass: also classify each site against prettier.
    async fn scan_with_prettier(&self, files: &[PathBuf]) -> Report {
        let mut report = Report::default();
        for path in files {
            let Some((f, sites)) = self.prepare_file(path, &mut report) else {
                continue;
            };
            // Prettier's take on the base form, computed once per file.
            let prettier_f = run_prettier(&f, PrettierParser::Parser("svelte"))
                .await
                .ok();
            for site in &sites {
                let Some(mut outcome) = self.tsv_outcome(path, &f, site, &mut report) else {
                    continue;
                };
                let variant = splice(&f, site);
                match (
                    &prettier_f,
                    run_prettier(&variant, PrettierParser::Parser("svelte")).await,
                ) {
                    (Some(pf), Ok(pv)) => outcome.prettier_converge = Some(pf == &pv),
                    _ => outcome.prettier_error = true,
                }
                // Dump bucket-a bugs (tsv diverges where prettier converges).
                if let Some(dir) = &self.dump_dir
                    && outcome.bucket() == Bucket::BugA
                {
                    report.dump_seq += 1;
                    let ftry = format_source(&variant, ParserType::Svelte).unwrap_or_default();
                    dump_case(
                        dir,
                        report.dump_seq,
                        "bug_a",
                        &outcome.path,
                        &f,
                        &variant,
                        &ftry,
                    );
                }
                self.record(&mut report, outcome);
            }
        }
        report
    }

    /// Format the file, gate on parse / base-idempotency, and enumerate sites.
    fn prepare_file(&self, path: &Path, report: &mut Report) -> Option<(String, Vec<Site>)> {
        // Skip fixtures expected to fail parsing.
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("input_invalid"))
        {
            return None;
        }
        let source = std::fs::read_to_string(path).ok()?;
        report.files_scanned += 1;
        let Ok(f) = format_source(&source, ParserType::Svelte) else {
            report.files_parse_error += 1;
            return None;
        };
        // Base idempotency: a file whose own format isn't a fixed point has a more
        // fundamental bug; exclude it from authoring analysis (and flag it).
        match format_source(&f, ParserType::Svelte) {
            Ok(f2) if f2 == f => {}
            _ => {
                report.files_base_non_idempotent += 1;
                report
                    .base_non_idempotent_paths
                    .push(path.display().to_string());
                return None;
            }
        }
        let mut sites = svelte_sites(&f)?;
        if self.limit > 0 && sites.len() > self.limit {
            sites.truncate(self.limit);
        }
        report.sites += sites.len();
        Some((f, sites))
    }

    /// Compute the tsv-only outcome for one site (no prettier fields set).
    fn tsv_outcome(
        &self,
        path: &Path,
        f: &str,
        site: &Site,
        report: &mut Report,
    ) -> Option<Outcome> {
        let variant = splice(f, site);
        let Ok(ftry) = format_source(&variant, ParserType::Svelte) else {
            report.variant_parse_errors += 1;
            return None;
        };
        let tsv_converge = ftry == f;
        let tsv_self_stable = if tsv_converge {
            true
        } else {
            format_source(&ftry, ParserType::Svelte).is_ok_and(|x| x == ftry)
        };
        // Dump non-idempotent findings (always a hard bug) when requested.
        if let Some(dir) = &self.dump_dir
            && !tsv_converge
            && !tsv_self_stable
        {
            report.dump_seq += 1;
            dump_case(
                dir,
                report.dump_seq,
                "nonidem",
                &path.display().to_string(),
                f,
                &variant,
                &ftry,
            );
        }
        Some(Outcome {
            path: path.display().to_string(),
            offset: site.start,
            kind: site.kind,
            had_newline: site.had_newline,
            tsv_converge,
            tsv_self_stable,
            prettier_converge: None,
            prettier_error: false,
            context: line_context(f, site.start),
        })
    }

    fn record(&self, report: &mut Report, outcome: Outcome) {
        let bucket = outcome.bucket();
        *report.counts.entry(bucket).or_default() += 1;
        *report
            .kind_counts
            .entry((outcome.kind, bucket))
            .or_default() += 1;
        if interesting(bucket) {
            let slot = report.examples.entry(bucket).or_default();
            if slot.len() < self.examples {
                slot.push(outcome);
            }
        }
    }
}

fn is_svelte(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()) == Some("svelte")
}

/// Build a variant of `f` with one site's whitespace run flipped.
fn splice(f: &str, site: &Site) -> String {
    let mut out = String::with_capacity(f.len());
    out.push_str(&f[..site.start]);
    out.push_str(site.flipped);
    out.push_str(&f[site.end..]);
    out
}

/// The (trimmed) source line containing byte `offset`, for human context.
fn line_context(f: &str, offset: usize) -> String {
    let start = f[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = f[offset..].find('\n').map_or(f.len(), |i| offset + i);
    let line = f[start..end].trim();
    let truncated: String = line.chars().take(80).collect();
    truncated
}

fn print_human(report: &Report, verbose: bool, triaged: bool) {
    println!("Authoring-independence audit (Svelte boundary whitespace)");
    println!(
        "  files: {} scanned, {} parse-error, {} base-non-idempotent",
        report.files_scanned, report.files_parse_error, report.files_base_non_idempotent,
    );
    println!(
        "  sites probed: {} ({} variant parse-errors)",
        report.sites, report.variant_parse_errors,
    );
    let c = |b: Bucket| report.count(b);
    println!();
    if triaged {
        println!("  triage vs prettier:");
        println!(
            "    clean (both converge):                  {}",
            c(Bucket::CleanBoth)
        );
        println!(
            "    (a) BUG  tsv diverges, prettier converges: {}",
            c(Bucket::BugA)
        );
        println!(
            "    (a) BUG  tsv NON-IDEMPOTENT:               {}",
            c(Bucket::NonIdempotent)
        );
        println!(
            "    (b) PIN  tsv converges, prettier diverges: {}",
            c(Bucket::PinB)
        );
        println!(
            "    (c) ---  both diverge (sanctioned):        {}",
            c(Bucket::SanctionedC)
        );
        println!(
            "    prettier error (untriaged):               {}",
            c(Bucket::PrettierError)
        );
    } else {
        println!("  pure-Rust verdict:");
        println!(
            "    converge:                  {}",
            c(Bucket::ConvergeUntriaged)
        );
        println!(
            "    diverge (dual-stable):     {}",
            c(Bucket::DivergeUntriaged)
        );
        println!(
            "    diverge (NON-IDEMPOTENT):  {}",
            c(Bucket::NonIdempotent)
        );
    }

    println!();
    println!("  by site kind (which boundary family a divergence lives in):");
    let buckets: &[Bucket] = if triaged {
        &[
            Bucket::CleanBoth,
            Bucket::BugA,
            Bucket::NonIdempotent,
            Bucket::PinB,
            Bucket::SanctionedC,
        ]
    } else {
        &[
            Bucket::ConvergeUntriaged,
            Bucket::DivergeUntriaged,
            Bucket::NonIdempotent,
        ]
    };
    print!("    {:<24}", "kind");
    for b in buckets {
        print!("{:>20}", bucket_key(*b));
    }
    println!();
    for kind in SiteKind::ALL {
        let total: usize = buckets.iter().map(|b| report.kind_count(kind, *b)).sum();
        if total == 0 {
            continue;
        }
        print!("    {:<24}", kind.label());
        for b in buckets {
            print!("{:>20}", report.kind_count(kind, *b));
        }
        println!();
    }

    if !report.base_non_idempotent_paths.is_empty() {
        println!();
        println!("  base-non-idempotent files (pre-existing, excluded):");
        for p in report.base_non_idempotent_paths.iter().take(20) {
            println!("    {p}");
        }
    }

    if verbose {
        for (bucket, list) in &report.examples {
            if list.is_empty() {
                continue;
            }
            println!();
            println!("  [{}] examples:", bucket_key(*bucket));
            for o in list {
                println!(
                    "    {}:{}  {} ({})  «{}»",
                    o.path,
                    o.offset,
                    o.kind.label(),
                    if o.had_newline {
                        "newline→space"
                    } else {
                        "space→newline"
                    },
                    o.context,
                );
            }
        }
    }
}

fn print_json(report: &Report) {
    let counts: serde_json::Map<String, serde_json::Value> = report
        .counts
        .iter()
        .map(|(k, v)| (bucket_key(*k).to_string(), serde_json::json!(v)))
        .collect();
    let examples: serde_json::Map<String, serde_json::Value> = report
        .examples
        .iter()
        .map(|(k, list)| {
            let arr: Vec<serde_json::Value> = list
                .iter()
                .map(|o| {
                    serde_json::json!({
                        "path": o.path,
                        "offset": o.offset,
                        "kind": o.kind.label(),
                        "direction": if o.had_newline { "newline_to_space" } else { "space_to_newline" },
                        "tsv_converge": o.tsv_converge,
                        "tsv_self_stable": o.tsv_self_stable,
                        "prettier_converge": o.prettier_converge,
                        "context": o.context,
                    })
                })
                .collect();
            (bucket_key(*k).to_string(), serde_json::Value::Array(arr))
        })
        .collect();
    let kind_counts: serde_json::Map<String, serde_json::Value> = SiteKind::ALL
        .iter()
        .map(|kind| {
            let per_bucket: serde_json::Map<String, serde_json::Value> = report
                .kind_counts
                .iter()
                .filter(|((k, _), _)| k == kind)
                .map(|((_, b), n)| (bucket_key(*b).to_string(), serde_json::json!(n)))
                .collect();
            (
                kind.label().to_string(),
                serde_json::Value::Object(per_bucket),
            )
        })
        .collect();
    let out = serde_json::json!({
        "files_scanned": report.files_scanned,
        "files_parse_error": report.files_parse_error,
        "files_base_non_idempotent": report.files_base_non_idempotent,
        "sites": report.sites,
        "variant_parse_errors": report.variant_parse_errors,
        "counts": counts,
        "kind_counts": kind_counts,
        "examples": examples,
        "base_non_idempotent_paths": report.base_non_idempotent_paths,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}
