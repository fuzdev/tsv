//! The borrow-only discipline, enforced: `tsv_check` visitors borrow AST nodes and
//! never clone them.
//!
//! The binder keys its address map on `(std::ptr::from_ref(node) as usize, NodeKind)`,
//! which resolves only while every node the checker sees is the *same* arena node the
//! lowering walk numbered. Every tsv AST type derives `Clone`, so one accidental
//! `.clone()` mints a differently-addressed copy the map has never seen — and the two
//! resolution paths fail differently: the strict one (`BoundFile::require_node_id`, the
//! flow builder) aborts loudly, while the lenient unreachable-candidate lookup simply
//! misses, leaving a silently wrong candidate table rather than a crash. The quiet half
//! is why the convention needs a guard instead of a code review.
//!
//! This test scans every `src/**/*.rs` in the crate for clone-shaped calls (see
//! [`PATTERNS`]) and fails on any that is not in the reviewed [`ALLOW`] ledger — and on
//! any ledger entry whose line is no longer in the source, so a sanction can't outlive
//! the code it sanctions. Occurrences after a comment-starting `//` don't count, so
//! prose about cloning is inert; a `//` *inside a string literal* does not start a
//! comment (this crate embeds TS fixtures like `"\t// @ts-ignore\n"`, and truncating
//! there would hide a clone later on the line).
//!
//! An entry is keyed on `(path, exact trimmed line)`, so **one entry covers every
//! identical line in that file** — identical text carries identical review, and
//! reformatting a sanctioned line forces a fresh one. That equivalence holds only while
//! the reason is derivable from the text: a key generic enough to recur incidentally
//! (a bare `.clone()` left by a chain wrap) would blanket-sanction a file, so
//! `allow_ledger_keys_carry_context` requires every key to carry surrounding code.
//!
//! Test modules are scanned like the rest — a `#[cfg(test)]` clone gets the same
//! one-line review as any other. Oracle-free, std-only, and it rides `cargo test`, so
//! the guard lives and dies with the crate it guards.
//!
//! **What it cannot see.** A clone scanner is a syntax filter, so four re-addressing
//! routes stay outside it: (i) a `Copy` AST type needs no clone syntax at all —
//! `TSKeywordType` is `Copy` *and* address-map-keyed (`leaf(NodeKind::TSKeywordType,
//! …, addr_of(kw), …)`), so a plain `*kw` deref mints a fresh address invisibly; (ii)
//! `to_owned()` / `to_vec()` over an AST slice would copy nodes without a clone call
//! (the one live `decls.to_vec()` in `sym/declare.rs` copies `Decl` PODs, not AST);
//! (iii) clones performed in another crate are out of scope entirely; and (iv) the
//! converse — cloning a struct that merely *holds* `&'arena` references is safe, since
//! the referenced addresses are preserved, so the ledger should not be read as a ban on
//! all cloning. Keep `Copy` AST nodes borrowed for the same reason the ledger exists.

use std::path::{Path, PathBuf};

/// One reviewed, sanctioned clone site: `(crate-relative path, exact trimmed source
/// line, why it is safe)`. Every entry must be a **non-AST** clone — an owned name, a
/// diagnostic, a POD of ids and spans — never an AST node.
type Allow = (&'static str, &'static str, &'static str);

/// The reviewed ledger. Regenerate the candidate list with
/// `grep -rn '\.clone(\|\.cloned(\|Clone::clone(' crates/tsv_check/src`, then classify
/// each site by hand: what is cloned, and why cloning it can't perturb the address map.
const ALLOW: &[Allow] = &[
    // ── binder ───────────────────────────────────────────────────────────────
    (
        "src/binder/atoms.rs",
        "self.names.push(owned.clone());",
        "the interner's owned `Box<str>` name — one copy for the id vector, one as the \
         lookup key; no AST node is involved",
    ),
    (
        "src/binder/flow/build/statements.rs",
        "self.label_scratch.get(&label).cloned().unwrap_or_default()",
        "a label's pending-antecedent `SmallVec<[FlowNodeId; 4]>` — dense flow-node ids, \
         not AST nodes",
    ),
    // ── check ────────────────────────────────────────────────────────────────
    (
        "src/check/duplicate_members.rs",
        "display: key.clone(),",
        "the member key `String` — an `Entry` carries it twice, as the bucket key and as \
         the diagnostic display text",
    ),
    (
        "src/check/duplicate_members.rs",
        "Some((name.clone(), name, id.name_span()))",
        "the identifier's owned name `String`, returned as both key and display; the AST \
         identifier itself is only read (`name_span`)",
    ),
    (
        "src/check/duplicate_members.rs",
        "Expression::Literal(lit) => literal_key(ctx, lit).map(|k| (k.clone(), k, lit.span)),",
        "the literal's owned key `String`, returned as both key and display; the AST \
         literal itself is only read (`.span`)",
    ),
    (
        "src/check/duplicate_members.rs",
        "Some((keyed.clone(), keyed, pid.span))",
        "the `#name` key `String` built by `format!`, returned as both key and display; \
         the AST private identifier is only read (`.span`)",
    ),
    // ── diag ─────────────────────────────────────────────────────────────────
    (
        "src/diag.rs",
        "let a = with_chain(diag(Some(0), 0, 0, 1), vec![mid.clone()]);",
        "unit-test fixture: an owned `Diagnostic` reused as the chain of two comparands",
    ),
    (
        "src/diag.rs",
        "let a = with_related(diag(Some(0), 0, 0, 1), vec![outer_r.clone()]);",
        "unit-test fixture: an owned `Diagnostic` reused as the related info of two \
         comparands",
    ),
    // ── merge ────────────────────────────────────────────────────────────────
    (
        "src/merge.rs",
        "lib_files: libs.iter().map(|l| l.name.clone()).collect(),",
        "lib file-name `String`s copied into the base's path table",
    ),
    (
        "src/merge.rs",
        "let entry = globals.entry(sym.name.clone()).or_insert_with(|| LibEntry {",
        "the merge symbol's owned name `String` as a globals map key — `FileMerge` is \
         deliberately AST-free and program-independent",
    ),
    (
        "src/merge.rs",
        "source.name.clone(),",
        "the merge symbol's owned name `String` as the globals map key",
    ),
    (
        "src/merge.rs",
        "name: source.name.clone(),",
        "the same owned name `String`, stored on the `GlobalEntry` for diagnostics",
    ),
    (
        "src/merge.rs",
        "decls: source.decls.clone(),",
        "`Vec<MergeDecl>` — owned `{FileId, Span, bool}` PODs, no AST reference",
    ),
    (
        "src/merge.rs",
        "target.decls.extend(source.decls.iter().cloned());",
        "the same owned `MergeDecl` PODs, accumulated onto the merge target",
    ),
    (
        "src/merge.rs",
        "let symbol_name = source.name.clone();",
        "the merge symbol's owned name `String`, borrowed by both `add_dup_errors` calls",
    ),
    // ── program ──────────────────────────────────────────────────────────────
    (
        "src/program.rs",
        "diagnostics.extend(unit.bind_diagnostics.iter().cloned());",
        "owned `Diagnostic`s copied out of the variant-independent bound product so each \
         variant's run gets its own vector",
    ),
    (
        "src/program.rs",
        "name: u.name.clone(),",
        "the unit's file-name `String` for its `FileReport`",
    ),
    (
        "src/program.rs",
        "parse: u.parse.clone(),",
        "`ParseReport` — an owned goal / module-ness / node-count record (or a rejection \
         message), never the AST",
    ),
    (
        "src/program.rs",
        ".map(|d| (d.args.clone(), d.span.start, d.span.end))",
        "unit-test fixture: a diagnostic's owned `Vec<String>` args, for the assertion's \
         failure summary",
    ),
];

/// The clone-shaped call forms the scan recognizes: the method calls `.clone(` /
/// `.cloned(` / `.clone_from(`, and the qualified forms `Clone::clone(` and `>::clone(`
/// (which catches UFCS `<T as Clone>::clone(x)`). Classification is an `any`, so the
/// overlap between the two qualified forms costs nothing.
const PATTERNS: [&str; 5] = [
    ".clone(",
    ".cloned(",
    ".clone_from(",
    "Clone::clone(",
    ">::clone(",
];

/// The crate directory, prefixed onto reported paths so a violation line resolves from
/// the workspace root — where `cargo test` runs. Ledger keys stay crate-relative.
const CRATE_DIR: &str = "crates/tsv_check";

/// The floor on an [`ALLOW`] key's length. A key is a blanket sanction for every
/// identical line in its file, so it must carry enough surrounding code to be specific;
/// a bare `.clone();` left behind by a chain wrap must not qualify.
const MIN_ALLOW_KEY_LEN: usize = 16;

/// What a new, unreviewed clone site costs — printed beside every violation, because
/// the failure mode this guards is silent and the fix depends on what was cloned.
const HAZARD_HELP: &str = "\
Cloning an AST node mints a differently-addressed copy, and the binder's address map
keys on `(address, NodeKind)` — so the copy resolves to nothing. The strict path
(`BoundFile::require_node_id`, the flow builder) aborts on that miss, but the lenient
unreachable-candidate lookup just skips the node, leaving a silently wrong candidate
table instead of a crash.

Two ways out: borrow the node (`&'arena`) instead of cloning it — or, if this is
genuinely a non-AST clone (an owned name, a diagnostic, a POD of ids and spans), add a
reviewed entry to ALLOW in crates/tsv_check/tests/clone_discipline.rs naming what is
cloned and why it cannot perturb the address map.";

/// What a stale ledger entry means, and the one thing to do about it.
const STALE_HELP: &str = "\
The sanctioned line is no longer in the source — the clone was removed, or the line was
reformatted (which invalidates its review). Remove the entry from ALLOW in
crates/tsv_check/tests/clone_discipline.rs.";

/// A detected clone-shaped call site.
struct Site {
    path: String,
    line_no: usize,
    code: String,
}

#[test]
fn every_clone_site_is_reviewed() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = crate_root.join("src");
    let mut files = Vec::new();
    // An unreadable path — file OR directory — means the scan covered less than it
    // claims, which would let a clone through silently. Both land in one loud list.
    let mut unreadable: Vec<PathBuf> = Vec::new();
    collect_rs_files(&src, &mut files, &mut unreadable);
    assert!(
        !files.is_empty(),
        "no .rs files found under {} — the scan would pass vacuously",
        src.display()
    );

    let mut sites: Vec<Site> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            unreadable.push(file.clone());
            continue;
        };
        let rel = crate_relative(file, crate_root);
        for (i, line) in text.lines().enumerate() {
            if let Some(code) = clone_site_line(line) {
                sites.push(Site {
                    path: rel.clone(),
                    line_no: i + 1,
                    code,
                });
            }
        }
    }
    assert!(
        unreadable.is_empty(),
        "unreadable source path(s), so the scan is incomplete: {unreadable:?}"
    );

    let violations: Vec<&Site> = sites.iter().filter(|s| !is_allowed(s)).collect();
    // A sanctioned line that no longer exists: the clone was removed, or the line was
    // reformatted (which invalidates the review). The ledger must mirror the live sites
    // exactly, so a dead entry fails too.
    let stale: Vec<&Allow> = ALLOW
        .iter()
        .filter(|entry| !sites.iter().any(|s| s.path == entry.0 && s.code == entry.1))
        .collect();

    let mut report: Vec<String> = Vec::new();
    if !violations.is_empty() {
        report.push(format!(
            "{} unreviewed clone site(s) in tsv_check:\n",
            violations.len()
        ));
        for v in &violations {
            report.push(format!(
                "  {CRATE_DIR}/{}:{}: {}",
                v.path, v.line_no, v.code
            ));
        }
        report.push(format!("\n{HAZARD_HELP}"));
    }
    if !stale.is_empty() {
        if !report.is_empty() {
            report.push(String::new());
        }
        report.push(format!("{} stale ALLOW entr(y/ies):\n", stale.len()));
        for (path, line, reason) in &stale {
            report.push(format!("  {path}: {line}\n      ({reason})"));
        }
        report.push(format!("\n{STALE_HELP}"));
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}

#[test]
fn allow_ledger_has_no_duplicate_keys() {
    // (path, line) must be unique: a second entry for the same key is dead weight that
    // can never go stale on its own, so it would silently outlive its review.
    let mut seen = std::collections::BTreeSet::new();
    for (path, line, _) in ALLOW {
        assert!(
            seen.insert((*path, *line)),
            "duplicate ALLOW key: {path}: {line}"
        );
    }
}

#[test]
fn detector_recognizes_the_clone_forms() {
    assert_eq!(
        clone_site_line("let b = a.clone();").as_deref(),
        Some("let b = a.clone();")
    );
    assert_eq!(
        clone_site_line("\t\txs.iter().cloned().collect()").as_deref(),
        Some("xs.iter().cloned().collect()")
    );
    assert_eq!(
        clone_site_line("dst.clone_from(&src);").as_deref(),
        Some("dst.clone_from(&src);")
    );
    assert_eq!(
        clone_site_line("let b = Clone::clone(&a);").as_deref(),
        Some("let b = Clone::clone(&a);")
    );
    // The UFCS form names no `Clone::` path of its own.
    assert_eq!(
        clone_site_line("let b = <Program as Clone>::clone(a);").as_deref(),
        Some("let b = <Program as Clone>::clone(a);")
    );
    // A derive is not a call site.
    assert_eq!(clone_site_line("#[derive(Clone)]"), None);
    assert_eq!(clone_site_line("let x = 1 + 2;"), None);
}

#[test]
fn detector_ignores_comments() {
    // Doc and line comments discussing clones are prose, not call sites.
    assert_eq!(clone_site_line("//! one `.clone()` breaks the map"), None);
    assert_eq!(clone_site_line("/// never call `.clone()` here"), None);
    assert_eq!(
        clone_site_line("    // node.clone() would mint a copy"),
        None
    );
    // …but a real clone with trailing commentary still counts, keyed on the whole line.
    assert_eq!(
        clone_site_line("let n = name.clone(); // owned String").as_deref(),
        Some("let n = name.clone(); // owned String")
    );
}

#[test]
fn detector_sees_past_a_slash_slash_inside_a_string() {
    // The crate embeds TS fixtures carrying `//`; cutting the line there would hide
    // every clone after it. Both a URL and an embedded line comment must stay open.
    assert_eq!(
        clone_site_line("let s = \"https://x\"; s.clone();").as_deref(),
        Some("let s = \"https://x\"; s.clone();")
    );
    assert_eq!(
        clone_site_line("let src = \"a();\\n\\t// @ts-ignore\\n\"; src.clone();").as_deref(),
        Some("let src = \"a();\\n\\t// @ts-ignore\\n\"; src.clone();")
    );
    // A real comment after a balanced string still ends the code.
    assert_eq!(clone_site_line("let s = \"//\"; // s.clone() here"), None);
}

#[test]
fn allow_ledger_keys_carry_context() {
    // A key blanket-sanctions every identical line in its file, so it has to be
    // specific: it must not be (or start as) a bare clone call, and must carry
    // surrounding code. A `.clone();` left by a rustfmt chain wrap fails both.
    for (path, line, _) in ALLOW {
        assert!(
            !PATTERNS.iter().any(|pattern| line.starts_with(pattern)),
            "ALLOW key is a bare clone call, so it would sanction any such line in \
             {path}: {line}"
        );
        assert!(
            line.len() >= MIN_ALLOW_KEY_LEN,
            "ALLOW key is too generic ({} < {MIN_ALLOW_KEY_LEN} chars) in {path}: {line}",
            line.len()
        );
    }
}

/// Whether `site` matches an [`ALLOW`] entry on path **and** exact trimmed line.
fn is_allowed(site: &Site) -> bool {
    ALLOW
        .iter()
        .any(|(path, line, _)| *path == site.path && *line == site.code)
}

/// The trimmed text of `line` if its code carries a clone-shaped call, else `None`.
///
/// The returned key is the *whole* trimmed line, trailing comment included — the ledger
/// sanctions a line as written.
fn clone_site_line(line: &str) -> Option<String> {
    let code = code_before_comment(line);
    PATTERNS
        .iter()
        .any(|pattern| code.contains(pattern))
        .then(|| line.trim().to_string())
}

/// The code portion of `line`: everything ahead of the first `//` that actually starts
/// a comment. Cutting there subsumes the whole-line-comment case (`//`, `///` and `//!`
/// all leave nothing but indentation ahead of it) and drops trailing commentary, so
/// prose about cloning never trips the scan — but a `//` inside a string literal is
/// data, not a comment, and is skipped so a clone later on the line stays visible.
fn code_before_comment(line: &str) -> &str {
    let mut from = 0;
    while let Some(rel) = line[from..].find("//") {
        let at = from + rel;
        if !ends_inside_string(&line[..at]) {
            return &line[..at];
        }
        from = at + 2;
    }
    line
}

/// Whether `prefix` ends inside a double-quoted literal — an odd number of `"` that
/// aren't backslash-escaped.
///
/// A heuristic, and deliberately biased: raw strings (`r#"…"#`), byte strings and a
/// `'"'` char literal all read as an unbalanced quote, which makes this answer *yes*
/// and keeps **more** of the line in scope. So a misjudgment can only surface a site
/// for review (a false positive the ledger resolves), never hide one.
fn ends_inside_string(prefix: &str) -> bool {
    let bytes = prefix.as_bytes();
    let mut quotes = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // Skip the escaped byte, so `\"` never counts as a delimiter.
            b'\\' => i += 1,
            b'"' => quotes += 1,
            _ => {}
        }
        i += 1;
    }
    quotes % 2 == 1
}

/// Every `.rs` file under `dir`, recursively, in deterministic (sorted) order. A
/// directory that cannot be read is recorded in `unreadable` rather than skipped — a
/// silently pruned subtree is a hole in the scan.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>, unreadable: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        unreadable.push(dir.to_path_buf());
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_rs_files(&path, out, unreadable);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Crate-relative path: `<CARGO_MANIFEST_DIR>/src/merge.rs` → `src/merge.rs`.
fn crate_relative(path: &Path, crate_root: &Path) -> String {
    path.strip_prefix(crate_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
