use crate::cli::CliError;
use crate::fixtures;
use argh::FromArgs;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// Audit doc/fixture integrity: divergence cataloging, link resolution, README hygiene.
///
/// Runs four checks over one fixture walk, reading each file at most once:
///
/// 1. **Orphans** — every `_*_divergence`-suffixed fixture must be linked in the doc
///    that sanctions its claim (`_prettier_divergence` → `docs/conformance_prettier.md`,
///    `_svelte_divergence` → `docs/conformance_svelte.md`, both for the combined suffix).
///    A divergence suffix asserts a deliberate difference; that claim must be cataloged
///    so it is discoverable and reviewable.
/// 2. **Dead links** — every Markdown link (relative path and `#anchor`) in the two
///    conformance docs, in the compiler doc pair ([`LINK_CHECKED_DOCS`]), and in
///    every fixture README must resolve on disk. The orphan
///    check only proves *forward* coverage (live fixture → mentioned in doc); this is
///    the *reverse* direction — a link to a renamed/demoted/deleted fixture, or a
///    back-link with the wrong `../` depth or a stale anchor, is otherwise invisible.
/// 3. **Missing back-links** — every `_*_divergence` fixture's README must *contain* a
///    link that resolves to the doc that sanctions its claim (`_prettier_divergence` →
///    `docs/conformance_prettier.md`, `_svelte_divergence` → `docs/conformance_svelte.md`,
///    both for the combined suffix). Checks 1+2 prove the doc catalogs the fixture and
///    that any link present resolves, but neither requires the back-link to *exist* — a
///    README that simply omits it passes both. This closes that gap: the README→doc link
///    is mandatory, not just well-formed-if-present. (A divergence fixture with no README
///    at all is the fixture validator's `D1` rule, run separately in `fixtures_validate`.)
/// 4. **Stray READMEs** — a non-divergence fixture (matches both Prettier and Svelte)
///    should not carry a README; there is no divergence to sanction, and any conformance
///    back-link it holds rots unaudited. A small allowlist (`ALLOWED_NONDIVERGENCE_READMES`)
///    holds the deliberate exceptions that document a real parser/spec/contrast fact.
///
/// Exits non-zero on any finding. Part of `deno task check`.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "conformance_audit")]
pub struct ConformanceAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

const CONFORMANCE_PRETTIER: &str = "docs/conformance_prettier.md";
const CONFORMANCE_SVELTE: &str = "docs/conformance_svelte.md";

/// Docs that participate in the dead-link check only — they sanction no fixture
/// suffix, so the orphan and back-link checks have nothing to ask of them, but
/// their relative paths and cross-doc anchors rot exactly like the others'. The
/// compiler pair cross-links each other, the checklists, and repo-root
/// `CLAUDE.md`, and nothing else was resolving those.
///
/// Only Markdown *links* (`[text](target)`) are checked; a backticked path such
/// as `` `../../svelte/packages/…` `` is a code span, not a link, so the
/// out-of-repo source references these docs cite are never visited.
const LINK_CHECKED_DOCS: &[&str] = &[
    "docs/conformance_svelte_compiler.md",
    "docs/checklist_svelte_compiler.md",
];

/// Non-divergence fixtures that deliberately keep a README because it documents a
/// real parser/spec/contrast fact that cannot live as an `input.*` comment. Every
/// entry is a conscious exception to the "matching fixtures carry no README" rule;
/// adding one is a review decision, not a default.
const ALLOWED_NONDIVERGENCE_READMES: &[&str] = &[
    // JS-vs-TS contrast: documents why the JSDoc-cast parens are *preserved* (matching
    // prettier's babel path) here, pointing at the TS-context divergence sibling.
    "typescript/calls/arrow_jsdoc_cast_body_long",
    "typescript/syntax/comments/jsdoc_type_cast_svelte",
    // Parser-behavior note about content deliberately *excluded* from the fixture.
    "svelte/syntax/entities/numeric_hex",
    "css/tokens/escapes/type_selector_escaped",
    // CSS spec edge cases (digit-count truncation boundary).
    "css/tokens/escapes/unicode_6_digits",
    "css/tokens/escapes/unicode_7_digits",
    // Comment-attachment behavior + contrast with the body-less divergence sibling.
    "typescript/declarations/function/type_params_paren_comment",
    // Empty-brace-drop matching behavior + idempotency note + `basic` contrast.
    "typescript/modules/imports/default_empty_braces",
];

impl ConformanceAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let all = super::walk_fixtures_or_fail()?;

        // Read each conformance doc once; reuse its content for the orphan scan and
        // the link/heading parse (primed into the cache so anchors resolve for free).
        let mut cache = DocCache::new();
        let prettier_src = read_doc(CONFORMANCE_PRETTIER)?;
        let svelte_src = read_doc(CONFORMANCE_SVELTE)?;
        cache.prime(CONFORMANCE_PRETTIER, &prettier_src);
        cache.prime(CONFORMANCE_SVELTE, &svelte_src);

        let orphans = [
            run_orphan_audit(
                &all,
                CONFORMANCE_PRETTIER,
                &prettier_src,
                "_prettier_divergence",
                fixtures::Fixture::is_prettier_divergence,
            ),
            run_orphan_audit(
                &all,
                CONFORMANCE_SVELTE,
                &svelte_src,
                "_svelte_divergence",
                fixtures::Fixture::is_svelte_divergence,
            ),
        ];

        // One README existence stat per fixture, shared by both checks below.
        let readmes: Vec<(&fixtures::Fixture, PathBuf)> = all
            .iter()
            .filter_map(|f| {
                let p = f.path.join("README.md");
                p.exists().then_some((f, p))
            })
            .collect();

        let dead_links = run_link_audit(&readmes, &mut cache);
        let missing_backlinks = run_backlink_audit(&readmes, &mut cache);
        let stray_readmes = run_readme_audit(&readmes);

        let report = Report {
            orphans,
            dead_links,
            missing_backlinks,
            stray_readmes,
        };

        if self.json {
            report.print_json();
        } else {
            report.print_human();
        }

        if report.is_clean() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

/// Read a doc file, returning [`CliError::Failed`] (after a message) on failure.
fn read_doc(path: &str) -> Result<String, CliError> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("Error reading {path}: {e}");
        CliError::Failed
    })
}

//
// Check 1 — orphans (divergence fixture not linked in its doc)
//

struct OrphanAudit {
    doc_path: &'static str,
    suffix_label: &'static str,
    total: usize,
    unlinked: Vec<String>,
}

fn run_orphan_audit(
    all: &[fixtures::Fixture],
    doc_path: &'static str,
    doc: &str,
    suffix_label: &'static str,
    is_in_class: impl Fn(&fixtures::Fixture) -> bool,
) -> OrphanAudit {
    let linked = extract_linked_fixtures(doc);
    let divergence: BTreeSet<String> = all
        .iter()
        .filter(|f| is_in_class(f))
        .map(|f| normalize_fixture_path(&f.relative_path))
        .collect();
    let total = divergence.len();
    let unlinked: Vec<String> = divergence
        .into_iter()
        .filter(|p| !linked.contains(p))
        .collect();
    OrphanAudit {
        doc_path,
        suffix_label,
        total,
        unlinked,
    }
}

/// Strip a fixture's `relative_path` (`./tests/fixtures/<p>`) down to `<p>`.
fn normalize_fixture_path(rel: &str) -> String {
    rel.rsplit_once("tests/fixtures/")
        .map_or(rel, |(_, p)| p)
        .trim_end_matches('/')
        .to_string()
}

/// Extract every `tests/fixtures/<path>` reference in a doc, normalized to `<path>`.
///
/// Captures any link or prose form — a path ends at the first `)`, `]`, backtick,
/// `|`, or whitespace — since the orphan check only needs set membership. Fenced
/// code blocks are skipped (a fixture path in an example doesn't sanction it);
/// inline-code mentions still count (a catalog entry may be `` `tests/fixtures/…` ``).
fn extract_linked_fixtures(doc: &str) -> BTreeSet<String> {
    const MARKER: &str = "tests/fixtures/";
    let mut set = BTreeSet::new();
    let mut in_fence = false;
    for line in doc.lines() {
        if is_fence_marker(line.trim_start()) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(idx) = rest.find(MARKER) {
            let after = &rest[idx + MARKER.len()..];
            let end = after
                .find(|c: char| c == ')' || c == ']' || c == '`' || c == '|' || c.is_whitespace())
                .unwrap_or(after.len());
            let path = after[..end].trim_end_matches('/');
            if !path.is_empty() {
                set.insert(path.to_string());
            }
            rest = &after[end..];
        }
    }
    set
}

//
// Check 2 — dead links (every Markdown link resolves on disk)
//

struct DeadLink {
    source: String,
    line: usize,
    target: String,
    reason: String,
}

/// Sources to link-check: the two conformance docs, the link-checked-only docs
/// ([`LINK_CHECKED_DOCS`]), and every fixture README.
fn run_link_audit(
    readmes: &[(&fixtures::Fixture, PathBuf)],
    cache: &mut DocCache,
) -> Vec<DeadLink> {
    let mut sources: Vec<PathBuf> = vec![CONFORMANCE_PRETTIER.into(), CONFORMANCE_SVELTE.into()];
    sources.extend(LINK_CHECKED_DOCS.iter().map(PathBuf::from));
    sources.extend(readmes.iter().map(|(_, p)| p.clone()));

    let mut dead = Vec::new();
    for source in &sources {
        // Clone the parsed links so we can borrow the cache mutably while resolving.
        let links = match cache.get(source) {
            Some(doc) => doc.links.clone(),
            // A README that vanished between the walk and here is a race we ignore,
            // but a *named* source that can't be read is a mis-wired audit, not a
            // clean run — report it rather than silently checking nothing.
            None => {
                if LINK_CHECKED_DOCS.iter().any(|d| Path::new(d) == source) {
                    dead.push(DeadLink {
                        source: source.display().to_string(),
                        line: 0,
                        target: String::new(),
                        reason: "link-checked doc could not be read".to_string(),
                    });
                }
                continue;
            }
        };
        for link in links {
            if let Err(reason) = resolve_link(source, &link.target, cache) {
                dead.push(DeadLink {
                    source: source.display().to_string(),
                    line: link.line,
                    target: link.target,
                    reason,
                });
            }
        }
    }
    dead
}

/// Resolve a single Markdown link target against the filesystem. External schemes
/// are out of scope (we never fetch); everything else must resolve on disk.
fn resolve_link(source: &Path, target: &str, cache: &mut DocCache) -> Result<(), String> {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
    {
        return Ok(());
    }
    let (path_part, anchor) = match target.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (target, None),
    };

    // Pure `#anchor` — resolve against the source file's own headings.
    if path_part.is_empty() {
        return match anchor {
            Some(a) => resolve_anchor(source, a, cache),
            None => Ok(()),
        };
    }

    let base = source.parent().unwrap_or_else(|| Path::new("."));
    let resolved = base.join(path_part);
    if !resolved.exists() {
        return Err(format!("path not found: {path_part}"));
    }
    // Anchor into another Markdown file — verify the heading exists there.
    if let Some(a) = anchor
        && path_part.ends_with(".md")
    {
        return resolve_anchor(&resolved, a, cache);
    }
    Ok(())
}

fn resolve_anchor(md_path: &Path, anchor: &str, cache: &mut DocCache) -> Result<(), String> {
    match cache.get(md_path) {
        Some(doc) if doc.headings.contains(anchor) => Ok(()),
        Some(_) => Err(format!(
            "anchor #{anchor} not found in {}",
            md_path.display()
        )),
        None => Err(format!(
            "cannot read {} (for anchor #{anchor})",
            md_path.display()
        )),
    }
}

//
// Check 3 — missing back-links (divergence README lacks a link to its sanctioning doc)
//

struct MissingBacklink {
    fixture: String,
    doc_path: &'static str,
}

/// Every divergence README must link to the doc that sanctions its claim. For each
/// fixture that *has* a README, check the suffix class against the required doc(s):
/// a `_prettier_divergence` must link `conformance_prettier.md`, a `_svelte_divergence`
/// must link `conformance_svelte.md`, and the combined suffix (both predicates true)
/// must link both. A missing README is out of scope here — that's the validator's `D1`.
fn run_backlink_audit(
    readmes: &[(&fixtures::Fixture, PathBuf)],
    cache: &mut DocCache,
) -> Vec<MissingBacklink> {
    let mut missing = Vec::new();
    for (f, readme_path) in readmes {
        for (doc_path, in_class) in [
            (CONFORMANCE_PRETTIER, f.is_prettier_divergence()),
            (CONFORMANCE_SVELTE, f.is_svelte_divergence()),
        ] {
            if in_class && !readme_links_to_doc(readme_path, doc_path, cache) {
                missing.push(MissingBacklink {
                    fixture: normalize_fixture_path(&f.relative_path),
                    doc_path,
                });
            }
        }
    }
    missing
}

/// Does the README at `readme_path` hold a Markdown link whose *path part* resolves
/// (on disk, canonicalized) to `doc_path`? Anchor validity is the dead-link check's
/// job — here we only assert the back-link is present and aimed at the right doc, so a
/// broken link won't match (canonicalize fails → the joined path can't equal the doc's).
fn readme_links_to_doc(readme_path: &Path, doc_path: &str, cache: &mut DocCache) -> bool {
    let doc_key = canonical_key(Path::new(doc_path));
    let links = match cache.get(readme_path) {
        Some(doc) => doc.links.clone(),
        None => return false,
    };
    let base = readme_path.parent().unwrap_or_else(|| Path::new("."));
    links.iter().any(|link| {
        let path_part = link
            .target
            .split_once('#')
            .map_or(link.target.as_str(), |(p, _)| p);
        // A pure `#anchor` points within the README itself, not at the doc.
        !path_part.is_empty() && canonical_key(&base.join(path_part)) == doc_key
    })
}

//
// Check 4 — stray READMEs (non-divergence fixture carrying a non-allowlisted README)
//

fn run_readme_audit(readmes: &[(&fixtures::Fixture, PathBuf)]) -> Vec<String> {
    let allow: BTreeSet<&str> = ALLOWED_NONDIVERGENCE_READMES.iter().copied().collect();
    readmes
        .iter()
        .map(|(f, _)| *f)
        .filter(|f| !f.is_prettier_divergence() && !f.is_svelte_divergence())
        .map(|f| normalize_fixture_path(&f.relative_path))
        .filter(|p| !allow.contains(p.as_str()))
        .collect()
}

//
// Markdown parsing (headings → anchor slugs, inline links) + read-once cache
//

#[derive(Clone)]
struct MdLink {
    line: usize,
    target: String,
}

struct MarkdownDoc {
    headings: BTreeSet<String>,
    links: Vec<MdLink>,
}

/// Parse a Markdown document into its anchor slugs and inline links, skipping fenced
/// code blocks (so example code and link-shaped snippets are not mistaken for links).
fn parse_markdown(content: &str) -> MarkdownDoc {
    let mut headings = BTreeSet::new();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut links = Vec::new();
    let mut in_fence = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if is_fence_marker(trimmed) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(text) = heading_text(trimmed) {
            let base = slugify(text);
            // GitHub disambiguates repeated headings with `-1`, `-2`, …
            let n = slug_counts.entry(base.clone()).or_insert(0);
            let slug = if *n == 0 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            *n += 1;
            headings.insert(slug);
        }
        extract_inline_links(line, i + 1, &mut links);
    }

    MarkdownDoc { headings, links }
}

/// True for a fenced-code-block delimiter line (```` ``` ```` or `~~~`), given the
/// already-`trim_start`ed line. Toggles in/out of a code block.
fn is_fence_marker(trimmed: &str) -> bool {
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// The text of an ATX heading (`#`..`######` + space), or `None` for non-headings.
fn heading_text(trimmed: &str) -> Option<&str> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && trimmed.as_bytes().get(hashes) == Some(&b' ') {
        Some(trimmed[hashes + 1..].trim())
    } else {
        None
    }
}

/// GitHub-style anchor slug: lowercase; keep alphanumerics and `_`; map each space
/// or `-` to `-` (1:1, so `a / b` → `a--b`); drop all other punctuation.
fn slugify(heading: &str) -> String {
    let mut s = String::new();
    for c in heading.trim().chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' {
            s.push('-');
        } else if c == '_' {
            s.push('_');
        }
    }
    s
}

/// Blank out inline code spans (backtick-delimited) so a `](` *inside* code —
/// e.g. `` `set [x](/* c */ a)` `` — is not mistaken for a link. Code content is
/// replaced char-for-char with spaces; real link text containing inline code
/// (`` [`foo`](./foo) ``) still exposes its `](` outside the span.
fn mask_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Extract the target of every inline link `[text](target)` on a line. The target
/// ends at the first `)` or whitespace (so `(target "title")` keeps just the path).
fn extract_inline_links(raw: &str, lineno: usize, out: &mut Vec<MdLink>) {
    let line = mask_inline_code(raw);
    let line = line.as_str();
    let mut i = 0;
    while let Some(rel) = line[i..].find("](") {
        let start = i + rel + 2;
        let rest = &line[start..];
        let end = rest
            .find(|c: char| c == ')' || c.is_whitespace())
            .unwrap_or(rest.len());
        let target = &rest[..end];
        if !target.is_empty() {
            out.push(MdLink {
                line: lineno,
                target: target.to_string(),
            });
        }
        i = start + end;
    }
}

/// Reads and parses each Markdown file at most once, keyed by canonical path so the
/// same physical doc reached via different relative links shares one parse.
struct DocCache {
    map: HashMap<PathBuf, Option<MarkdownDoc>>,
}

impl DocCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Seed the cache with an already-read doc (avoids a second read of the
    /// conformance docs, which the orphan scan has in hand).
    fn prime(&mut self, path: &str, content: &str) {
        let key = canonical_key(Path::new(path));
        self.map
            .entry(key)
            .or_insert_with(|| Some(parse_markdown(content)));
    }

    fn get(&mut self, path: &Path) -> Option<&MarkdownDoc> {
        let key = canonical_key(path);
        self.map
            .entry(key)
            .or_insert_with(|| {
                std::fs::read_to_string(path)
                    .ok()
                    .map(|c| parse_markdown(&c))
            })
            .as_ref()
    }
}

fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

//
// Reporting
//

struct Report {
    orphans: [OrphanAudit; 2],
    dead_links: Vec<DeadLink>,
    missing_backlinks: Vec<MissingBacklink>,
    stray_readmes: Vec<String>,
}

impl Report {
    fn is_clean(&self) -> bool {
        self.orphans.iter().all(|a| a.unlinked.is_empty())
            && self.dead_links.is_empty()
            && self.missing_backlinks.is_empty()
            && self.stray_readmes.is_empty()
    }

    fn print_human(&self) {
        for a in &self.orphans {
            if a.unlinked.is_empty() {
                println!(
                    "✓ all {} {} fixtures linked in {}",
                    a.total, a.suffix_label, a.doc_path
                );
            } else {
                eprintln!(
                    "✗ {} of {} {} fixtures NOT linked in {}:",
                    a.unlinked.len(),
                    a.total,
                    a.suffix_label,
                    a.doc_path
                );
                for p in &a.unlinked {
                    eprintln!("  - {p}");
                }
            }
        }

        if self.dead_links.is_empty() {
            println!("✓ all Markdown links resolve (conformance + compiler docs, fixture READMEs)");
        } else {
            eprintln!("✗ {} dead link(s):", self.dead_links.len());
            for d in &self.dead_links {
                eprintln!(
                    "  - {}:{} → `{}` — {}",
                    d.source, d.line, d.target, d.reason
                );
            }
        }

        if self.missing_backlinks.is_empty() {
            println!("✓ every divergence README links to its sanctioning conformance doc");
        } else {
            eprintln!(
                "✗ {} divergence README(s) missing a back-link to their sanctioning doc \
                 (add `See [conformance_*.md §…](…)`):",
                self.missing_backlinks.len()
            );
            for m in &self.missing_backlinks {
                eprintln!("  - {} → no link to {}", m.fixture, m.doc_path);
            }
        }

        if self.stray_readmes.is_empty() {
            println!("✓ no stray READMEs on non-divergence fixtures");
        } else {
            eprintln!(
                "✗ {} non-divergence fixture(s) carry a README (matches both tools — \
                 delete it, or allowlist in ALLOWED_NONDIVERGENCE_READMES with a reason):",
                self.stray_readmes.len()
            );
            for p in &self.stray_readmes {
                eprintln!("  - {p}");
            }
        }
    }

    fn print_json(&self) {
        let report = serde_json::json!({
            "orphans": self.orphans.iter().map(|a| serde_json::json!({
                "doc": a.doc_path,
                "suffix": a.suffix_label,
                "total": a.total,
                "unlinked": a.unlinked,
            })).collect::<Vec<_>>(),
            "dead_links": self.dead_links.iter().map(|d| serde_json::json!({
                "source": d.source,
                "line": d.line,
                "target": d.target,
                "reason": d.reason,
            })).collect::<Vec<_>>(),
            "missing_backlinks": self.missing_backlinks.iter().map(|m| serde_json::json!({
                "fixture": m.fixture,
                "doc": m.doc_path,
            })).collect::<Vec<_>>(),
            "stray_readmes": self.stray_readmes,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_linked_fixtures_handles_link_and_prose_forms() {
        let doc = "see [foo](../tests/fixtures/a/b/) and `tests/fixtures/c/d` | tests/fixtures/e/f]\n\
                   also tests/fixtures/g/h done\n";
        let set = extract_linked_fixtures(doc);
        let expected: BTreeSet<String> = ["a/b", "c/d", "e/f", "g/h"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(set, expected);
    }

    #[test]
    fn extract_linked_fixtures_skips_fenced_blocks_but_keeps_inline_code() {
        let doc = "catalog `tests/fixtures/real/one`\n\
                   ```\ntests/fixtures/example/in_fence\n```\n\
                   prose tests/fixtures/real/two\n";
        let set = extract_linked_fixtures(doc);
        let expected: BTreeSet<String> = ["real/one", "real/two"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(set, expected, "fence example excluded; inline + prose kept");
    }

    #[test]
    fn normalize_fixture_path_strips_prefix_and_trailing_slash() {
        assert_eq!(normalize_fixture_path("./tests/fixtures/x/y/"), "x/y");
        assert_eq!(
            normalize_fixture_path("../tests/fixtures/foo/bar"),
            "foo/bar"
        );
        assert_eq!(normalize_fixture_path("x/y/"), "x/y");
    }

    #[test]
    fn slugify_matches_github_rules() {
        assert_eq!(slugify("Svelte: Attributes"), "svelte-attributes");
        // `/` is dropped but the surrounding spaces each become a hyphen → `--`.
        assert_eq!(slugify("JSDoc / paren semantics"), "jsdoc--paren-semantics");
        assert_eq!(slugify("CSS: At-Rules"), "css-at-rules");
        assert_eq!(
            slugify("Comment Position Philosophy"),
            "comment-position-philosophy"
        );
        assert_eq!(slugify("keep_underscores"), "keep_underscores");
    }

    #[test]
    fn parse_markdown_collects_headings_and_skips_fences() {
        let md = "# Title\n\n## Sub Section\n\n```\n# not a heading\n[x](./nope)\n```\n\nsee [y](./real)\n";
        let doc = parse_markdown(md);
        assert!(doc.headings.contains("title"));
        assert!(doc.headings.contains("sub-section"));
        assert!(!doc.headings.contains("not-a-heading"));
        // Only the out-of-fence link is captured.
        let targets: Vec<&str> = doc.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, vec!["./real"]);
    }

    #[test]
    fn parse_markdown_disambiguates_repeated_headings() {
        let doc = parse_markdown("## Dup\n## Dup\n");
        assert!(doc.headings.contains("dup"));
        assert!(doc.headings.contains("dup-1"));
    }

    #[test]
    fn extract_inline_links_reads_multiple_per_line() {
        let mut out = Vec::new();
        extract_inline_links("a [x](./one) b [y](../two#anchor) c", 3, &mut out);
        let got: Vec<(usize, &str)> = out.iter().map(|l| (l.line, l.target.as_str())).collect();
        assert_eq!(got, vec![(3, "./one"), (3, "../two#anchor")]);
    }

    #[test]
    fn readme_links_to_doc_matches_only_the_sanctioning_doc() {
        // A divergence README in a deep fixture dir, plus the two conformance docs,
        // laid out under a tempdir so canonicalization has real paths to resolve.
        let root =
            std::env::temp_dir().join(format!("tsv_conf_audit_backlink_{}", std::process::id()));
        let docs = root.join("docs");
        let fixture = root.join("tests/fixtures/css/x_prettier_divergence");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::create_dir_all(&fixture).unwrap();
        std::fs::write(docs.join("conformance_prettier.md"), "# CSS: Layout\n").unwrap();
        std::fs::write(docs.join("conformance_svelte.md"), "# Svelte\n").unwrap();
        let readme = fixture.join("README.md");
        std::fs::write(
            &readme,
            "See [conformance_prettier.md §CSS: Layout]\
             (../../../../docs/conformance_prettier.md#css-layout).\n",
        )
        .unwrap();

        let mut cache = DocCache::new();
        let prettier = docs.join("conformance_prettier.md");
        let svelte = docs.join("conformance_svelte.md");
        assert!(
            readme_links_to_doc(&readme, prettier.to_str().unwrap(), &mut cache),
            "back-link to conformance_prettier.md resolves"
        );
        assert!(
            !readme_links_to_doc(&readme, svelte.to_str().unwrap(), &mut cache),
            "no link to conformance_svelte.md"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn extract_inline_links_ignores_brackets_inside_inline_code() {
        let mut out = Vec::new();
        // `](` inside backticks is not a link; a real link whose text is inline code is.
        extract_inline_links(
            "prose `set [x](/* c */ a)` then [`foo`](./foo)",
            1,
            &mut out,
        );
        let got: Vec<&str> = out.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(got, vec!["./foo"]);
    }
}
