use crate::cli::CliError;
use crate::compile_fixtures::{COMPILE_FIXTURES_DIR, walk_compile_fixtures};
use argh::FromArgs;
use std::path::Path;
use tsv_cli::json_utils::to_json_with_tabs;

/// Audit compile-fixture divergence integrity (the compiler analog of
/// `conformance_audit`, deliberately minimal).
///
/// A `_compiled_divergence`-suffixed compile fixture asserts a deliberate,
/// sanctioned difference from the canonical Svelte compiler's output. That
/// catalog (`docs/conformance_svelte_compiler.md`) is expected to stay empty —
/// it is a safety valve, not a tolerance budget — so this audit mostly asserts
/// emptiness:
///
/// 1. **Orphans** — every `_compiled_divergence` fixture must be linked in the
///    catalog document.
/// 2. **Missing back-links** — every such fixture must carry a `README.md`
///    containing a link that resolves to the catalog document.
///
/// Both are per-fixture, so with an empty catalog they check nothing; they are a
/// tripwire armed for a future entry. The third check does not depend on one:
///
/// 3. **Checklist ↔ `Refusal` drift** — every bucket key
///    `docs/checklist_svelte_compiler.md` quotes must be one the refusal catalog
///    can actually produce (see [`audit_refusal_keys`] for why only this
///    direction gates).
///
/// Pure Rust, no sidecar. Exits non-zero on any finding. Gated in
/// `deno task check`.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_conformance_audit")]
pub struct CompileConformanceAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

const CATALOG_DOC: &str = "docs/conformance_svelte_compiler.md";
const CHECKLIST_DOC: &str = "docs/checklist_svelte_compiler.md";
const DIVERGENCE_SUFFIX: &str = "_compiled_divergence";

#[derive(serde::Serialize)]
struct AuditReport {
    divergence_fixtures: usize,
    orphans: Vec<String>,
    missing_backlinks: Vec<String>,
    /// Keys the checklist quotes that no `Refusal` variant produces (gating).
    stale_refusal_keys: Vec<String>,
    /// Bucket keys the checklist never quotes (report-only — see the check's docs).
    unmentioned_refusal_keys: Vec<String>,
}

impl CompileConformanceAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let doc = match std::fs::read_to_string(CATALOG_DOC) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {CATALOG_DOC}: {e}");
                return Err(CliError::Failed);
            }
        };

        // A missing tree is an empty tree (nothing to audit).
        let root = Path::new(COMPILE_FIXTURES_DIR);
        let fixtures = if root.exists() {
            walk_compile_fixtures(root).map_err(|e| {
                eprintln!("Error walking compile fixtures: {e}");
                CliError::Failed
            })?
        } else {
            Vec::new()
        };

        let checklist = match std::fs::read_to_string(CHECKLIST_DOC) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {CHECKLIST_DOC}: {e}");
                return Err(CliError::Failed);
            }
        };
        let (stale_refusal_keys, unmentioned_refusal_keys) = audit_refusal_keys(&checklist);

        let mut report = AuditReport {
            divergence_fixtures: 0,
            orphans: Vec::new(),
            missing_backlinks: Vec::new(),
            stale_refusal_keys,
            unmentioned_refusal_keys,
        };

        for fixture in &fixtures {
            if !fixture.relative_path.ends_with(DIVERGENCE_SUFFIX) {
                continue;
            }
            report.divergence_fixtures += 1;
            if !doc.contains(&fixture.relative_path) {
                report.orphans.push(fixture.relative_path.clone());
            }
            let readme = fixture.path.join("README.md");
            let has_backlink = std::fs::read_to_string(&readme)
                .is_ok_and(|content| content.contains("conformance_svelte_compiler.md"));
            if !has_backlink {
                report.missing_backlinks.push(fixture.relative_path.clone());
            }
        }

        let findings =
            report.orphans.len() + report.missing_backlinks.len() + report.stale_refusal_keys.len();

        if self.json {
            match to_json_with_tabs(&report) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("Error serializing report: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else {
            for orphan in &report.orphans {
                println!("ORPHAN: {orphan} not cataloged in {CATALOG_DOC}");
            }
            for missing in &report.missing_backlinks {
                println!("MISSING BACK-LINK: {missing} README must link {CATALOG_DOC}");
            }
            for stale in &report.stale_refusal_keys {
                println!(
                    "STALE REFUSAL KEY: {CHECKLIST_DOC} quotes `{stale}`, which no Refusal variant produces"
                );
            }
            if !report.unmentioned_refusal_keys.is_empty() {
                println!(
                    "note: {} bucket key(s) have no `**Refused**:` bullet in {CHECKLIST_DOC} (report-only)",
                    report.unmentioned_refusal_keys.len()
                );
            }
            println!(
                "compile_conformance_audit: {} divergence fixture(s), {} refusal key(s) quoted of {} produced, {} finding(s)",
                report.divergence_fixtures,
                quoted_refusal_keys(&checklist).len(),
                tsv_svelte_compile::Refusal::all_bucket_keys().len(),
                findings
            );
        }

        if findings > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

//
// Check 3 — checklist ↔ `Refusal` drift
//

/// Compare the refusal bucket keys `docs/checklist_svelte_compiler.md` quotes
/// against the keys the [`Refusal`](tsv_svelte_compile::Refusal) catalog actually
/// produces, returning `(stale, unmentioned)`.
///
/// The checklist's Coverage section claims its **Refused** bullets quote bucket
/// keys verbatim, so the document "maps one-to-one onto corpus runs". Nothing
/// verified that, and a renamed or deleted variant leaves the claim quietly false.
///
/// The two directions are graded differently on purpose:
///
/// - **stale** (a quoted key no variant produces) is a **gating** finding — the
///   doc asserts a bucket a corpus run can never report, which is the claim being
///   false in the direction a reader is misled by.
/// - **unmentioned** (a producible key the doc never quotes) is **report-only**.
///   A variant needs no checklist row to be correct: several are internal or are
///   covered by a prose paragraph rather than a `**Refused**:` bullet, so gating
///   this direction would be born red and would pressure the doc toward a
///   mechanical key dump rather than a coverage map.
fn audit_refusal_keys(checklist: &str) -> (Vec<String>, Vec<String>) {
    let produced: std::collections::BTreeSet<String> =
        tsv_svelte_compile::Refusal::all_bucket_keys()
            .iter()
            .map(|k| strip_backticks(k))
            .collect();
    let quoted = quoted_refusal_keys(checklist);

    let stale = quoted
        .iter()
        .filter(|k| !produced.contains(*k))
        .cloned()
        .collect();
    let unmentioned = produced
        .iter()
        .filter(|k| !quoted.contains(*k))
        .cloned()
        .collect();
    (stale, unmentioned)
}

/// Drop every backtick from a key, the one normalization applied to both sides.
fn strip_backticks(key: &str) -> String {
    key.replace('`', "").trim().to_string()
}

/// Every bucket key quoted by a `- **Refused**: `-prefixed checklist bullet.
///
/// The key is the bullet's leading code span (a ``` `` ```-fenced span when the
/// span contains a backtick). Both sides are compared with backticks stripped
/// ([`strip_backticks`]): a key may legitimately *contain* them (the `` `$:` ``
/// in the legacy-reactive-statement key), while the doc also decorates some
/// placeholders it quotes (`` `{name}` ``) — stripping is the one normalization
/// that reads both spellings the same. A bullet that opens with prose instead of
/// a code span makes no verbatim-key claim and is skipped.
fn quoted_refusal_keys(checklist: &str) -> std::collections::BTreeSet<String> {
    const MARKER: &str = "- **Refused**:";
    checklist
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix(MARKER)?.trim_start();
            let (fence, body) = if let Some(b) = rest.strip_prefix("``") {
                ("``", b)
            } else {
                ("`", rest.strip_prefix('`')?)
            };
            let key = body.split_once(fence)?.0;
            Some(strip_backticks(key))
        })
        .filter(|k| !k.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_refusal_keys_reads_both_fence_widths() {
        let md = "\
- **Refused**: `plain key {name}` — prose after the key.\n\
- **Refused**: `` key with a `$:` backtick `` — prose.\n\
- **Refused**: prose-first bullet with no code span\n\
- **Supported**: `not a refusal bullet`\n";
        let keys = quoted_refusal_keys(md);
        let got: Vec<&str> = keys.iter().map(String::as_str).collect();
        assert_eq!(got, vec!["key with a $: backtick", "plain key {name}"]);
    }

    #[test]
    fn every_quoted_key_is_producible() {
        // The gating direction, exercised on a synthetic doc so the test states
        // the rule rather than re-reading the live checklist.
        let real = tsv_svelte_compile::Refusal::ClientGeneration
            .bucket_key()
            .into_owned();
        let md = format!("- **Refused**: `{real}`\n- **Refused**: `no variant emits this`\n");
        let (stale, _) = audit_refusal_keys(&md);
        assert_eq!(stale, vec!["no variant emits this".to_string()]);
    }

    /// The bucket keys `Refusal::every_variant()` is expected to produce, sorted.
    ///
    /// Pinned as the full SET rather than as a count: `every_variant` is
    /// hand-maintained and not compiler-enforced, and a count alone is blind to
    /// every way it can be wrong *without changing size* — adding one variant
    /// while dropping another, listing one variant twice in place of a missing
    /// one, or giving a variant a representative whose parameters render the
    /// wrong placeholder. Each of those changes the key set, so each fails here
    /// with a readable diff.
    ///
    /// Regenerate by printing `Refusal::all_bucket_keys()` (a sorted
    /// `BTreeSet`) — never by hand-editing a single line to match a failure.
    const EXPECTED_BUCKET_KEYS: &[&str] = &[
        "$-prefixed binding {name}",
        "$-prefixed identifier {name}",
        "$props() binding pattern (not an identifier or object pattern — the oracle rejects it)",
        "$props() used more than once",
        "$props.id() outside a plain top-level variable declaration",
        "$props.id() used more than once",
        "--custom-property attribute on <{name}> component",
        "<option> (oracle emits $$renderer.option closures)",
        "<svelte:boundary> {name}={…} attribute form",
        "<svelte:head> alongside a {@const} in the same fragment (hoist order)",
        "<svelte:options>",
        "<{name}> cannot have children (the oracle rejects it)",
        "<{name}> component with both a children prop and default children",
        "<{name}> must be a top-level element (the oracle rejects it)",
        "<{name}> with children",
        "TS enum (the oracle rejects it)",
        "TS namespace/module with a value member (the oracle rejects it)",
        "TS parameter property with readonly/accessibility (the oracle rejects it)",
        "TypeScript syntax without lang=\"ts\" (the oracle parse-errors)",
        "abstract class property (the oracle emits invalid JS)",
        "accessor class field (the oracle rejects it)",
        "assignment to a constant (a const declarator or import local — the oracle's constant_assignment)",
        "assignment to an {#each} item (the oracle's each_item_invalid_assignment)",
        "assignment to a {#snippet} parameter (the oracle's snippet_parameter_assignment)",
        "attribute on <title> (the oracle rejects it)",
        "attributes on <svelte:head>",
        "bind: directive on <{name}> component",
        "bind: directive {name}",
        "binding pattern shape ({kind})",
        "binding {name} declared in both the module and instance scripts",
        "block-scope binding {name} shadows a $derived binding",
        "bodiless class method (overload signature — the oracle rejects it)",
        "children on void element <{name}>",
        "class-field $state with a lone store/$derived argument (the oracle keeps it bare)",
        "class: directive alongside a mixed-value class attribute",
        "client generation",
        "comment after the last script statement in a template that emits a nested block (the oracle drops it)",
        "comment in a module script placed after the instance script (the oracle re-attaches it into the template)",
        "comment inside a rewritten rune region (dropped by the transform)",
        "comment inside an erased TypeScript region",
        "comments in a script alongside a multi-declarator declaration (the oracle re-anchors comments inside the split)",
        "comments in a script that references a store ($$store_subs injection)",
        "comments in a script with a $$slots reference (injected sanitize_slots)",
        "comments in a script with a $bindable() prop default",
        "comments in a script with a $props.id() declarator",
        "comments in a script with a non-destructured $props() (injected $$slots/$$events)",
        "comments in a script with a rest-element $props() (injected $$slots/$$events)",
        "comments in a script with an argument-less $state()",
        "comments with template markup before the script (window ordering)",
        "conflicting transition directives (an element may have at most one intro and one outro — the oracle rejects it)",
        "css at-rule in <style>",
        "css attribute selector against a dynamic attribute value (static-eval not ported)",
        "css case-insensitive match with a non-ASCII operand (Unicode case-fold not ported)",
        "css combinator selector in <style>",
        "css selector {selector} matches no element",
        "decorator (the oracle rejects it)",
        "default export in <script module> (the oracle rejects it)",
        "destructured {@const} (only `{@const name = …}`)",
        "destructuring a $derived declarator",
        "destructuring a $derived.by declarator",
        "destructuring a $state declarator",
        "destructuring a $state.snapshot declarator",
        "dev mode output",
        "directive on <{name}> component",
        "dotted TS namespace A.B (the oracle crashes on it)",
        "duplicate <{name}> element (the oracle rejects it)",
        "duplicate {#snippet} {name} (the oracle rejects it)",
        "dynamic <{name}> component (member or reactive binding)",
        "dynamic class attribute on a styled component",
        "dynamic style attribute on a styled component",
        "empty css rule in <style> (the oracle comment-wraps it)",
        "event attribute {name}",
        "event capture attribute on a load-error element",
        "export = … (the oracle emits invalid JS)",
        "export as namespace … (the oracle emits invalid JS)",
        "format-ignore directive comment in script",
        "generated name {name} collides with a user binding",
        "generics attribute on <script> (implies TypeScript)",
        "import from svelte/internal (forbidden)",
        "import x = require(…) (the oracle emits invalid JS)",
        "index signature in a class body (the oracle crashes on it)",
        "instance-script export (component exports / $.bind_props not implemented)",
        "interpolated {name} attribute on a styled component",
        "invalid <title> content (only text and {expression} — the oracle rejects it)",
        "invalid animate: directive (one per element, only on the sole child of a keyed {#each} — the oracle rejects it)",
        "invalid attribute on <svelte:boundary> (the oracle rejects it)",
        "invalid attribute on <{name}> (the oracle rejects it)",
        "lang=\"{lang}\" script",
        "leading comment glued to the <script> line (no newline before it)",
        "legacy reactive statement `$:` (invalid in runes mode)",
        "legacy {directive} directive (runes-only fence)",
        "member/call rooted at an escaped identifier (classification not ported)",
        "member/call rooted at prop/import {name} also bound in a nested scope",
        "multi-line block comment in script (interior-line re-indentation not carried through)",
        "mutation inside a template expression",
        "named slot on <{name}> component",
        "nested css rule in <style>",
        "nested {#each} (the nested emission path is not yet validated)",
        "non-expression value for <svelte:boundary> attribute {name} (the oracle rejects it)",
        "read of derived binding {name}",
        "read of derived binding {name} shadowed in a nested scope",
        "rune {name}",
        "rune {name} whose base is also an instance binding",
        "runes-invalid import of {name} from svelte",
        "static evaluation not portable",
        "static fold not portable",
        "store destructuring write",
        "store member write ($.store_mutate)",
        "store subscription whose base is not a top-level component binding",
        "string-literal expression attribute value (inline-literal path)",
        "style: directive alongside a mixed-value style attribute",
        "style: directive with a mixed-value (text + expression) value",
        "style: directive with an invalid modifier (only |important, once, is allowed)",
        "template comments (only instance-script comments are carried through)",
        "template node {kind}",
        "template-level <{name}>",
        "top-level await (async component output not implemented)",
        "unsupported css selector in <style> (:global/:is/:where/:has/:not/:root/nesting)",
        "use: directive on a load-error element (event-capture markup not implemented)",
        "value attribute on <{name}>",
        "{#snippet} alongside a {@const}/<svelte:head> in the same fragment (hoist order)",
        "{#snippet} rest parameter (the oracle rejects it)",
        "{#snippet} signature the parser fell back to raw text for",
        "{#snippet} with an escaped name",
        "{#snippet} {name} hoist classification ambiguous",
        "{...spread} on <select> (the oracle routes to $$renderer.select)",
        "{...spread} on a load-error element (event-capture markup not implemented)",
        "{@const} at the component root (only valid inside a block)",
        "{@const} outside a block scope",
        "{@const} with a non-plain binding name",
        "{@html} with a statically-known value",
        "{@render} callee is not a resolvable local snippet or snippet prop",
    ];

    #[test]
    fn all_bucket_keys_covers_the_catalog() {
        // A representative per variant, so the audit's oracle is the whole catalog
        // rather than whichever variants happen to be constructed elsewhere.
        let actual: Vec<String> = tsv_svelte_compile::Refusal::all_bucket_keys()
            .into_iter()
            .collect();
        let expected: Vec<String> = EXPECTED_BUCKET_KEYS
            .iter()
            .map(|k| (*k).to_string())
            .collect();
        let missing: Vec<&String> = expected.iter().filter(|k| !actual.contains(k)).collect();
        let unexpected: Vec<&String> = actual.iter().filter(|k| !expected.contains(k)).collect();
        assert!(
            missing.is_empty() && unexpected.is_empty(),
            "Refusal bucket-key set drifted from EXPECTED_BUCKET_KEYS.\n  \
             no longer produced: {missing:#?}\n  newly produced: {unexpected:#?}\n\
             If a variant was added, add it to `every_variant` AND to \
             EXPECTED_BUCKET_KEYS."
        );
        // The set is injective over the catalog today (no two variants share a
        // bucket key), so the set comparison above sees every representative.
        // Pin that too — were it to stop holding, a duplicated representative
        // could hide behind an equal set.
        assert_eq!(
            tsv_svelte_compile::Refusal::every_variant().len(),
            actual.len(),
            "two `every_variant` representatives now collapse to one bucket key; \
             the key-set pin above no longer sees every variant"
        );
    }
}
