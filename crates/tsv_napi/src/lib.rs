//! N-API bindings for tsv (Node.js / Bun).
//!
//! The Node-runtime sibling of [`tsv_ffi`](../tsv_ffi) (the Deno/C-FFI path) and
//! [`tsv_wasm`](../tsv_wasm) (the universal WASM path). napi-rs marshals the
//! JS string into a Rust `String` and the returned `String` back out, so there
//! are no raw pointers and no manual free — the cleanest of the three bindings.
//!
//! Transport mirrors `tsv_wasm`'s deliberate choice: `parse_<lang>` returns a
//! JSON **string** for the host to `JSON.parse`, rather than building the object
//! graph node-by-node across the boundary (measurably slower). `format_<lang>`
//! returns the formatted source directly. Engine errors surface as thrown JS
//! errors (`napi::Error`); `parse_internal_<lang>` parses without converting
//! (benchmark-only, AST kept live via `black_box`).
//!
//! **Panic contract**: every export carries `#[napi(catch_unwind)]`, and the
//! addon builds with the workspace `napi` profile (`release` + `panic =
//! "unwind"`), so a Rust panic — always a tsv bug — surfaces as a thrown JS
//! error rather than aborting the host process, and the per-thread arenas stay
//! usable afterwards (`tsv_arena`'s take/park protocol leaves the slot empty
//! while a call runs, so unwind and abort converge on the same state). The
//! attribute is inert without the unwinding profile — both halves are required.
//! Stack overflow is NOT catchable and still aborts the host.
//!
//! Built as a `cdylib` and loaded by Node as a `.node` addon. The `format` /
//! `parse` cargo features gate which entry points are emitted (mirrors
//! `tsv_ffi` / `tsv_wasm`).

use napi_derive::napi;

#[cfg(feature = "format")]
use napi::bindgen_prelude::{Either, Undefined};
// Per-thread reusable arenas live in the shared `tsv_arena` crate (used by both
// native bindings — see its module docs for the reuse rationale + soundness).
// The goal-axis macros come from the same crate, so the three bindings share ONE
// definition of which languages have a goal rather than three hand-synced copies.
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;
#[cfg(any(feature = "parse", feature = "format"))]
use tsv_arena::{goal_allowed, parse_ast};

/// Decode the optional goal argument (`"script"` / `"module"`; omitted or
/// `undefined` means `"module"`).
///
/// `allowed` is the language's goal axis ([`goal_allowed!`]). A goal against a
/// language that has none is an **error**, not a silent Module: Svelte hard-wires
/// `Module` and CSS has no goal, so a caller passing one asked for something that
/// cannot be honored and must be told — the same stance `tsv_wasm`'s
/// `read_options` takes when it rejects the `goal` key outright.
#[cfg(any(feature = "parse", feature = "format"))]
fn napi_goal(goal: Option<String>, allowed: bool) -> napi::Result<tsv_ts::Goal> {
    let Some(goal) = goal else {
        return Ok(tsv_ts::Goal::Module);
    };
    if !allowed {
        return Err(napi::Error::from_reason(
            "option 'goal' is only supported for TypeScript".to_string(),
        ));
    }
    tsv_ts::Goal::from_source_type(&goal).ok_or_else(|| {
        napi::Error::from_reason(format!(
            "invalid goal '{goal}' (expected 'script' or 'module')"
        ))
    })
}

// Per-language compound-op helpers: parse the source into a per-thread AST arena
// and run the conversion/format/no-op over it. Every language crate is
// interner-free (identifier and element/attribute names are span-identity), so
// these are uniform across svelte/typescript/css — no per-language arity split.
#[cfg(feature = "parse")]
macro_rules! parse_convert {
    ($goalness:ident, $lang:ident, $conv:ident, $source:expr, $goal:expr) => {
        with_ast_arena(|arena| {
            let ast = parse_ast!($goalness, $lang, $source, $goal, arena)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            Ok($lang::$conv(&ast, $source))
        })
    };
}

#[cfg(feature = "parse")]
macro_rules! parse_internal {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr) => {
        with_ast_arena(|arena| {
            let ast = parse_ast!($goalness, $lang, $source, $goal, arena)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            std::hint::black_box(&ast);
            Ok(())
        })
    };
}

#[cfg(feature = "format")]
macro_rules! parse_format {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr) => {{
        // The format path's line-terminator fold, ahead of the parse — see
        // `tsv_lang::printing::normalize_carriage_returns`. `parse_convert!` deliberately
        // skips it: the wire's offsets are a drop-in contract over the author's own bytes.
        let folded = tsv_lang::printing::normalize_carriage_returns($source);
        let source = folded.text();
        with_ast_arena(|arena| {
            let ast = parse_ast!($goalness, $lang, source, $goal, arena)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            Ok(with_doc_arena(|doc_arena| {
                $lang::format_folded_in(&ast, &folded, doc_arena)
            }))
        })
    }};
}

/// Generate `parse_<lang>` / `parse_<lang>_no_locations` /
/// `parse_internal_<lang>` / `format_<lang>` N-API functions for one language
/// module. The `js_name` literals keep the JS export names snake_case for parity
/// with `tsv_wasm` (napi-rs would otherwise camelCase them).
// One export per (language, operation), each taking the same `(source, goal?)`
// arguments. The `$goalness` axis decides only whether a goal ARGUMENT is
// accepted, never the arity: there is no goalless twin of a goal-aware export to
// drift from it, and the `@fuzdev/tsv` loader hands every export the same bag.
//
// At Script goal `await` is an ordinary identifier and `import`/`export`/
// `import.meta` are syntax errors. See `tsv parse --goal` and
// `tsv_ts::parse_with_goal`.
//
// `tsv_ffi` spells the same axis as a `u32` goal code and `tsv_wasm` as one key
// of a per-call options bag (`format_typescript(src, {goal})`); each binding's
// own `lang_bindings!` reads the SAME `parse_ast!` / `goal_allowed!` pair out of
// `tsv_arena`, so which languages have a goal axis is one fact in one place
// rather than three that agree today.
macro_rules! lang_bindings {
    (
        $goalness:ident,
        $lang:ident,
        $parse_fn:ident, $parse_js:literal,
        $parse_no_loc_fn:ident, $parse_no_loc_js:literal,
        $parse_internal_fn:ident, $parse_internal_js:literal,
        $format_fn:ident, $format_js:literal
    ) => {
        /// Parse source code and return its public JSON AST as a string.
        #[cfg(feature = "parse")]
        #[napi(js_name = $parse_js, catch_unwind)]
        pub fn $parse_fn(source: String, goal: Option<String>) -> napi::Result<String> {
            let goal = napi_goal(goal, goal_allowed!($goalness))?;
            parse_convert!($goalness, $lang, convert_ast_json_string, &source, goal)
        }

        /// Parse source and return its JSON AST string **without** per-node `loc`
        /// (the span-only `no-locations` wire). CSS is identical to `$parse_fn`.
        #[cfg(feature = "parse")]
        #[napi(js_name = $parse_no_loc_js, catch_unwind)]
        pub fn $parse_no_loc_fn(source: String, goal: Option<String>) -> napi::Result<String> {
            let goal = napi_goal(goal, goal_allowed!($goalness))?;
            parse_convert!(
                $goalness,
                $lang,
                convert_ast_json_string_no_locations,
                &source,
                goal
            )
        }

        /// Parse source to the internal AST only (no conversion, no
        /// serialization). Benchmark-only: `black_box` keeps the AST live so the
        /// parse can't be optimized away.
        #[cfg(feature = "parse")]
        #[napi(js_name = $parse_internal_js, catch_unwind)]
        pub fn $parse_internal_fn(source: String, goal: Option<String>) -> napi::Result<()> {
            let goal = napi_goal(goal, goal_allowed!($goalness))?;
            parse_internal!($goalness, $lang, &source, goal)
        }

        /// Format source code and return the formatted string. The goal shapes
        /// only the parse the formatter runs; formatting is non-configurable.
        #[cfg(feature = "format")]
        #[napi(js_name = $format_js, catch_unwind)]
        pub fn $format_fn(source: String, goal: Option<String>) -> napi::Result<String> {
            let goal = napi_goal(goal, goal_allowed!($goalness))?;
            parse_format!($goalness, $lang, &source, goal)
        }
    };
}

lang_bindings!(
    nogoal,
    tsv_svelte,
    parse_svelte,
    "parse_svelte",
    parse_svelte_no_locations,
    "parse_svelte_no_locations",
    parse_internal_svelte,
    "parse_internal_svelte",
    format_svelte,
    "format_svelte"
);
lang_bindings!(
    goal,
    tsv_ts,
    parse_typescript,
    "parse_typescript",
    parse_typescript_no_locations,
    "parse_typescript_no_locations",
    parse_internal_typescript,
    "parse_internal_typescript",
    format_typescript,
    "format_typescript"
);
lang_bindings!(
    nogoal,
    tsv_css,
    parse_css,
    "parse_css",
    parse_css_no_locations,
    "parse_css_no_locations",
    parse_internal_css,
    "parse_internal_css",
    format_css,
    "format_css"
);

//
// Discovery: the gitignore-aware matcher + its `tsv_discover` verdicts
//
// A hand-mirrored twin of `tsv_wasm`'s `IgnoreStack` — same method names, same
// argument order, same return shapes — so `npm/cli.js`, which imports its
// engine from `./index.js`, drives either package's copy unchanged. The point
// is agreement by construction with the native CLI: one matcher and one prune
// decision behind every surface. Format-only, like the wasm side; discovery
// exists to feed the formatter.
//
// ⚠️ `Option<String>` is deliberately spelled `Either<String, Undefined>`.
// napi-rs maps `None` to JS `null`, wasm-bindgen maps it to `undefined`, and a
// package that swaps for the other must not change which of the two a caller
// sees. `Undefined` is napi-rs's `()`, so the none arm allocates nothing.

/// The gitignore-aware matcher stack, mirroring `tsv_wasm`'s `IgnoreStack`.
///
/// Holds `.gitignore` and tsv (`.formatignore` / `.prettierignore`) layers
/// pushed shallowest-first, answers the per-path ignore status
/// (`is_ignored`), and delegates the discovery verdicts (`classify_dir`,
/// `should_format_file`, `is_path_pruned`) plus the shared warning strings to
/// `tsv_discover`.
#[cfg(feature = "format")]
#[napi]
pub struct IgnoreStack {
    inner: tsv_ignore::IgnoreStack,
}

#[cfg(feature = "format")]
#[napi]
impl IgnoreStack {
    /// An empty stack (ignores nothing until layers are added).
    #[napi(constructor, catch_unwind)]
    // `allow`, not `expect`: the lint fires without it, but through the `#[napi]` expansion
    // an expectation never registers as fulfilled.
    #[allow(clippy::new_without_default)] // napi exports the constructor
    pub fn new() -> IgnoreStack {
        IgnoreStack {
            inner: tsv_ignore::IgnoreStack::new(),
        }
    }

    /// Push one directory's `.gitignore`. `anchor` is the directory relative to
    /// the format root, `/`-separated (`""` = the root). Push shallowest-first.
    #[napi(js_name = "push_gitignore", catch_unwind)]
    pub fn push_gitignore(&mut self, anchor: String, content: String) {
        self.inner.push_gitignore(&anchor, &content);
    }

    /// Pop the most recently pushed `.gitignore` layer (a traversal unwinding
    /// out of a directory).
    #[napi(js_name = "pop_gitignore", catch_unwind)]
    pub fn pop_gitignore(&mut self) {
        self.inner.pop_gitignore();
    }

    /// Push one directory's tsv file, applied after every `.gitignore`. `anchor`
    /// is the directory relative to the format root (`""` = root). The caller
    /// resolves which file's content this is — `.formatignore` hierarchically, or
    /// a `.prettierignore` (also hierarchical) shadowed by a sibling `.formatignore`.
    #[napi(js_name = "push_tsv", catch_unwind)]
    pub fn push_tsv(&mut self, anchor: String, content: String) {
        self.inner.push_tsv(&anchor, &content);
    }

    /// Pop the most recently pushed tsv layer (a traversal unwinding out of a
    /// directory).
    #[napi(js_name = "pop_tsv", catch_unwind)]
    pub fn pop_tsv(&mut self) {
        self.inner.pop_tsv();
    }

    /// Whether `path` (relative to the format root, `/`-separated) is ignored;
    /// `is_dir` marks directories so trailing-`/` patterns apply.
    #[napi(js_name = "is_ignored", catch_unwind)]
    pub fn is_ignored(&self, path: String, is_dir: bool) -> bool {
        self.inner.is_ignored(&path, is_dir)
    }

    /// The discovery verdict for one child **directory**: `"descend"`,
    /// `"prune"`, or `"prune_warn"`. `name` is the directory's final path
    /// segment, `child_rel` its format-root-relative `/`-separated path, and
    /// `heuristic_active` is true while no `.gitignore` governs this level. On
    /// `"prune_warn"` the caller fetches the message via
    /// [`heuristic_shadow_warning`](IgnoreStack::heuristic_shadow_warning).
    ///
    /// A string tag rather than an enum or a struct — same as the wasm side,
    /// and it allocates no JS object on the common descend path.
    #[napi(js_name = "classify_dir", catch_unwind)]
    pub fn classify_dir(&self, name: String, child_rel: String, heuristic_active: bool) -> String {
        match tsv_discover::classify_dir(&name, &child_rel, heuristic_active, &self.inner) {
            tsv_discover::DirVerdict::Descend => "descend".to_string(),
            tsv_discover::DirVerdict::Prune => "prune".to_string(),
            tsv_discover::DirVerdict::PruneWithWarning(_) => "prune_warn".to_string(),
        }
    }

    /// Whether a child **file** should be formatted (a formattable extension and
    /// not ignored). `name` is the file's final path segment, `child_rel` its
    /// format-root-relative `/`-separated path.
    #[napi(js_name = "should_format_file", catch_unwind)]
    pub fn should_format_file(&self, name: String, child_rel: String) -> bool {
        tsv_discover::should_format_file(&name, &child_rel, &self.inner)
    }

    /// Whether `rel` (a format-root-relative file path) is skipped because some
    /// ancestor directory would be pruned by discovery — the safety nets, the
    /// build-output heuristic, or the matcher. A per-file companion to
    /// `classify_dir` for a consumer with no top-down traversal: it reconstructs
    /// each ancestor's `heuristic_active` from this stack's own pushed
    /// `.gitignore` anchors, so it takes no extra arguments. Pair with
    /// `is_ignored(rel, false)` for the file-level match.
    #[napi(js_name = "is_path_pruned", catch_unwind)]
    pub fn is_path_pruned(&self, rel: String) -> bool {
        tsv_discover::is_path_pruned(&rel, &self.inner)
    }

    /// The argument error for an explicitly named **file** whose extension tsv
    /// doesn't format; `undefined` when the extension is formattable. A method
    /// (not a free function) so it rides the class through the package facade;
    /// the receiver is unused — an argument check runs before any matcher
    /// exists. Single source of truth with the native CLI, including the
    /// rendered extension list, so `npm/cli.js` never hand-mirrors
    /// `FORMATTABLE_EXTENSIONS`.
    #[napi(js_name = "unsupported_extension_error", catch_unwind)]
    pub fn unsupported_extension_error(&self, path: String) -> Either<String, Undefined> {
        or_undefined(tsv_discover::unsupported_extension_error(&path))
    }

    /// The heuristic-shadow warning text for a pruned directory `dir`
    /// (format-root relative). A method (not a free function) so it rides the
    /// class through the package facade; the receiver is unused. Single source
    /// of truth with the native CLI — the JS CLI never templates this string.
    #[napi(js_name = "heuristic_shadow_warning", catch_unwind)]
    pub fn heuristic_shadow_warning(&self, dir: String) -> String {
        tsv_discover::heuristic_shadow_warning(&dir)
    }

    /// The `.prettierignore`-outside-a-repo warning for the target root `dir`
    /// (its display path); `undefined` unless, outside a git repo, a
    /// target-root `.prettierignore` is present and unshadowed by a sibling
    /// `.formatignore`. The JS CLI calls this once at the target root and pushes
    /// any returned string into its warnings channel — single source of truth
    /// with the native CLI, never templated in JS.
    #[napi(js_name = "prettierignore_outside_repo_warning", catch_unwind)]
    pub fn prettierignore_outside_repo_warning(
        &self,
        dir: String,
        in_repo: bool,
        has_prettierignore: bool,
        has_formatignore: bool,
    ) -> Either<String, Undefined> {
        or_undefined(tsv_discover::prettierignore_outside_repo_warning(
            &dir,
            in_repo,
            has_prettierignore,
            has_formatignore,
        ))
    }

    /// The heads-up when, inside a git repo, a directory holds both a
    /// `.formatignore` and a `.prettierignore` — the sibling `.formatignore`
    /// shadows the `.prettierignore`, so its rules go unread there.
    /// `undefined` unless both files are present inside a repo.
    #[napi(js_name = "prettierignore_shadowed_warning", catch_unwind)]
    pub fn prettierignore_shadowed_warning(
        &self,
        dir: String,
        in_repo: bool,
        has_prettierignore: bool,
        has_formatignore: bool,
    ) -> Either<String, Undefined> {
        or_undefined(tsv_discover::prettierignore_shadowed_warning(
            &dir,
            in_repo,
            has_prettierignore,
            has_formatignore,
        ))
    }

    /// Whether no layer carries any rule — callers skip per-path matching.
    #[napi(js_name = "is_empty", catch_unwind)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// `Option<String>` as JS `string | undefined` rather than napi-rs's default
/// `string | null` — the wasm package's shape, which this one must match.
#[cfg(feature = "format")]
fn or_undefined(value: Option<String>) -> Either<String, Undefined> {
    match value {
        Some(s) => Either::A(s),
        None => Either::B(()),
    }
}

/// Deliberately panic inside the binding — the panic-contract probe.
///
/// Compiled only under the test-only `panic_probe` feature (`deno task
/// test:napi` builds with it; published artifacts never carry it).
/// `scripts/test_napi.ts` drives it to prove the contract end to end: the panic
/// unwinds into `catch_unwind`, surfaces as a thrown JS error, and the process
/// plus the per-thread arenas stay usable afterwards. It panics *inside*
/// `with_ast_arena` so the take/park recovery is exercised with an arena
/// genuinely in flight.
#[cfg(feature = "panic_probe")]
#[napi(js_name = "__panic_probe", catch_unwind)]
// `allow`, not `expect`: same `#[napi]`-expansion artifact as `IgnoreStack::new` above.
#[allow(clippy::panic)] // panicking is the export's entire purpose
pub fn panic_probe() {
    with_ast_arena(|arena| {
        let _ = arena.alloc_str("doomed");
        panic!("tsv_napi panic probe");
    });
}

// Drive every entry point in-process so `cargo test` exercises the native
// binding without a Node host (the Deno/WASM smoke paths don't cover napi).
// These call the plain Rust functions the `#[napi]` macro wraps; the JS
// marshalling layer is what `scripts/test_napi.ts` covers under Node.
#[cfg(test)]
mod tests {
    use super::*;

    /// Signature shared by every `parse_<lang>` / `format_<lang>` entry point.
    type StringFn = fn(String, Option<String>) -> napi::Result<String>;
    /// Signature shared by every `parse_internal_<lang>` entry point.
    type UnitFn = fn(String, Option<String>) -> napi::Result<()>;

    /// Call at the default (Module) goal, i.e. with the goal argument omitted —
    /// the shape every non-TypeScript caller uses.
    fn at_default(f: StringFn, source: &str) -> napi::Result<String> {
        f(source.to_owned(), None)
    }

    /// Call at an explicit goal.
    fn at_goal(f: StringFn, source: &str, goal: &str) -> napi::Result<String> {
        f(source.to_owned(), Some(goal.to_owned()))
    }

    // --- format: normalizes, every language (exact output) ---

    #[test]
    fn format_normalizes_per_language() {
        // Annotate the array type so the fn items coerce to `StringFn` (no casts).
        let cases: [(&str, StringFn, &str, &str); 3] = [
            (
                "typescript",
                format_typescript,
                "const   x=1",
                "const x = 1;\n",
            ),
            ("css", format_css, "a{color:red}", "a {\n\tcolor: red;\n}\n"),
            (
                "svelte",
                format_svelte,
                "<div   >x</div   >",
                "<div>x</div>\n",
            ),
        ];
        for (label, f, input, expected) in cases {
            assert_eq!(at_default(f, input).unwrap(), expected, "{label} format");
        }
    }

    // --- parse: returns the language's own JSON root type ---

    #[test]
    fn parse_returns_language_root_type() {
        // The root `type` is distinct per language, so asserting it also guards
        // the `lang_bindings!` wiring: a transposed invocation (e.g. `parse_css`
        // pointed at `tsv_ts`) would return the wrong root, not just "some JSON."
        let cases: [(&str, StringFn, &str, &str); 3] = [
            ("typescript", parse_typescript, "const x = 1;", "Program"),
            ("css", parse_css, "a { color: red }", "StyleSheetFile"),
            ("svelte", parse_svelte, "<div>x</div>", "Root"),
        ];
        for (label, f, src, root_type) in cases {
            let json = at_default(f, src).unwrap();
            let value: serde_json::Value =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{label}: not JSON: {e}"));
            assert_eq!(
                value.get("type").and_then(serde_json::Value::as_str),
                Some(root_type),
                "{label}: unexpected root type in {json}"
            );
        }
    }

    // --- the goal axis: script accepts `await` as identifier, module rejects ---

    #[test]
    fn typescript_goal_switches_await() {
        // `await` is an ordinary identifier at Script goal, reserved at Module goal.
        let src = "var await = 1;\n";
        // Annotate the array type so the fn items coerce to `StringFn` (no casts).
        let parsers: [StringFn; 2] = [parse_typescript, parse_typescript_no_locations];
        for f in parsers {
            assert!(at_goal(f, src, "script").is_ok());
            assert!(at_goal(f, src, "module").is_err());
            // An omitted goal is the Module default, not a third behavior.
            assert!(at_default(f, src).is_err());
        }
        assert!(parse_internal_typescript(src.to_owned(), Some("script".to_owned())).is_ok());
        assert!(parse_internal_typescript(src.to_owned(), Some("module".to_owned())).is_err());
        assert!(parse_internal_typescript(src.to_owned(), None).is_err());
        // The format twin: the goal shapes the parse the formatter runs.
        assert_eq!(
            at_goal(format_typescript, "var   await=1", "script").unwrap(),
            "var await = 1;\n",
            "script-goal format"
        );
        assert!(at_goal(format_typescript, src, "module").is_err());
        // An invalid goal string is a thrown error, not a silent module fallback.
        assert!(at_goal(parse_typescript, src, "sloppy").is_err());
        assert!(at_goal(format_typescript, src, "sloppy").is_err());
    }

    #[test]
    fn goalless_languages_reject_a_goal_argument() {
        // Svelte hard-wires Module and CSS has no goal, so a goal argument asks
        // for something that cannot be honored — the caller is told rather than
        // silently served a Module parse. The same stance `tsv_wasm`'s
        // `read_options` takes when it rejects the key outright.
        //
        // Every string-returning export, not one per language: each is a
        // separately generated entry point that calls `napi_goal` on its own
        // line, so a refusal can be lost on exactly one of them.
        // `parse_internal_*` returns `()` and is driven separately below.
        let cases: [(&str, StringFn, &str); 6] = [
            ("svelte parse", parse_svelte, "<div>x</div>"),
            (
                "svelte parse_no_locations",
                parse_svelte_no_locations,
                "<div>x</div>",
            ),
            ("svelte format", format_svelte, "<div>x</div>"),
            ("css parse", parse_css, "a { color: red }"),
            (
                "css parse_no_locations",
                parse_css_no_locations,
                "a { color: red }",
            ),
            ("css format", format_css, "a { color: red }"),
        ];
        for (label, f, src) in cases {
            // Even `"module"` — the value they would have used — is refused: the
            // rejection is of the AXIS, so a caller cannot read agreement into it.
            for goal in ["script", "module"] {
                let err = at_goal(f, src, goal).unwrap_err();
                assert!(
                    err.reason.contains("only supported for TypeScript"),
                    "{label} at {goal}: {}",
                    err.reason
                );
            }
            at_default(f, src).unwrap_or_else(|e| panic!("{label}: {}", e.reason));
        }

        let internal: [(&str, UnitFn, &str); 2] = [
            (
                "svelte parse_internal",
                parse_internal_svelte,
                "<div>x</div>",
            ),
            ("css parse_internal", parse_internal_css, "a { color: red }"),
        ];
        for (label, f, src) in internal {
            for goal in ["script", "module"] {
                let err = f(src.to_owned(), Some(goal.to_owned())).unwrap_err();
                assert!(
                    err.reason.contains("only supported for TypeScript"),
                    "{label} at {goal}: {}",
                    err.reason
                );
            }
            f(src.to_owned(), None).unwrap_or_else(|e| panic!("{label}: {}", e.reason));
        }
    }

    // --- parse_internal: parses without converting (Ok, no JSON), every language ---

    #[test]
    fn parse_internal_ok_per_language() {
        let cases: [(&str, UnitFn, &str); 3] = [
            ("typescript", parse_internal_typescript, "const x = 1;"),
            ("css", parse_internal_css, "a { color: red }"),
            ("svelte", parse_internal_svelte, "<div>x</div>"),
        ];
        for (label, f, src) in cases {
            f(src.to_owned(), None).unwrap_or_else(|e| panic!("{label}: {}", e.reason));
        }
    }

    // --- errors surface as a thrown napi::Error carrying the engine message ---

    #[test]
    fn invalid_syntax_errors_per_language() {
        // Both parse and format wrap the engine error into a napi::Error (which
        // napi-rs throws — there is no `{"error": …}` envelope, unlike FFI).
        // Cover the error arm for every language across both entry-point kinds.
        let cases: [(&str, StringFn, StringFn, &str); 3] = [
            (
                "typescript",
                parse_typescript,
                format_typescript,
                "const = ;",
            ),
            ("css", parse_css, format_css, "a {"),
            ("svelte", parse_svelte, format_svelte, "<div {"),
        ];
        for (label, parse_fn, format_fn, src) in cases {
            let parse_err = at_default(parse_fn, src).unwrap_err();
            assert!(
                !parse_err.reason.is_empty(),
                "{label} parse: error must carry a reason"
            );
            let format_err = at_default(format_fn, src).unwrap_err();
            assert!(
                !format_err.reason.is_empty(),
                "{label} format: error must carry a reason"
            );
        }
    }

    // --- the per-thread arenas are reset+reused across calls (warm-path soundness) ---

    #[test]
    fn repeated_calls_reuse_arenas() {
        // This crate's distinctive risk: `with_ast_arena` / `with_doc_arena`
        // keep one arena per thread and `reset()` it at the start of each call,
        // so nothing built in a prior call may leak past the next reset. Two
        // back-to-back formats on a warm arena must produce identical output,
        // and interleaving a parse (which drives the AST arena on its own)
        // between them must not perturb the format result.
        let once = at_default(format_typescript, "const   x=1").unwrap();
        let twice = at_default(format_typescript, "const   x=1").unwrap();
        assert_eq!(once, twice, "second format on a warm arena diverged");
        at_default(parse_typescript, "const y = 2;").unwrap();
        let after_parse = at_default(format_typescript, "const   x=1").unwrap();
        assert_eq!(once, after_parse, "interleaved parse perturbed format");
    }

    // --- multibyte source survives the JS-string marshalling + char-offset boundary ---

    #[test]
    fn parse_and_format_preserve_multibyte_source() {
        // napi-rs marshals JS strings in/out and the AST carries char offsets;
        // this is the same boundary risk tsv_ffi's same-named test guards.
        let src = "const x = '€🦀';\n";
        let json = at_default(parse_typescript, src).unwrap();
        assert!(json.contains("\"type\""), "parse produced no AST: {json}");
        let formatted = at_default(format_typescript, src).unwrap();
        assert!(
            formatted.contains("€🦀"),
            "multibyte content lost: {formatted}"
        );
        // Re-formatting is stable (idempotent) across the boundary.
        assert_eq!(
            at_default(format_typescript, &formatted).unwrap(),
            formatted,
            "re-format not idempotent across the boundary"
        );
    }

    // --- discovery: the IgnoreStack wrapper delegates, and `None` is undefined ---

    #[test]
    #[cfg(feature = "format")]
    fn ignore_stack_layers_and_verdicts() {
        let mut stack = IgnoreStack::new();
        assert!(stack.is_empty());
        stack.push_gitignore(String::new(), "dist/\n".to_owned());
        assert!(!stack.is_empty());
        assert!(stack.is_ignored("dist".to_owned(), true));
        assert!(!stack.is_ignored("src/a.ts".to_owned(), false));
        assert_eq!(
            stack.classify_dir("node_modules".to_owned(), "node_modules".to_owned(), false),
            "prune"
        );
        assert_eq!(
            stack.classify_dir("src".to_owned(), "src".to_owned(), false),
            "descend"
        );
        assert!(stack.should_format_file("a.ts".to_owned(), "src/a.ts".to_owned()));
        assert!(!stack.should_format_file("a.txt".to_owned(), "src/a.txt".to_owned()));
        assert!(stack.is_path_pruned("node_modules/x.ts".to_owned()));
        stack.pop_gitignore();
        assert!(stack.is_empty());
    }

    /// The maybe-a-warning methods must yield JS `undefined`, not `null`, for
    /// the none arm — wasm-bindgen's shape, which this package promises. The
    /// `Either::B` variant is what encodes that, so this pins the variant
    /// rather than the JS value (which only `test_napi_npm.ts` can observe).
    #[test]
    #[cfg(feature = "format")]
    fn absent_warnings_are_the_undefined_variant() {
        let stack = IgnoreStack::new();
        assert!(matches!(
            stack.unsupported_extension_error("a.ts".to_owned()),
            Either::B(())
        ));
        assert!(matches!(
            stack.unsupported_extension_error("a.txt".to_owned()),
            Either::A(_)
        ));
        assert!(matches!(
            stack.prettierignore_shadowed_warning("d".to_owned(), true, false, false),
            Either::B(())
        ));
        assert!(matches!(
            stack.prettierignore_outside_repo_warning("d".to_owned(), true, true, false),
            Either::B(())
        ));
    }
}
