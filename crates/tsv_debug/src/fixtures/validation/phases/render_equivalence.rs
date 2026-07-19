//! Render-equivalence validation: a whitespace-only variant must render
//! identically to `input`.
//!
//! An `unformatted_*` / `unformatted_ours_*` variant is asserted elsewhere only
//! to *normalize to* `input` (`ours(variant) == input`, N4/N5) — never to be
//! *render-equivalent* to it. So a formatter bug that changes the rendered output
//! AND happens to land on `input` would pass every other gate green. It is worst
//! for `unformatted_ours_*`, where N6 makes prettier deliberately *disagree*
//! (`prettier(variant) != input`), leaving `ours` — the formatter under test — as
//! the sole witness to the variant↔input relationship. `unformatted_*` is only
//! transitively covered via N3 (`prettier(variant) == input`, sound only if
//! prettier is render-faithful). The structure checks (S4/S7) assert merely
//! `variant != input`, not "differs only in render-insignificant whitespace".
//!
//! This phase closes that hole for Svelte templates, asserting the variant and
//! `input` render the same **independent of the formatter**.
//!
//! ## Oracle (hybrid)
//!
//! - **Compile arm (authoritative).** Compare the two sources' browser-visible
//!   **render keys** (`svelte compile --generate server` → baked template text,
//!   holes for `${…}`, HTML comments stripped, whitespace runs collapsed; see
//!   `deno::svelte_render_key`). Equal keys prove equal renders. Because the key
//!   is baked-template-only, a `<script>`/`<style>` reformatting that leaves the
//!   template unchanged shares a key — so this arm judges the *render*, not the
//!   code. Used whenever both sides compile.
//! - **Fallback arm.** `compile` runs the full semantic **analyzer**, far stricter
//!   than the parser, and synthetic parser/formatter fixtures routinely violate it:
//!   TS features needing a preprocessor, experimental `await`, an illegal default
//!   export, a `bind:` to an undeclared or non-assignable target, duplicate
//!   declarations, invalid node placement, CSS analysis errors (~6% of
//!   variant-bearing fixtures). Those errors are unrelated to rendering, and
//!   `runes: false` does not avoid them. When either side won't compile, fall back
//!   to a **template-only** [`crate::render_browser`] compare (canonical parse,
//!   `instance`/`module`/`css` erased, Svelte-5 whitespace normalization).
//!   Template-only because a script-only difference (e.g. a dropped
//!   `EmptyStatement`, `a();;` → `a();`) is a formatter normalization, not a render
//!   change. On top of the Svelte 5 compiler model it applies the *browser* model
//!   ([`crate::render_browser`]): block-boundary whitespace vanishes, and a quoted
//!   single-expression attribute value compares equal to its bare spelling.
//!   The model still **over-flags by construction** — it compares expression and
//!   structure syntax (parens, comment position, `{#await x then y}` ↔
//!   `{#await x}{:then y}`) that never reaches the render — so its divergences are
//!   gated against the hand-verified [`BENIGN_FALLBACK_DIVERGENCES`] allow-list
//!   rather than trusted outright: an unlisted one fails, and a listed one that
//!   stops firing fails as stale.

use serde_json::Value;

use crate::deno;
use crate::diff;
use crate::fixtures::{Fixture, FixtureFiles, InputType, read_file};
use crate::render_browser::browser_normalize_pair;

use super::super::FixtureValidation;
use super::super::errors::ValidationError;

/// Fallback-arm divergences confirmed BENIGN by hand, keyed by the fixture path
/// (relative to `tests/fixtures/`) plus its variant file.
///
/// ⚠️ **Unlike the `gap_audit` / `blank_audit` ratchets, a line here is NOT a known
/// bug** — it is a known FALSE POSITIVE of the weak fallback oracle (see the module
/// docs: it compares expression/structure syntax that never reaches the render).
/// Shrinking this list means **improving the oracle**, never fixing the formatter.
/// The compile arm is unaffected: an authoritative divergence always fails, and is
/// never allow-listed.
///
/// Each entry was verified authoritatively by compiling both sides with the `bind:`
/// targets declared as `$state` — the same transform applied to both — and comparing
/// the generated server output. In every case the compile arm would have returned
/// "equivalent" had the fixture been analyzable; they land here only because Svelte's
/// semantic analyzer rejects the fixture (a `bind:` to an undeclared or non-assignable
/// target), so the compile arm never runs.
///
/// The list is ratcheted: a fallback divergence NOT listed here fails, and a listed
/// entry that no longer fires fails as stale (so a fixed oracle forces a re-pin).
const BENIGN_FALLBACK_DIVERGENCES: &[&str] = &[
    // Paren + multiline-comment position inside a directive expression. Verified: the
    // generated JS differs only in a *comment's* indentation, never in template text.
    // Retiring these needs the fallback to hole out expression subtrees — i.e. to
    // reimplement what the compile arm already does; deliberately not pursued.
    "svelte/directives/value_paren_multiline_comment_prettier_divergence/unformatted_bare.svelte",
    "svelte/directives/value_paren_multiline_comment_prettier_divergence/unformatted_ours_paren.svelte",
    // `{#await x then y}` ↔ `{#await x}{:then y}` — the block's structural shape
    // differs (shorthand has no pending branch, the explicit form an empty one).
    // Verified: compiles byte-identical. Retiring it needs await-shorthand structural
    // normalization in the fallback; narrow, and the compile arm covers the class.
    "svelte/syntax/comments/expr_trailing_prettier_divergence/unformatted_ours_await_block.svelte",
];

/// Ratchet [`BENIGN_FALLBACK_DIVERGENCES`] for staleness: every listed entry must
/// still fire somewhere in the run. A stale entry means the fallback oracle improved,
/// or the fixture changed/moved — either way the list must be re-pinned, the same
/// discipline the `gap_audit` / `blank_audit` ratchets apply.
///
/// Only meaningful on an UNFILTERED run: a narrowed run visits too few fixtures, so
/// the caller skips it when filters are active.
pub(in crate::fixtures::validation) fn stale_benign_entries(
    fired: &std::collections::HashSet<String>,
) -> Vec<&'static str> {
    BENIGN_FALLBACK_DIVERGENCES
        .iter()
        .filter(|entry| !fired.contains(**entry))
        .copied()
        .collect()
}

/// The allow-list key for a variant: the fixture path relative to `tests/fixtures/`
/// plus the variant file name. `Fixture::relative_path` carries a `./tests/fixtures/`
/// prefix that would only add noise to every entry.
fn benign_key(fixture: &Fixture, variant_name: &str) -> String {
    let dir = fixture
        .relative_path
        .trim_start_matches("./")
        .trim_start_matches("tests/fixtures/");
    format!("{dir}/{variant_name}")
}

/// Which oracle decided a render-equivalence verdict.
#[derive(Clone, Copy)]
enum Oracle {
    /// Authoritative: equal `svelte compile --generate server` render keys.
    Compile,
    /// Fallback: the in-process template-only `render_browser` model (compile
    /// unavailable).
    Fallback,
}

/// The verdict for one variant-vs-input comparison.
enum Verdict {
    /// The variant renders identically to `input` (which arm proved it).
    Equivalent(Oracle),
    /// The variant renders differently; `input_side`/`variant_side` are the two
    /// compared artifacts (render key or normalized AST) for the triage diff.
    Divergent {
        oracle: Oracle,
        input_side: String,
        variant_side: String,
    },
    /// Neither arm could reach a verdict (parse/compile infra failure). Other
    /// phases (P1/P3 parser freshness) own reporting such failures.
    Indeterminate,
}

/// Render-equivalence phase: assert every whitespace variant renders identically
/// to `input`. Svelte templates only — `.svelte.ts` (runes, no template), `.ts`,
/// and `.css` have nothing Svelte renders.
pub(in crate::fixtures::validation) async fn validate_render_equivalence(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    files: &FixtureFiles,
) {
    if fixture.input_type() != InputType::Svelte {
        return;
    }
    if files.unformatted.is_empty()
        && files.unformatted_ours.is_empty()
        && files.prettier_variant.is_empty()
    {
        return;
    }

    // Authoritative arm: render-key `input` once and reuse for every variant.
    let input_key = deno::svelte_render_key(input).await.ok();
    // Fallback arm: `input`'s template-only normalized AST, built lazily on the
    // first fallback.
    let mut input_template: Option<Value> = None;

    // All three variant kinds share the identical check; the file lists differ
    // only in which N-rule guarantees `ours(variant) == input` upstream. A
    // `prettier_variant_*` belongs here for the same reason `unformatted_ours_*`
    // does: prettier keeps the variant (≠ input), so `ours` is the SOLE witness
    // to the variant↔input relationship — without this check a render-changing
    // normalization that lands on `input` would validate green. (`variant_*` /
    // `divergent_variant_*` stay out: ours does not map them to input, so there
    // is no variant↔input claim to prove.)
    let variants = files
        .unformatted
        .iter()
        .chain(files.unformatted_ours.iter())
        .chain(files.prettier_variant.iter());

    for variant_name in variants {
        let variant_path = fixture.path.join(variant_name);
        // The read is owned + reported by the normalization phases (N3/N4 for
        // unformatted_*, N5/N6 for unformatted_ours_*, N1/N2 for
        // prettier_variant_*); skip silently here to avoid double-reporting.
        let Ok(variant_content) = read_file(&variant_path) else {
            continue;
        };

        match render_equivalent(
            &variant_content,
            input,
            input_key.as_deref(),
            &mut input_template,
        )
        .await
        {
            Verdict::Equivalent(Oracle::Compile) => result.render_equiv_verified_compile += 1,
            Verdict::Equivalent(Oracle::Fallback) => result.render_equiv_verified_fallback += 1,

            // Compile arm (authoritative): a confirmed render difference — GATE.
            Verdict::Divergent {
                oracle: Oracle::Compile,
                input_side,
                variant_side,
            } => {
                result.add_error(ValidationError::RenderEquivalenceMismatch(
                    variant_name.clone(),
                ));
                result.add_diff(
                    &format!(
                        "render-equivalence (compile): {}/{}",
                        fixture.relative_path, variant_name
                    ),
                    &input_side,
                    &variant_side,
                    &diff::DiffOptions::freshness(),
                );
            }

            // Fallback arm: compile unavailable and the template-only model flags a
            // difference. The model over-flags by construction, so a divergence is
            // gated against the hand-verified benign allow-list: a listed one is
            // recorded (the summary ratchets it for staleness), an unlisted one FAILS
            // and must be triaged — a real render change, or a new oracle artifact to
            // verify and pin.
            Verdict::Divergent {
                oracle: Oracle::Fallback,
                input_side,
                variant_side,
            } => {
                let key = benign_key(fixture, variant_name);
                if BENIGN_FALLBACK_DIVERGENCES.contains(&key.as_str()) {
                    result.render_equiv_benign_fired.push(key);
                } else {
                    result.add_error(ValidationError::RenderEquivalenceFallbackDivergence(
                        variant_name.clone(),
                    ));
                    result.add_diff(
                        &format!(
                            "render-equivalence (fallback, template-only): {}/{}",
                            fixture.relative_path, variant_name
                        ),
                        &input_side,
                        &variant_side,
                        &diff::DiffOptions::freshness(),
                    );
                }
            }

            Verdict::Indeterminate => {}
        }
    }
}

/// Decide whether `variant` renders identically to `input`.
///
/// `input_key` is `input`'s render key (computed once per fixture); `Some` enables
/// the authoritative compile arm. `input_template` caches `input`'s template-only
/// normalized AST for the fallback arm, built lazily on first need.
async fn render_equivalent(
    variant: &str,
    input: &str,
    input_key: Option<&str>,
    input_template: &mut Option<Value>,
) -> Verdict {
    // Compile arm (authoritative): both sides must compile to a render key.
    if let Some(input_key) = input_key
        && let Ok(variant_key) = deno::svelte_render_key(variant).await
    {
        return if variant_key == input_key {
            Verdict::Equivalent(Oracle::Compile)
        } else {
            Verdict::Divergent {
                oracle: Oracle::Compile,
                input_side: input_key.to_string(),
                variant_side: variant_key,
            }
        };
    }

    // Fallback arm: the template-only render_browser model (compile unavailable
    // on a side). Erase `instance`/`module`/`css` so a script/style-only
    // reformatting — which the compile arm ignores by construction — is ignored
    // here too, leaving a pure template-render compare.
    let Ok(mut variant_ast) = deno::parse_svelte(variant).await else {
        return Verdict::Indeterminate;
    };
    strip_non_template(&mut variant_ast);

    // Build `input`'s template-only AST once, caching it for later variants.
    if input_template.is_none() {
        let Ok(mut v) = deno::parse_svelte(input).await else {
            return Verdict::Indeterminate;
        };
        strip_non_template(&mut v);
        *input_template = Some(v);
    }
    let Some(input_val) = input_template.as_ref() else {
        return Verdict::Indeterminate;
    };
    let (normalized_variant, normalized_input) =
        browser_normalize_pair(variant_ast, input_val.clone());
    if normalized_variant == normalized_input {
        Verdict::Equivalent(Oracle::Fallback)
    } else {
        Verdict::Divergent {
            oracle: Oracle::Fallback,
            input_side: serde_json::to_string_pretty(&normalized_input).unwrap_or_default(),
            variant_side: serde_json::to_string_pretty(&normalized_variant).unwrap_or_default(),
        }
    }
}

/// Erase the non-template members of a Svelte `Root` AST — `instance` / `module`
/// (`<script>`) and `css` (`<style>`) — so the fallback render compare judges the
/// template alone. A script/style reformatting is a formatter normalization, not
/// a render change; leaving those subtrees in would make `a();;` → `a();` (a
/// dropped `EmptyStatement`) read as a render divergence.
fn strip_non_template(value: &mut Value) {
    if let Value::Object(map) = value {
        for key in ["instance", "module", "css"] {
            if map.contains_key(key) {
                map.insert(key.to_string(), Value::Null);
            }
        }
    }
}
