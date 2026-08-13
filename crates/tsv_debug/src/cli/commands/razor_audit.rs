use argh::FromArgs;
use std::collections::BTreeSet;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::{
    is_format_ignore_directive, is_format_ignore_range_end, is_format_ignore_range_start,
};
use tsv_svelte::ast::internal::{
    AttributeNode, AttributeValue, Element, Fragment, FragmentNode, TextDecoding,
};

use crate::audit::vacuity::{FIXTURES_FORMATTED_MIN, check_formatted_min, check_graded_nonzero};
use crate::cli::CliError;

use super::profile::{is_input_invalid_fixture, is_svelte, resolve_seed_files_named};

/// Walk each Svelte seed ACROSS the print-width razor and grade the output at every width.
///
/// **The dimension no other gate varies.** `authoring_audit` mutates the *spelling* of a
/// document's whitespace, `fuzz_audit` mutates its *structure*, `blank`/`gap` inject at
/// *sites* — but every one of them formats each document at the width its content happens to
/// have. This family's bugs are width-keyed: a layout rule fires only once a construct
/// crosses column 100, so a document one character short of the razor exercises none of it.
/// This audit supplies that variation by *padding a text word*, which shifts everything
/// downstream of it by `k` columns, so `--width k` formats each seed at `k + 1` distinct
/// geometries instead of one.
///
/// **Two properties are graded at each width, and neither subsumes the other.**
///
/// - **F1** (`format(out) == out`) catches the half where the two authorings of a document
///   disagree forever. That is the half a width sweep alone would find.
/// - **The line-head boundary space** ([`line_head_boundary_spaces`]) catches the half F1
///   cannot see. When a text run's leading boundary is baked into its first word rather than
///   claimed as a break point, the space rides the fill's fresh-line drop to the head of a
///   continuation line — and after a predecessor whose break is *forced* (a tag, a component,
///   a `svelte:*`), that mangled form keeps its own break under every authoring, so it is its
///   **own fixed point**. F1, the fuzzer and the round-trip all pass straight through it; only
///   a column separates the two forms. That is exactly how the dropped-tail bug survived every
///   gate in the repo and fell only to a hand-run ±1-char sweep.
///
/// **The oracle must be structural, and that is measured, not assumed.** A raw text scan for
/// "line starts with indent + a space" is unusable: 406 lines across the fixture tree's own
/// *output* files already do, dominated by block-comment continuations (` *`, ` */`),
/// expression alignment (` )`, ` }`), multiline attribute values and `<pre>` content — all
/// legitimate. So the scan asks the **parse**: a violation is a space at a line head *inside a
/// fragment `Text` node*, which excludes every one of those by node kind.
///
/// Its first catch was a live F1 break in the fused element+tail measurement that the
/// inline-sibling wrap used to take — invisible to every other gate because the strayed pass is
/// only reachable at widths no fixture happened to sit at
/// (`inline_sibling_drop_tail_wide_long`). Green since, and gated in `deno task check`.
///
/// Pure Rust — no Deno. Defaults to `tests/fixtures`, `.svelte` seeds only (the class is
/// Svelte inline layout; the TS and CSS printers have no fill-boundary bake).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "razor_audit")]
pub struct RazorAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// how many columns to sweep each site across (default 24)
    #[argh(option, default = "24")]
    width: usize,

    /// stop after N seed files (0 = no limit)
    #[argh(option, default = "0")]
    limit: usize,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// What a graded output was found guilty of.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    /// A collapsible space at a line head inside a fragment `Text` — the fixed-point half.
    StraySpace,
    /// `format(output) != output` at this width — the F1-visible half.
    NonIdempotent,
    /// The output does not reparse. Absolute: no width may produce it.
    Unreparseable,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::StraySpace => "STRAY-SPACE",
            Self::NonIdempotent => "NON-IDEMPOTENT",
            Self::Unreparseable => "UNREPARSEABLE",
        }
    }
}

/// One graded width that failed, with the reproducer.
struct Finding {
    kind: Kind,
    path: PathBuf,
    /// Columns of padding this mutant carried (0 = the pristine format).
    pad: usize,
    /// Byte offset in the *formatted seed* where the padding was inserted.
    site: usize,
    /// 1-based output line the finding sits on.
    line: usize,
    /// The offending output line, as emitted.
    text: String,
}

/// The deduped shape of a finding — what a report groups by, path-free so it reads the same
/// on any corpus.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Shape {
    kind: Kind,
    /// The offending line with its indent and its content words normalized away, so two
    /// findings that differ only in prose read as one shape.
    outline: String,
}

/// What one corpus walk produced.
struct Sweep {
    findings: Vec<Finding>,
    shapes: BTreeSet<Shape>,
    /// Seeds whose pristine format succeeded — the count the verdict rests on.
    formatted: usize,
    /// Seeds skipped (unreadable, `input_invalid_*`, parse failure, panic).
    skipped: usize,
    /// Distinct (seed, site, width) geometries actually formatted and graded.
    graded: usize,
}

impl RazorAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let files =
            resolve_seed_files_named(&self.paths, self.limit, "`.svelte` files", is_svelte)?;
        let sweep = sweep_files(&files, self.width);

        if self.json {
            print_json(&sweep);
        } else {
            print_report(&sweep, self.width);
        }

        check_graded_nonzero(sweep.graded, "widths graded")?;
        if default_paths {
            check_formatted_min(sweep.formatted, FIXTURES_FORMATTED_MIN)?;
        }

        if sweep.findings.is_empty() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

/// Format under `catch_unwind`, so a printer panic on a mutant is a skip rather than a dead run.
fn try_format(source: &str) -> Option<String> {
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        format_source(source, ParserType::Svelte)
    }))
    .ok()?
    .ok()
}

fn sweep_files(files: &[PathBuf], width: usize) -> Sweep {
    let mut sweep = Sweep {
        findings: Vec::new(),
        shapes: BTreeSet::new(),
        formatted: 0,
        skipped: 0,
        graded: 0,
    };
    for path in files {
        if is_input_invalid_fixture(path) {
            sweep.skipped += 1;
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            sweep.skipped += 1;
            continue;
        };
        // The seed is graded from its own FIXED POINT, not from the bytes on disk: padding a
        // formatted document perturbs one known geometry, where padding an arbitrary authoring
        // would confound the width sweep with whatever reflow the first format performs.
        let Some(base) = try_format(&source) else {
            sweep.skipped += 1;
            continue;
        };
        sweep.formatted += 1;

        // Width 0 — the seed's natural geometry. Free, and the only width a corpus normally
        // exercises.
        grade(
            &mut sweep,
            Mutant {
                path,
                pad: 0,
                site: 0,
            },
            &base,
        );

        for site in pad_sites(&base) {
            for pad in 1..=width {
                let mut mutant = String::with_capacity(base.len() + pad);
                mutant.push_str(&base[..site]);
                for _ in 0..pad {
                    mutant.push('x');
                }
                mutant.push_str(&base[site..]);
                let Some(out) = try_format(&mutant) else {
                    continue;
                };
                grade(&mut sweep, Mutant { path, pad, site }, &out);
            }
        }
    }
    sweep
}

/// The one geometry being graded: which seed, padded by how much, where. Carried as a unit
/// because the three travel together through every recording site and mean nothing apart —
/// passing them positionally put five same-typed `usize`s in a row at the call site.
#[derive(Clone, Copy)]
struct Mutant<'a> {
    path: &'a Path,
    /// Columns of padding (0 = the seed's own fixed point, graded at its natural width).
    pad: usize,
    /// Byte offset in the formatted seed where the padding was inserted.
    site: usize,
}

/// Grade one formatted output against both properties.
fn grade(sweep: &mut Sweep, mutant: Mutant<'_>, out: &str) {
    sweep.graded += 1;

    for (line, text) in line_head_boundary_spaces(out) {
        sweep.record(Kind::StraySpace, mutant, line, text);
    }

    // F1 at this width. A second format that fails to parse its own predecessor's output is a
    // distinct, absolute failure — no width may produce output tsv rejects.
    match try_format(out) {
        None => {
            let (line, text) = first_line(out);
            sweep.record(Kind::Unreparseable, mutant, line, text);
        }
        Some(second) if second != out => {
            let (line, text) = first_diff_line(out, &second);
            sweep.record(Kind::NonIdempotent, mutant, line, text);
        }
        Some(_) => {}
    }
}

impl Sweep {
    /// Record one finding and the shape it contributes to the report's grouping.
    fn record(&mut self, kind: Kind, mutant: Mutant<'_>, line: usize, text: String) {
        self.shapes.insert(Shape {
            kind,
            outline: outline(&text),
        });
        self.findings.push(Finding {
            kind,
            path: mutant.path.to_path_buf(),
            pad: mutant.pad,
            site: mutant.site,
            line,
            text,
        });
    }
}

/// Normalize a line to its shape: indent depth + the markup/word skeleton, so two findings
/// differing only in prose collapse to one entry.
fn outline(text: &str) -> String {
    let indent = text.len() - text.trim_start_matches('\t').len();
    let body: String = text
        .trim_start_matches('\t')
        .split_whitespace()
        .map(|w| {
            // Markup survives verbatim; prose collapses to `W`, so two findings differing only
            // in their words read as one shape.
            if w.starts_with('<') || w.starts_with('{') {
                w.to_string()
            } else {
                "W".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("indent{indent} {body}")
}

/// Byte offsets in `base` where a padding run may be inserted: the end of the FIRST word of
/// every eligible fragment text run.
///
/// One site per run rather than one per word — padding a word shifts *everything downstream of
/// it*, so the first word of each run already sweeps that run and all its successors across the
/// razor, and per-word sites would multiply the run count for geometries the run's own site
/// already reaches.
fn pad_sites(base: &str) -> Vec<usize> {
    let mut sites = Vec::new();
    for_each_eligible_text(base, &mut |raw: &str, start: usize| {
        // The first maximal run of non-collapsible-whitespace characters.
        let lead = raw.len() - raw.trim_start_matches(is_collapsible).len();
        let word_len = raw[lead..].find(is_collapsible).unwrap_or(raw.len() - lead);
        if word_len > 0 {
            sites.push(start + lead + word_len);
        }
    });
    sites.sort_unstable();
    sites.dedup();
    sites
}

/// Every space sitting at the head of an output line **inside a fragment `Text` node** —
/// returned as `(byte offset, 1-based line, the line's text)`.
///
/// The signature of the bug class: a text run's leading boundary space was baked into word 0
/// instead of being claimed as the run's own break point, so it travelled to the head of a
/// continuation line. The space is render-free (Svelte collapses the whole run to one space
/// either way), which is precisely why no render oracle, and no idempotency check, can see it.
///
/// Node kind is the whole discriminator — see the command doc for the measurement that forced
/// it. `<pre>`/`<textarea>` content, raw-content element text, attribute values, comment
/// bodies and format-ignored regions are all excluded structurally rather than by pattern.
fn line_head_boundary_spaces(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for_each_eligible_text(source, &mut |raw: &str, start: usize| {
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != b'\n' {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] == b'\t' {
                j += 1;
            }
            // A space here sits at a line head. A following newline would make it trailing
            // whitespace, which the renderer trims — so it cannot be this bug.
            if j < bytes.len() && bytes[j] == b' ' && bytes.get(j + 1) != Some(&b'\n') {
                out.push(line_at(source, start + j));
            }
            i = j.max(i + 1);
        }
    });
    out
}

/// Walk every fragment `Text` node whose whitespace the formatter OWNS, calling `f(raw, start)`.
///
/// The exclusions mirror the printer's own verbatim-emission dispatch (`build_element_doc`'s
/// leading arms) rather than re-deriving a rule of their own — an audit that excluded a
/// *different* set from the one the printer emits verbatim would either accuse the author's
/// bytes or blind itself to the printer's:
///
/// - **`preserve`** — inside `<pre>`/`<textarea>` boundary whitespace is literal content, and
///   those elements are dispatched to `build_whitespace_sensitive_element_doc` before any of
///   the layout this audit grades ever runs.
/// - **raw content** — `<script>`/`<style>`, and a `<template>` in a foreign language
///   ([`foreign_template_lang`]), whose bodies the printer emits verbatim.
/// - **decoding** — a `Text` that is not [`TextDecoding::Fragment`] is raw-content element text
///   or an attribute value.
/// - **format-ignore** — both the *node* form (`<!-- prettier-ignore -->` freezes the next
///   node) and the *range* form (`<!-- prettier-ignore-start -->` … `-end`), which freezes
///   every sibling between the markers. Missing the range form was a real false-positive class
///   caught on the first run: it accused 476 lines of the author's own frozen bytes.
fn for_each_eligible_text(source: &str, f: &mut dyn FnMut(&str, usize)) {
    fn walk(frag: &Fragment<'_>, source: &str, preserve: bool, f: &mut dyn FnMut(&str, usize)) {
        let mut frozen = false;
        let mut in_frozen_range = false;
        for node in frag.nodes {
            // The node freeze applies to the next node that is not whitespace-only text,
            // mirroring `format_ignore_raw_doc`; the range freeze spans every sibling between
            // its markers.
            let was_frozen = frozen || in_frozen_range;
            match node {
                FragmentNode::Comment(c) => {
                    let content = c.content(source);
                    if is_format_ignore_range_start(content) {
                        in_frozen_range = true;
                        continue;
                    }
                    if is_format_ignore_range_end(content) {
                        in_frozen_range = false;
                        continue;
                    }
                    if is_format_ignore_directive(content) {
                        frozen = true;
                        continue;
                    }
                }
                FragmentNode::Text(t) if t.is_collapsible_ws_only && was_frozen => continue,
                _ => {}
            }
            frozen = false;
            // A frozen node contributes nothing and is not descended into — its bytes are the
            // author's. Hoisted here rather than repeated as `!was_frozen` in every arm below.
            if was_frozen {
                continue;
            }

            match node {
                FragmentNode::Text(t) => {
                    if !preserve && t.decoding == TextDecoding::Fragment {
                        f(t.raw(source), t.raw_span.start as usize);
                    }
                }
                FragmentNode::Element(e) => {
                    if !emits_verbatim_body(e, source) {
                        let child_preserve =
                            preserve || tsv_html::preserves_whitespace(e.name(source));
                        walk(&e.fragment, source, child_preserve, f);
                    }
                }
                FragmentNode::SpecialElement(e) => walk(&e.fragment, source, preserve, f),
                FragmentNode::IfBlock(b) => {
                    walk(&b.consequent, source, preserve, f);
                    if let Some(alt) = &b.alternate {
                        walk(alt, source, preserve, f);
                    }
                }
                FragmentNode::EachBlock(b) => {
                    walk(&b.body, source, preserve, f);
                    if let Some(fallback) = &b.fallback {
                        walk(fallback, source, preserve, f);
                    }
                }
                FragmentNode::AwaitBlock(b) => {
                    for frag in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                        walk(frag, source, preserve, f);
                    }
                }
                FragmentNode::KeyBlock(b) => walk(&b.fragment, source, preserve, f),
                FragmentNode::SnippetBlock(b) => walk(&b.body, source, preserve, f),
                FragmentNode::Comment(_)
                | FragmentNode::ExpressionTag(_)
                | FragmentNode::HtmlTag(_)
                | FragmentNode::ConstTag(_)
                | FragmentNode::DeclarationTag(_)
                | FragmentNode::DebugTag(_)
                | FragmentNode::RenderTag(_) => {}
            }
        }
    }

    let arena = bumpalo::Bump::new();
    let Ok(root) = tsv_svelte::parse(source, &arena) else {
        return;
    };
    walk(&root.fragment, source, false, f);
}

/// Whether the printer emits this element's body **verbatim** — `<script>` / `<style>` (raw
/// text) or a foreign-language `<template>`. Mirrors the two leading arms of
/// `build_element_doc`: the body is the author's bytes, so its whitespace is never the
/// formatter's to answer for.
fn emits_verbatim_body(element: &Element<'_>, source: &str) -> bool {
    let name = element.name(source);
    name == "script" || name == "style" || foreign_template_lang(element, source)
}

/// Whether this is a `<template>` in a language other than HTML — the printer's
/// `build_foreign_template_doc` condition (`is_template && lang.is_some_and(|l| l != "html")`),
/// reading `lang` / `type` the same way `Printer::get_lang_attribute` does.
fn foreign_template_lang(element: &Element<'_>, source: &str) -> bool {
    if element.name(source) != "template" {
        return false;
    }
    element.attributes.iter().any(|node| {
        let AttributeNode::Attribute(attr) = node else {
            return false;
        };
        let name = attr.name(source);
        if name != "lang" && name != "type" {
            return false;
        }
        attr.value.is_some_and(|parts| {
            parts.iter().any(|part| {
                let AttributeValue::Text(text) = part else {
                    return false;
                };
                let lang = text.raw(source).trim();
                lang.strip_prefix("text/").unwrap_or(lang) != "html"
            })
        })
    })
}

fn is_collapsible(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// 1-based line number and the full text of the line containing `offset`.
fn line_at(source: &str, offset: usize) -> (usize, String) {
    let line = source[..offset].matches('\n').count() + 1;
    let start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = source[start..]
        .find('\n')
        .map_or(source.len(), |i| start + i);
    (line, source[start..end].to_string())
}

fn first_line(source: &str) -> (usize, String) {
    (1, source.lines().next().unwrap_or_default().to_string())
}

/// The first line at which two formats disagree — the F1 break's location.
fn first_diff_line(a: &str, b: &str) -> (usize, String) {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return (i + 1, la.to_string());
        }
    }
    let n = a.lines().count().min(b.lines().count());
    (n + 1, a.lines().nth(n).unwrap_or_default().to_string())
}

fn print_report(sweep: &Sweep, width: usize) {
    println!("Razor audit — width sweep over the print-width boundary");
    println!(
        "  {} seeds formatted, {} skipped, {} widths graded (±{width} columns per site)",
        sweep.formatted, sweep.skipped, sweep.graded
    );

    if sweep.findings.is_empty() {
        println!("\n✓ no findings — F1 and the line-head boundary hold at every swept width");
        return;
    }

    println!(
        "\n{} findings, {} shapes:",
        sweep.findings.len(),
        sweep.shapes.len()
    );
    for shape in &sweep.shapes {
        println!("  {:<16} {}", shape.kind.label(), shape.outline);
    }

    println!("\nReproducers (first 20):");
    for f in sweep.findings.iter().take(20) {
        println!(
            "  {:<16} {}:{} (pad {} at byte {})\n      {:?}",
            f.kind.label(),
            f.path.display(),
            f.line,
            f.pad,
            f.site,
            f.text
        );
    }
}

fn print_json(sweep: &Sweep) {
    let findings: Vec<_> = sweep
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "kind": f.kind.label(),
                "path": f.path.display().to_string(),
                "pad": f.pad,
                "site": f.site,
                "line": f.line,
                "text": f.text,
            })
        })
        .collect();
    let value = serde_json::json!({
        "formatted": sweep.formatted,
        "skipped": sweep.skipped,
        "graded": sweep.graded,
        "shapes": sweep.shapes.iter().map(|s| format!("{}\t{}", s.kind.label(), s.outline)).collect::<Vec<_>>(),
        "findings": findings,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    //! The oracle is the load-bearing half of this audit and it grades tsv's own output, so on a
    //! healthy tree it reports nothing — which means a regression that blinded it would look
    //! exactly like a clean run. These pin both directions on synthetic sources: the shape it must
    //! flag, and one case per structural exclusion it must not.

    use super::line_head_boundary_spaces;

    fn lines(source: &str) -> Vec<String> {
        line_head_boundary_spaces(source)
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    #[test]
    fn flags_a_space_at_a_line_head_inside_fragment_text() {
        // The bug's signature: a boundary space baked into word 0 rode the fill's fresh-line drop
        // to the head of a continuation line.
        let found = lines("<p>\n\ttext1\n\t text2\n</p>\n");
        assert_eq!(found.len(), 1, "expected one finding, got {found:?}");
        assert_eq!(found[0], "\t text2");
    }

    #[test]
    fn ignores_a_line_head_space_in_preserved_whitespace_content() {
        // `<pre>`/`<textarea>` content is literal — the printer never owns that space.
        assert!(lines("<pre>\n\ttext1\n\t text2\n</pre>\n").is_empty());
        assert!(lines("<textarea>\n\ta\n\t b\n</textarea>\n").is_empty());
    }

    #[test]
    fn ignores_a_line_head_space_in_a_verbatim_body() {
        // `<script>`/`<style>` and a foreign-language `<template>` are emitted verbatim — the
        // arms `build_element_doc` dispatches before any layout this audit grades.
        assert!(lines("<template lang=\"pug\">\n\th1 Title\n\t\tp Hey\n</template>\n").is_empty());
        assert!(
            lines("<div>\n\t<style>\n\t\ta {\n\t\t color: red;\n\t\t}\n\t</style>\n</div>\n")
                .is_empty()
        );
    }

    #[test]
    fn ignores_a_format_ignored_region_in_both_directive_forms() {
        // The author's frozen bytes, not the formatter's. Missing the RANGE form was a real
        // false-positive class on this audit's first run.
        assert!(lines("<!-- prettier-ignore -->\n<p>\n\ta\n\t b\n</p>\n").is_empty());
        assert!(
            lines("<!-- prettier-ignore-start -->\n<p>\n\ta\n\t b\n</p>\n<!-- prettier-ignore-end -->\n")
                .is_empty()
        );
    }

    #[test]
    fn a_frozen_range_reopens_after_its_end_marker() {
        // The range freeze must not leak past `-end`, or every later finding is silently dropped.
        let found = lines(
            "<!-- prettier-ignore-start -->\n<p>\n\ta\n\t b\n</p>\n<!-- prettier-ignore-end -->\n<p>\n\tc\n\t d\n</p>\n",
        );
        assert_eq!(found, vec!["\t d".to_string()]);
    }

    #[test]
    fn ignores_trailing_whitespace_and_non_collapsible_boundaries() {
        // A space before a newline is trailing whitespace the renderer trims, and a non-breaking
        // space is content rather than a collapsible boundary — neither is this bug.
        assert!(lines("<p>\n\ttext1 \n</p>\n").is_empty());
        assert!(lines("<p>\n\ttext1\n\t\u{a0}text2\n</p>\n").is_empty());
    }
}
