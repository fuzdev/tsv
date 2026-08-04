use crate::cli::CliError;
use crate::fixtures;
use argh::FromArgs;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use tsv_ignore::IgnoreStack;

/// Audit doc/fixture integrity: divergence cataloging, link resolution, README hygiene.
///
/// Runs five checks over one fixture walk, reading each file at most once:
///
/// 1. **Orphans** — every `_*_divergence`-suffixed fixture must be linked in the doc
///    that sanctions its claim (`_prettier_divergence` → any `docs/conformance_prettier*.md`,
///    `_svelte_divergence` → `docs/conformance_svelte.md`, both for the combined suffix).
///    A divergence suffix asserts a deliberate difference; that claim must be cataloged
///    so it is discoverable and reviewable.
/// 2. **Dead links** — every Markdown link (relative path and `#anchor`) in every
///    Markdown file in the repo (walked at run time, so a new doc is gated by existing)
///    must resolve on disk: `docs/*.md`, every fixture README, and the files that once
///    had no link gate at all — root `CLAUDE.md` / `README.md`, each crate's `CLAUDE.md`,
///    the shipped `crates/tsv_wasm/README_*.md`. The orphan check only proves
///    *forward* coverage (live fixture → mentioned in doc); this is the *reverse*
///    direction — a link to a renamed/demoted/deleted fixture, or a back-link with
///    the wrong `../` depth or a stale anchor, is otherwise invisible. Targets that
///    climb out of the repo (sibling checkouts like `../../ecma262/…`) are
///    machine-dependent, so they are out of scope like external URLs — the audit
///    gates repo-internal integrity only.
/// 3. **Missing back-links** — every `_*_divergence` fixture's README must *contain* a
///    link that resolves to a doc which actually **catalogs that fixture** (check 1's
///    per-doc attribution), not merely to some member of the sanctioning family. Checks
///    1+2 prove the doc catalogs the fixture and that any link present resolves, but
///    neither requires the back-link to *exist* — a README that simply omits it passes
///    both. This closes that gap, and a second one the doc split opened: with a single
///    conformance doc, "cataloged in D" and "links D" were the same fact, so the two
///    checks agreed for free; across a six-doc family they are independent, and a README
///    could link the frame while its entry lived in a language catalog. A fixture no
///    member catalogs falls back to the whole family here, so it reports once (as check
///    1's orphan) rather than twice. (A divergence fixture with no README at all is the
///    fixture validator's `D1` rule, run separately in `fixtures_validate`.)
/// 4. **Stray READMEs** — a non-divergence fixture (matches both Prettier and Svelte)
///    should not carry a README; there is no divergence to sanction, and any conformance
///    back-link it holds rots unaudited. A small allowlist (`ALLOWED_NONDIVERGENCE_READMES`)
///    holds the deliberate exceptions that document a real parser/spec/contrast fact.
/// 5. **Catalog-family drift** — the `docs/conformance_prettier*.md` on disk must be
///    exactly [`CONFORMANCE_PRETTIER`], and the frame's §Catalogs table must index every
///    member. Checks 1 and 3 read a hand-listed family while reporting the glob, so a
///    catalog the list omits fails only as *their* findings — its entries as orphans, a
///    README aiming at it as a missing back-link — with nothing naming the constant. The
///    index half covers the reader's route: `CLAUDE.md` sends divergence authors to "the
///    catalog for the language you're touching, listed in its §Catalogs table", so an
///    unindexed member is unreachable that way even with every other check green.
///
/// Exits non-zero on any finding. Part of `deno task check`.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "conformance_audit")]
pub struct ConformanceAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

/// The Prettier-divergence catalog is split by language: a shared frame (terminology,
/// `◆reason` tags, decision framework) plus the per-language catalogs it indexes. Any
/// one of them can catalog a `_prettier_divergence` fixture — the orphan check accepts
/// the union — but a back-link must aim at a member that catalogs *that* fixture, so the
/// family is read per-doc ([`catalog_owners`]) rather than joined into one document.
///
/// Hand-listed rather than globbed so the family is a **declared** expectation, but
/// [`run_family_audit`] holds it to the glob its label advertises: a
/// `docs/conformance_prettier*.md` on disk that this list omits is a finding, not a
/// silent non-member. Without that check the omission surfaces only as a downstream
/// misdiagnosis — its fixtures reported as orphans, a README aimed at it reported as
/// missing a back-link — with nothing naming the real cause.
const CONFORMANCE_PRETTIER: &[&str] = &[
    "docs/conformance_prettier.md",
    "docs/conformance_prettier_css.md",
    "docs/conformance_prettier_svelte.md",
    "docs/conformance_prettier_ts.md",
    "docs/conformance_prettier_ts_comments.md",
    "docs/conformance_prettier_ignore.md",
];
const CONFORMANCE_PRETTIER_LABEL: &str = "docs/conformance_prettier*.md";
const CONFORMANCE_SVELTE: &[&str] = &["docs/conformance_svelte.md"];
const CONFORMANCE_SVELTE_LABEL: &str = "docs/conformance_svelte.md";

/// Fixture path (`normalize_fixture_path` form) → the family docs that catalog it, in
/// family order. Built by [`catalog_owners`].
type CatalogOwners = HashMap<String, Vec<&'static str>>;

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

        // Every repo doc is link-checked; the conformance docs additionally drive the
        // orphan and back-link checks. Read each doc once, priming the cache so anchors
        // into it resolve for free.
        let markdown = enumerate_markdown()?;
        let mut cache = DocCache::new();
        let mut family_src: HashMap<PathBuf, String> = HashMap::new();
        for doc in &markdown {
            let src = read_doc(doc)?;
            cache.prime(doc, &src);
            if CONFORMANCE_PRETTIER
                .iter()
                .chain(CONFORMANCE_SVELTE)
                .any(|d| doc.as_path() == Path::new(d))
            {
                family_src.insert(doc.clone(), src);
            }
        }
        let family = run_family_audit(
            &markdown,
            family_src.get(Path::new(CONFORMANCE_PRETTIER_FRAME)),
        );
        let prettier_owners = catalog_owners(CONFORMANCE_PRETTIER, &family_src)?;
        let svelte_owners = catalog_owners(CONFORMANCE_SVELTE, &family_src)?;

        let orphans = [
            run_orphan_audit(
                &all,
                CONFORMANCE_PRETTIER_LABEL,
                &prettier_owners,
                "_prettier_divergence",
                fixtures::Fixture::is_prettier_divergence,
            ),
            run_orphan_audit(
                &all,
                CONFORMANCE_SVELTE_LABEL,
                &svelte_owners,
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

        let (dead_links, links_checked) = run_link_audit(&markdown, &mut cache);
        let missing_backlinks =
            run_backlink_audit(&readmes, &prettier_owners, &svelte_owners, &mut cache);
        let stray_readmes = run_readme_audit(&readmes);

        // The link check's own coverage, split the way a reader asks about it: the
        // curated doc surface, the fixture READMEs, and everything else the walk reaches.
        let docs_checked = markdown.iter().filter(|p| p.starts_with("docs")).count();
        let fixture_docs_checked = markdown
            .iter()
            .filter(|p| p.starts_with("tests/fixtures"))
            .count();

        let report = Report {
            orphans,
            family,
            dead_links,
            docs_checked,
            fixture_docs_checked,
            other_docs_checked: markdown.len() - docs_checked - fixture_docs_checked,
            links_checked,
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

/// Which member(s) of a conformance family catalog each fixture, in family order.
///
/// The two checks that consume this ask different questions of it, and joining the
/// family into one document (as this once did) can only answer the first: the orphan
/// check wants the **union** — a fixture cataloged anywhere in the family is covered —
/// while the back-link check wants the **attribution**, since a README must aim at a doc
/// that actually lists that fixture. A member missing from `docs/` is a mis-wired audit —
/// the check would silently lose whatever that doc catalogs — not a clean run.
fn catalog_owners(
    family: &[&'static str],
    sources: &HashMap<PathBuf, String>,
) -> Result<CatalogOwners, CliError> {
    let mut owners: CatalogOwners = HashMap::new();
    for doc in family {
        let Some(part) = sources.get(Path::new(doc)) else {
            eprintln!("Error: docs/ is missing {doc}");
            return Err(CliError::Failed);
        };
        for fixture in extract_linked_fixtures(part) {
            owners.entry(fixture).or_default().push(doc);
        }
    }
    Ok(owners)
}

/// Read a doc file, returning [`CliError::Failed`] (after a message) on failure.
fn read_doc(path: &Path) -> Result<String, CliError> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("Error reading {}: {e}", path.display());
        CliError::Failed
    })
}

/// The one directory the walk prunes by name. Everything else it skips is
/// `.gitignore`'s decision — but git never lists its own store, so this is not
/// expressible there.
const WALK_SKIP_DIR: &str = ".git";

/// Enumerate every `.md` file the repo tracks (sorted, repo-relative).
///
/// Computed rather than hand-listed, precisely so a new doc — a new crate's
/// `CLAUDE.md`, a doc in a directory that does not exist yet — is gated by existing.
/// Two exclusions keep "the repo's own Markdown" honest:
///
/// - **`.gitignore`**, via [`tsv_ignore::IgnoreStack`] — the same matcher `tsv format`
///   walks with. Build output (`target/`, `crates/tsv_wasm/pkg/`), vendored deps
///   (`node_modules/`), and the per-machine `*.local.*` / `*.tmp` conventions are all
///   ignored there, so none of them can fail a gate over content the repo does not have.
///   Hand-listing those would be a second copy of `.gitignore`, drifting from it.
/// - **Symlinks**, because the target is enumerated on its own: `AGENTS.md` points at
///   `CLAUDE.md`, and following both would check one file twice and report every
///   finding in it twice.
///
/// An empty result is a failure (wrong cwd), not a vacuous pass.
fn enumerate_markdown() -> Result<Vec<PathBuf>, CliError> {
    let mut found = Vec::new();
    let mut stack = IgnoreStack::new();
    walk_markdown(Path::new("."), "", &mut stack, &mut found)?;
    found.sort();
    if found.is_empty() {
        eprintln!("Error: no .md files found — running from the repo root?");
        return Err(CliError::Failed);
    }
    Ok(found)
}

/// Recursive worker for [`enumerate_markdown`]. `dir_rel` is `dir` relative to the repo
/// root (`/`-separated, empty at the root) — the form both the ignore matcher and the
/// emitted paths want, so a finding reads as a repo-relative path.
fn walk_markdown(
    dir: &Path,
    dir_rel: &str,
    stack: &mut IgnoreStack,
    out: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    // This directory's own `.gitignore` classifies its children, not itself, so it is
    // pushed before the loop and popped after it. An unreadable one is a mis-wired
    // audit — silently walking a tree we would have pruned is worse than failing.
    let gitignore = dir.join(".gitignore");
    let pushed = match std::fs::read_to_string(&gitignore) {
        Ok(content) => {
            stack.push_gitignore(dir_rel, &content);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            eprintln!("Error reading {}: {e}", gitignore.display());
            return Err(CliError::Failed);
        }
    };

    let entries = std::fs::read_dir(dir).map_err(|e| {
        eprintln!("Error reading {}: {e}", dir.display());
        CliError::Failed
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            eprintln!("Error reading an entry of {}: {e}", dir.display());
            CliError::Failed
        })?;
        // `file_type` does not follow symlinks, which is what the skip below needs.
        let file_type = entry.file_type().map_err(|e| {
            eprintln!("Error stat-ing {}: {e}", entry.path().display());
            CliError::Failed
        })?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let child_rel = if dir_rel.is_empty() {
            name.to_string()
        } else {
            format!("{dir_rel}/{name}")
        };
        if stack.is_ignored(&child_rel, file_type.is_dir()) {
            continue;
        }
        if file_type.is_dir() {
            if name == WALK_SKIP_DIR {
                continue;
            }
            walk_markdown(&entry.path(), &child_rel, stack, out)?;
        } else if file_type.is_file() && name.ends_with(".md") {
            out.push(PathBuf::from(&child_rel));
        }
    }

    if pushed {
        stack.pop_gitignore();
    }
    Ok(())
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
    owners: &CatalogOwners,
    suffix_label: &'static str,
    is_in_class: impl Fn(&fixtures::Fixture) -> bool,
) -> OrphanAudit {
    let divergence: BTreeSet<String> = all
        .iter()
        .filter(|f| is_in_class(f))
        .map(|f| normalize_fixture_path(&f.relative_path))
        .collect();
    let total = divergence.len();
    let unlinked: Vec<String> = divergence
        .into_iter()
        .filter(|p| !owners.contains_key(p))
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

/// Sources to link-check: every Markdown file the walk found. That is `docs/*.md`, every
/// fixture README, and everything else we author — root `CLAUDE.md` / `README.md`, each
/// crate's `CLAUDE.md`, the shipped `crates/tsv_wasm/README_*.md`, a container-directory
/// README under `tests/fixtures/`. A stale `../` depth or dead anchor is the same defect
/// wherever it sits, and before the walk those files had no link gate at all.
///
/// Returns the dead links and the total link count checked (so a green pass reports its
/// own coverage instead of reading identically to a vacuous one).
fn run_link_audit(sources: &[PathBuf], cache: &mut DocCache) -> (Vec<DeadLink>, usize) {
    let mut dead = Vec::new();
    let mut checked = 0usize;
    for source in sources {
        // Clone the parsed links so we can borrow the cache mutably while resolving.
        let links = match cache.get(source) {
            Some(doc) => doc.links.clone(),
            // The walk just stat-ed this file, so an unreadable one is a mis-wired
            // audit, not a clean run — report it rather than silently checking nothing.
            None => {
                dead.push(DeadLink {
                    source: source.display().to_string(),
                    line: 0,
                    target: String::new(),
                    reason: "link-checked doc could not be read".to_string(),
                });
                continue;
            }
        };
        checked += links.len();
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
    (dead, checked)
}

/// Resolve a single Markdown link target against the filesystem. External schemes
/// and targets that climb out of the repo (sibling checkouts, machine-dependent)
/// are out of scope; everything else must resolve on disk.
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
    if escapes_repo(&resolved) {
        return Ok(());
    }
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

/// Lexically resolve `path` (relative to the repo root, i.e. the cwd) and report
/// whether it climbs out of the repo. `..` past the root means a sibling-checkout
/// target (e.g. `../../ecma262/…`) — present only where that checkout is, so
/// checking it would make the gate machine-dependent. Lexical on purpose: the
/// target may not exist, so it can't be canonicalized.
fn escapes_repo(path: &Path) -> bool {
    let mut depth: usize = 0;
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return true,
            },
            std::path::Component::Normal(_) => depth += 1,
            std::path::Component::CurDir => {}
            // An absolute target never escapes lexically; the existence check speaks.
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return false,
        }
    }
    false
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
    /// The doc(s) that catalog this fixture — what the README must link. Falls back to
    /// the whole family label when no member catalogs it (an orphan, reported by check 1).
    doc_path: String,
}

/// Every divergence README must link to the doc that sanctions its claim. For each
/// fixture that *has* a README, check the suffix class against the required family:
/// a `_prettier_divergence` must link one of the `conformance_prettier*.md` docs, a
/// `_svelte_divergence` must link `conformance_svelte.md`, and the combined suffix
/// (both predicates true) must link one of each. Any member of the prettier family
/// satisfies the claim — which one is right is the language question the doc's own
/// §Catalogs table answers, not something a path check can decide. A missing README is
/// out of scope here — that's the validator's `D1`.
fn run_backlink_audit(
    readmes: &[(&fixtures::Fixture, PathBuf)],
    prettier_owners: &CatalogOwners,
    svelte_owners: &CatalogOwners,
    cache: &mut DocCache,
) -> Vec<MissingBacklink> {
    let mut missing = Vec::new();
    for (f, readme_path) in readmes {
        let fixture = normalize_fixture_path(&f.relative_path);
        for (family, label, owners, in_class) in [
            (
                CONFORMANCE_PRETTIER,
                CONFORMANCE_PRETTIER_LABEL,
                prettier_owners,
                f.is_prettier_divergence(),
            ),
            (
                CONFORMANCE_SVELTE,
                CONFORMANCE_SVELTE_LABEL,
                svelte_owners,
                f.is_svelte_divergence(),
            ),
        ] {
            if !in_class {
                continue;
            }
            // The docs that actually catalog this fixture are the ones its README must
            // aim at — the frame satisfying a claim cataloged in a language doc is the
            // gap the split opened. A fixture no member catalogs is check 1's orphan, so
            // fall back to the whole family rather than reporting the same hole twice.
            let sanctioning: &[&str] = owners.get(&fixture).map_or(family, |docs| docs.as_slice());
            if !readme_links_to_doc(readme_path, sanctioning, cache) {
                missing.push(MissingBacklink {
                    fixture: fixture.clone(),
                    doc_path: if owners.contains_key(&fixture) {
                        sanctioning.join(" or ")
                    } else {
                        label.to_string()
                    },
                });
            }
        }
    }
    missing
}

/// Does the README at `readme_path` hold a Markdown link whose *path part* resolves
/// (on disk, canonicalized) to any doc in `docs`? Callers pass the docs that catalog
/// *this* fixture, not the whole family. Anchor validity is the dead-link check's job —
/// here we only assert the back-link is present and aimed at the right doc, so a broken
/// link won't match (canonicalize fails → the joined path can't equal the doc's).
fn readme_links_to_doc(readme_path: &Path, docs: &[&str], cache: &mut DocCache) -> bool {
    let keys: Vec<PathBuf> = docs.iter().map(|d| canonical_key(Path::new(d))).collect();
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
        !path_part.is_empty() && keys.contains(&canonical_key(&base.join(path_part)))
    })
}

//
// Check 5 — catalog-family drift (a member on disk the family list or its index omits)
//

/// The frame doc: the family's first member, and the one that carries the §Catalogs index.
const CONFORMANCE_PRETTIER_FRAME: &str = "docs/conformance_prettier.md";

/// The `docs/` file-name prefix every prettier-family member shares.
const CONFORMANCE_PRETTIER_PREFIX: &str = "conformance_prettier";

struct FamilyAudit {
    /// `docs/conformance_prettier*.md` on disk that [`CONFORMANCE_PRETTIER`] omits.
    unlisted: Vec<String>,
    /// Members the frame's §Catalogs table does not index.
    unindexed: Vec<String>,
}

impl FamilyAudit {
    fn is_clean(&self) -> bool {
        self.unlisted.is_empty() && self.unindexed.is_empty()
    }
}

/// Hold the prettier family to the glob its label advertises, in both directions a
/// reader depends on.
///
/// The **list** direction is the one nothing else covers. A member absent from disk is
/// already a hard error ([`catalog_owners`]) because it would make checks 1 and 3 vacuous;
/// the reverse — a catalog on disk the list omits — fails only *downstream*, as an orphan
/// or a missing back-link, never naming the constant that caused it.
///
/// The **index** direction is what a reader follows. `CLAUDE.md` sends every divergence
/// author to "the catalog for the language you're touching, listed in its §Catalogs
/// table", so a member missing from that table is unreachable by the route the docs
/// prescribe even though every check here would pass.
fn run_family_audit(markdown: &[PathBuf], frame_src: Option<&String>) -> FamilyAudit {
    let on_disk: BTreeSet<&Path> = markdown
        .iter()
        .filter(|p| {
            p.parent() == Some(Path::new("docs"))
                && p.file_name().is_some_and(|n| {
                    n.to_str()
                        .is_some_and(|n| n.starts_with(CONFORMANCE_PRETTIER_PREFIX))
                })
        })
        .map(PathBuf::as_path)
        .collect();
    let listed: BTreeSet<&Path> = CONFORMANCE_PRETTIER.iter().map(Path::new).collect();

    let unlisted = on_disk
        .difference(&listed)
        .map(|p| p.display().to_string())
        .collect();

    // An unreadable frame is `catalog_owners`' hard error a few lines later; reporting
    // every catalog as unindexed here would just bury it.
    let unindexed = frame_src.map_or_else(Vec::new, |src| {
        let indexed = catalogs_table_entries(src);
        listed
            .iter()
            .filter(|p| {
                p.to_str() != Some(CONFORMANCE_PRETTIER_FRAME)
                    && !p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| indexed.contains(n))
            })
            .map(|p| p.display().to_string())
            .collect()
    });

    FamilyAudit {
        unlisted,
        unindexed,
    }
}

/// The family members the frame's §Catalogs section links, by file name.
///
/// Scoped to that one section rather than the whole frame: the frame cross-links the
/// catalogs freely in prose, and a passing mention is not the index a reader is sent to.
fn catalogs_table_entries(frame_src: &str) -> BTreeSet<String> {
    let mut entries = BTreeSet::new();
    let mut in_section = false;
    let mut in_fence = false;
    for (i, line) in frame_src.lines().enumerate() {
        let trimmed = line.trim_start();
        if is_fence_marker(trimmed) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(text) = heading_text(trimmed) {
            // `## Catalogs` opens the section; the next heading at that level or higher
            // closes it. A deeper one (`###`) is still inside, so it must not close it.
            let level = trimmed.bytes().take_while(|&b| b == b'#').count();
            if text == "Catalogs" {
                in_section = true;
            } else if level <= 2 {
                in_section = false;
            }
            continue;
        }
        if !in_section {
            continue;
        }
        let mut links = Vec::new();
        extract_inline_links(line, i + 1, &mut links);
        for link in links {
            let target = link
                .target
                .split_once('#')
                .map_or(link.target.as_str(), |(p, _)| p);
            if let Some(name) = Path::new(target).file_name().and_then(|n| n.to_str())
                && name.starts_with(CONFORMANCE_PRETTIER_PREFIX)
                && name.ends_with(".md")
            {
                entries.insert(name.to_string());
            }
        }
    }
    entries
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
    /// docs the run loop has in hand).
    fn prime(&mut self, path: &Path, content: &str) {
        let key = canonical_key(path);
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
    family: FamilyAudit,
    dead_links: Vec<DeadLink>,
    docs_checked: usize,
    fixture_docs_checked: usize,
    other_docs_checked: usize,
    links_checked: usize,
    missing_backlinks: Vec<MissingBacklink>,
    stray_readmes: Vec<String>,
}

impl Report {
    fn is_clean(&self) -> bool {
        self.orphans.iter().all(|a| a.unlinked.is_empty())
            && self.family.is_clean()
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

        if self.family.is_clean() {
            println!(
                "✓ the {} catalogs on disk are the declared family, each indexed by §Catalogs",
                CONFORMANCE_PRETTIER.len()
            );
        } else {
            for p in &self.family.unlisted {
                eprintln!(
                    "✗ {p} is a docs/{CONFORMANCE_PRETTIER_PREFIX}*.md on disk that \
                     CONFORMANCE_PRETTIER does not list — add it there, or its entries \
                     read as orphans and a README aiming at it as missing a back-link"
                );
            }
            for p in &self.family.unindexed {
                eprintln!(
                    "✗ {p} is not linked from {CONFORMANCE_PRETTIER_FRAME} §Catalogs — \
                     the index every divergence author is sent to"
                );
            }
        }

        if self.dead_links.is_empty() {
            println!(
                "✓ all {} Markdown links resolve ({} docs/*.md + {} under tests/fixtures/ + {} other)",
                self.links_checked,
                self.docs_checked,
                self.fixture_docs_checked,
                self.other_docs_checked
            );
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
            println!("✓ every divergence README links the conformance doc that catalogs it");
        } else {
            eprintln!(
                "✗ {} divergence README(s) missing a back-link to the doc that CATALOGS them \
                 (add `See [conformance_*.md §…](…)` — the doc holding the entry, not merely \
                 the shared frame):",
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
            "family": {
                "unlisted": self.family.unlisted,
                "unindexed": self.family.unindexed,
            },
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
    fn catalog_owners_attributes_each_fixture_to_the_docs_that_link_it() {
        let mut sources = HashMap::new();
        sources.insert(
            PathBuf::from("docs/conformance_prettier.md"),
            "frame lists ../tests/fixtures/shared/both\n".to_string(),
        );
        sources.insert(
            PathBuf::from("docs/conformance_prettier_css.md"),
            "css lists ../tests/fixtures/css/only and ../tests/fixtures/shared/both\n".to_string(),
        );
        let owners = catalog_owners(
            &[
                "docs/conformance_prettier.md",
                "docs/conformance_prettier_css.md",
            ],
            &sources,
        )
        .expect("both members present");

        // The attribution the joined form could not express: only the CSS catalog
        // sanctions this fixture, so a back-link at the frame must NOT satisfy it.
        assert_eq!(
            owners.get("css/only"),
            Some(&vec!["docs/conformance_prettier_css.md"])
        );
        // Cataloged in two members → either satisfies, in family order.
        assert_eq!(
            owners.get("shared/both"),
            Some(&vec![
                "docs/conformance_prettier.md",
                "docs/conformance_prettier_css.md",
            ])
        );
        assert!(!owners.contains_key("never/mentioned"));
    }

    #[test]
    fn catalog_owners_fails_on_a_missing_family_member() {
        // A member absent from `docs/` would silently drop whatever it catalogs, turning
        // both checks vacuous for those fixtures — that is a mis-wired audit, not a pass.
        let sources = HashMap::new();
        assert!(catalog_owners(&["docs/conformance_prettier.md"], &sources).is_err());
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
    fn readme_links_to_doc_matches_only_the_sanctioning_family() {
        // A divergence README in a deep fixture dir, plus the conformance docs, laid out
        // under a tempdir so canonicalization has real paths to resolve. The README aims
        // at the *CSS catalog*, not the frame — any member of the prettier family
        // satisfies the claim, and no member of the svelte family does.
        let root =
            std::env::temp_dir().join(format!("tsv_conf_audit_backlink_{}", std::process::id()));
        let docs = root.join("docs");
        let fixture = root.join("tests/fixtures/css/x_prettier_divergence");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::create_dir_all(&fixture).unwrap();
        std::fs::write(docs.join("conformance_prettier.md"), "# Frame\n").unwrap();
        std::fs::write(docs.join("conformance_prettier_css.md"), "# CSS: Layout\n").unwrap();
        std::fs::write(docs.join("conformance_svelte.md"), "# Svelte\n").unwrap();
        let readme = fixture.join("README.md");
        std::fs::write(
            &readme,
            "See [conformance_prettier_css.md §CSS: Layout]\
             (../../../../docs/conformance_prettier_css.md#css-layout).\n",
        )
        .unwrap();

        let mut cache = DocCache::new();
        let frame = docs.join("conformance_prettier.md");
        let css = docs.join("conformance_prettier_css.md");
        let svelte = docs.join("conformance_svelte.md");
        assert!(
            readme_links_to_doc(
                &readme,
                &[frame.to_str().unwrap(), css.to_str().unwrap()],
                &mut cache
            ),
            "back-link to a prettier-family catalog resolves"
        );
        assert!(
            !readme_links_to_doc(&readme, &[frame.to_str().unwrap()], &mut cache),
            "the frame alone is not what this README links"
        );
        assert!(
            !readme_links_to_doc(&readme, &[svelte.to_str().unwrap()], &mut cache),
            "no link to conformance_svelte.md"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn catalogs_table_entries_reads_only_the_catalogs_section() {
        let frame = "# Frame\n\
                     \n\
                     Prose linking [css](./conformance_prettier_css.md) before the index.\n\
                     \n\
                     ## Catalogs\n\
                     \n\
                     | doc | covers |\n\
                     | --- | --- |\n\
                     | [conformance_prettier_svelte.md](./conformance_prettier_svelte.md) | x |\n\
                     | [ts](./conformance_prettier_ts.md#typescript) | y |\n\
                     \n\
                     ### A deeper heading is still inside\n\
                     \n\
                     | [ignore](./conformance_prettier_ignore.md) | z |\n\
                     \n\
                     ## Tooling\n\
                     \n\
                     [comments](./conformance_prettier_ts_comments.md) is out of section.\n";
        let entries = catalogs_table_entries(frame);
        let expected: BTreeSet<String> = [
            "conformance_prettier_svelte.md",
            "conformance_prettier_ts.md",
            "conformance_prettier_ignore.md",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(entries, expected);
    }

    /// A frame doc whose §Catalogs section indexes exactly `members`.
    fn frame_indexing(members: &[&str]) -> String {
        let rows: Vec<String> = members
            .iter()
            .map(|d| {
                format!(
                    "- [x](./{})\n",
                    Path::new(d).file_name().unwrap().to_str().unwrap()
                )
            })
            .collect();
        format!("# Frame\n\n## Catalogs\n\n{}", rows.concat())
    }

    /// Every family member but the frame, which is never expected to index itself.
    fn indexable_members() -> Vec<&'static str> {
        CONFORMANCE_PRETTIER
            .iter()
            .copied()
            .filter(|d| *d != CONFORMANCE_PRETTIER_FRAME)
            .collect()
    }

    #[test]
    fn family_audit_reports_a_catalog_the_declared_list_omits() {
        // The direction nothing else covers: a member on disk that `CONFORMANCE_PRETTIER`
        // does not name. Without this it fails only downstream — its fixtures as orphans,
        // a README aimed at it as a missing back-link — never naming the constant.
        let mut markdown: Vec<PathBuf> = CONFORMANCE_PRETTIER.iter().map(PathBuf::from).collect();
        markdown.push(PathBuf::from("docs/conformance_prettier_html.md"));
        // Neither a doc outside `docs/` nor an unrelated `docs/` file is a family member.
        markdown.push(PathBuf::from("crates/tsv_ts/CLAUDE.md"));
        markdown.push(PathBuf::from("docs/conformance_svelte.md"));

        let frame = frame_indexing(&indexable_members());
        let audit = run_family_audit(&markdown, Some(&frame));
        assert_eq!(audit.unlisted, vec!["docs/conformance_prettier_html.md"]);
        assert!(audit.unindexed.is_empty(), "every listed member is indexed");
        assert!(!audit.is_clean());
    }

    #[test]
    fn family_audit_reports_a_member_the_catalogs_index_omits() {
        let markdown: Vec<PathBuf> = CONFORMANCE_PRETTIER.iter().map(PathBuf::from).collect();
        // An index holding every member but one.
        let mut members = indexable_members();
        let dropped = members.pop().expect("the family has non-frame members");

        let audit = run_family_audit(&markdown, Some(&frame_indexing(&members)));
        assert!(audit.unlisted.is_empty(), "nothing on disk is unlisted");
        assert_eq!(audit.unindexed, vec![dropped.to_string()]);
        assert!(!audit.is_clean());
    }

    #[test]
    fn family_audit_is_clean_on_the_repos_own_family() {
        // The frame is never expected to index itself.
        let markdown: Vec<PathBuf> = CONFORMANCE_PRETTIER.iter().map(PathBuf::from).collect();
        let frame = frame_indexing(&indexable_members());
        assert!(run_family_audit(&markdown, Some(&frame)).is_clean());
    }

    #[test]
    fn escapes_repo_flags_only_targets_climbing_past_the_root() {
        // Stays inside: `..` never outruns the preceding components.
        assert!(!escapes_repo(Path::new("docs/../CLAUDE.md")));
        assert!(!escapes_repo(Path::new(
            "tests/fixtures/css/x/../../../../docs/conformance_prettier.md"
        )));
        // Climbs out: a sibling-checkout target (machine-dependent, skipped).
        assert!(escapes_repo(Path::new("docs/../../ecma262/CLAUDE.md")));
        assert!(escapes_repo(Path::new("../prettier/src")));
        // Absolute targets are left to the existence check.
        assert!(!escapes_repo(Path::new("/etc/passwd")));
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
