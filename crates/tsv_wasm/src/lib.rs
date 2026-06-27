//! WebAssembly bindings for tsv.
//!
//! Three builds from two features:
//! - default (`@fuzdev/tsv_wasm`): everything — `format_*` plus the parse exports.
//! - `--no-default-features --features format` (`@fuzdev/tsv_format_wasm`):
//!   `format_*` exports only.
//! - `--no-default-features --features parse` (`@fuzdev/tsv_parse_wasm`):
//!   `parse_*`, `parse_*_json`, and `parse_internal_*` plus the convert layer
//!   that serializes ASTs to JS; the printers drop out at link time.
//!
//! The AST crosses the JS boundary as a single JSON string: `parse_*` calls
//! the engine's native `JSON.parse` on it (via `js_sys`) and returns the
//! typed object; `parse_*_json` returns the string itself for consumers that
//! forward the wire format without materializing. Building the JS object
//! graph node-by-node with `serde_wasm_bindgen` is measurably slower.

use wasm_bindgen::prelude::*;

// Per-thread reusable AST/doc arenas, shared with the native bindings via the
// `tsv_arena` crate. WASM is single-threaded, so the thread-local is effectively
// a module static: the arena's high-water chunk is retained across calls and
// `reset()` rewinds it, removing the per-call `Bump` / `DocArena` allocation (the
// documented WASM-format allocation-count lever). Soundness matches the native
// bindings — the AST/doc are fully consumed into an owned return value before the
// next call's `reset()`.
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;

fn err(e: impl ToString) -> JsError {
    JsError::new(&e.to_string())
}

/// Hierarchical, git-faithful matcher for tsv's discovery ignore files,
/// wrapping `tsv_ignore::IgnoreStack`. Built up by the caller from a repo's
/// `.gitignore` files plus the repo-root tsv file
/// (`.formatignore`/`.prettierignore`), then queried per path — both for the raw
/// ignore status (`is_ignored`) and for the shared `tsv_discover` discovery
/// verdict (`classify_dir`/`should_format_file`). Exposed so the JS CLI
/// (`npm/cli.js`) and the VS Code extension share the exact same matcher **and**
/// prune decision as the native CLI — agreement by construction. Built only into
/// the `format`-capable packages (`@fuzdev/tsv_format_wasm`, `@fuzdev/tsv_wasm`);
/// the parse-only package omits it.
#[cfg(feature = "format")]
#[wasm_bindgen]
pub struct IgnoreStack {
    inner: tsv_ignore::IgnoreStack,
}

#[cfg(feature = "format")]
#[wasm_bindgen]
impl IgnoreStack {
    /// An empty stack (ignores nothing until layers are added).
    #[wasm_bindgen(constructor)]
    #[allow(clippy::new_without_default)] // wasm-bindgen exports the constructor
    pub fn new() -> IgnoreStack {
        IgnoreStack {
            inner: tsv_ignore::IgnoreStack::new(),
        }
    }

    /// Push one directory's `.gitignore`. `anchor` is the directory relative to
    /// the format root, `/`-separated (`""` = the root). Push shallowest-first.
    pub fn push_gitignore(&mut self, anchor: &str, content: &str) {
        self.inner.push_gitignore(anchor, content);
    }

    /// Pop the most recently pushed `.gitignore` layer (a traversal unwinding
    /// out of a directory).
    pub fn pop_gitignore(&mut self) {
        self.inner.pop_gitignore();
    }

    /// Push one directory's tsv file, applied after every `.gitignore`. `anchor`
    /// is the directory relative to the format root (`""` = root). The caller
    /// resolves which file's content this is — `.formatignore` hierarchically, or
    /// a repo-root `.prettierignore` shadowed by a repo-root `.formatignore`.
    pub fn push_tsv(&mut self, anchor: &str, content: &str) {
        self.inner.push_tsv(anchor, content);
    }

    /// Pop the most recently pushed tsv layer (a traversal unwinding out of a
    /// directory).
    pub fn pop_tsv(&mut self) {
        self.inner.pop_tsv();
    }

    /// Whether `path` (relative to the format root, `/`-separated) is ignored;
    /// `is_dir` marks directories so trailing-`/` patterns apply.
    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        self.inner.is_ignored(path, is_dir)
    }

    /// The discovery verdict for one child **directory**, delegating to
    /// `tsv_discover::classify_dir` — the safety-net / build-output-heuristic /
    /// matcher decision shared with the native CLI. Returns `"descend"`,
    /// `"prune"`, or `"prune_warn"`. `name` is the directory's final path
    /// segment, `child_rel` its format-root-relative `/`-separated path, and
    /// `heuristic_active` is true while no `.gitignore` governs this level. On
    /// `"prune_warn"` the caller fetches the message via
    /// [`heuristic_shadow_warning`](IgnoreStack::heuristic_shadow_warning).
    ///
    /// A string tag (rather than a wasm-bindgen enum or a returned struct) keeps
    /// the package facade / `patch_npm_package.ts` unchanged and allocates no JS
    /// object on the common descend path.
    pub fn classify_dir(&self, name: &str, child_rel: &str, heuristic_active: bool) -> String {
        match tsv_discover::classify_dir(name, child_rel, heuristic_active, &self.inner) {
            tsv_discover::DirVerdict::Descend => "descend".to_string(),
            tsv_discover::DirVerdict::Prune => "prune".to_string(),
            tsv_discover::DirVerdict::PruneWithWarning(_) => "prune_warn".to_string(),
        }
    }

    /// Whether a child **file** should be formatted (a formattable extension and
    /// not ignored), delegating to `tsv_discover::should_format_file`. `name` is
    /// the file's final path segment, `child_rel` its format-root-relative
    /// `/`-separated path.
    pub fn should_format_file(&self, name: &str, child_rel: &str) -> bool {
        tsv_discover::should_format_file(name, child_rel, &self.inner)
    }

    /// Whether `rel` (a format-root-relative file path) is skipped because some
    /// ancestor directory would be pruned by discovery — the safety nets, the
    /// build-output heuristic, or the matcher — delegating to
    /// `tsv_discover::is_path_pruned`. A per-file companion to `classify_dir` for a
    /// consumer with no top-down traversal: it reconstructs each ancestor's
    /// `heuristic_active` from this stack's own pushed `.gitignore` anchors, so it
    /// takes no extra arguments. Pair with `is_ignored(rel, false)` for the
    /// file-level match.
    pub fn is_path_pruned(&self, rel: &str) -> bool {
        tsv_discover::is_path_pruned(rel, &self.inner)
    }

    /// The heuristic-shadow warning text for a pruned directory `dir`
    /// (format-root relative), delegating to `tsv_discover::heuristic_shadow_warning`.
    /// A method (not a free function) so it rides the `IgnoreStack` class
    /// re-export through the package facade; the receiver is unused. Single source
    /// of truth with the native CLI — the JS CLI never templates this string.
    pub fn heuristic_shadow_warning(&self, dir: &str) -> String {
        tsv_discover::heuristic_shadow_warning(dir)
    }

    /// The `.prettierignore`-outside-a-repo warning text for the target root `dir`
    /// (its display path), delegating to
    /// `tsv_discover::prettierignore_outside_repo_warning`. Returns `undefined`
    /// (the JS view of `None`) unless, outside a git repo, a target-root
    /// `.prettierignore` is present and unshadowed by a sibling `.formatignore`.
    /// A method (not a free function) so it rides the `IgnoreStack` class
    /// re-export through the package facade; the receiver is unused. The JS CLI
    /// calls this once at the target root and pushes any returned string into its
    /// warnings channel — single source of truth with the native CLI, never
    /// templated in JS.
    pub fn prettierignore_outside_repo_warning(
        &self,
        dir: &str,
        in_repo: bool,
        has_prettierignore: bool,
        has_formatignore: bool,
    ) -> Option<String> {
        tsv_discover::prettierignore_outside_repo_warning(
            dir,
            in_repo,
            has_prettierignore,
            has_formatignore,
        )
    }

    /// Whether no layer carries any rule — callers skip per-path matching.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Re-export every type from the bundled `./tsv_ast` declaration file
/// so consumers of `@fuzdev/tsv_parse_wasm` can `import type { Program } from
/// '@fuzdev/tsv_parse_wasm'` without reaching into the bundled `.d.ts`.
#[cfg(feature = "parse")]
#[wasm_bindgen(typescript_custom_section)]
const TS_AST_REEXPORT: &'static str = r#"
export type * from "./tsv_ast";
"#;

/// Typed return types for `parse_*` exports. Each extern type points at
/// the matching interface in the bundled `tsv_ast.d.ts`, so the
/// wasm-pack-generated `tsv_wasm.d.ts` declares `parse_typescript` as
/// returning `Program`, etc.
#[cfg(feature = "parse")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "import('./tsv_ast').Program")]
    pub type TsProgram;

    #[wasm_bindgen(typescript_type = "import('./tsv_ast').StyleSheetFile")]
    pub type CssStyleSheet;

    #[wasm_bindgen(typescript_type = "import('./tsv_ast').Root")]
    pub type SvelteRoot;
}

/// Generate `parse_<lang>` / `parse_<lang>_json` / `parse_internal_<lang>` /
/// `format_<lang>` WASM functions for one language module. `parse_*`,
/// `parse_*_json`, and `parse_internal_*` are gated on `parse` (so the
/// format-only build excludes the convert layer) and `format_*` on `format`
/// (so the parse-only build drops the printers at link time). `$parse_ret`
/// is the extern type from the block above whose `typescript_type` attribute
/// names the matching interface in `tsv_ast.d.ts`.
macro_rules! lang_bindings {
    (
        $parse_fn:ident,
        $parse_json_fn:ident,
        $parse_internal_fn:ident,
        $format_fn:ident,
        $lang:ident,
        $parse_ret:ident $(,)?
    ) => {
        /// Parse source into the typed JSON AST.
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_fn(source: &str) -> Result<$parse_ret, JsError> {
            let json = $parse_json_fn(source)?;
            let js_value = js_sys::JSON::parse(&json)
                .map_err(|_| err("internal error: AST serialized to invalid JSON"))?;
            Ok(js_value.unchecked_into::<$parse_ret>())
        }

        /// Parse source into the JSON AST as a compact JSON string, skipping
        /// JS object materialization (for consumers forwarding the wire format).
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_json_fn(source: &str) -> Result<String, JsError> {
            with_ast_arena(|arena| {
                let ast = $lang::parse(source, arena).map_err(err)?;
                Ok($lang::convert_ast_json_string(&ast, source))
            })
        }

        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_internal_fn(source: &str) -> Result<(), JsError> {
            with_ast_arena(|arena| {
                let ast = $lang::parse(source, arena).map_err(err)?;
                std::hint::black_box(ast);
                Ok(())
            })
        }

        #[cfg(feature = "format")]
        #[wasm_bindgen]
        pub fn $format_fn(source: &str) -> Result<String, JsError> {
            with_ast_arena(|arena| {
                let ast = $lang::parse(source, arena).map_err(err)?;
                Ok(with_doc_arena(|doc_arena| {
                    $lang::format_in(&ast, source, doc_arena)
                }))
            })
        }
    };
}

lang_bindings!(
    parse_svelte,
    parse_svelte_json,
    parse_internal_svelte,
    format_svelte,
    tsv_svelte,
    SvelteRoot,
);
lang_bindings!(
    parse_typescript,
    parse_typescript_json,
    parse_internal_typescript,
    format_typescript,
    tsv_ts,
    TsProgram,
);
lang_bindings!(
    parse_css,
    parse_css_json,
    parse_internal_css,
    format_css,
    tsv_css,
    CssStyleSheet,
);

// --- TypeScript goal-aware exports ---
//
// The parse goal (`Script` vs `Module`) is a TypeScript-only axis — Svelte
// `<script>` is always a module and CSS has no goal — so these sit outside the
// uniform `lang_bindings!` macro rather than threading a meaningless `goal`
// through svelte/css. They mirror the `tsv_ts` arms of the macro
// (`parse_typescript_json` / `format_typescript`) with an explicit goal; the
// goalless exports remain the `Module` default. See `tsv parse|format --goal`.

/// Parse a goal string (`"script"` / `"module"`) for the goal-aware exports,
/// mirroring `tsv_cli`'s `parse_goal_arg`.
#[cfg(any(feature = "parse", feature = "format"))]
fn goal_from_str(goal: &str) -> Result<tsv_ts::Goal, JsError> {
    tsv_ts::Goal::from_source_type(goal).ok_or_else(|| {
        err(format!(
            "invalid goal '{goal}' (expected 'script' or 'module')"
        ))
    })
}

/// `parse_typescript_json` against an explicit goal (`"script"` / `"module"`):
/// at `script`, `await` is an ordinary identifier and `import`/`export`/
/// `import.meta` are syntax errors. Returns the compact JSON-string wire form.
#[cfg(feature = "parse")]
#[wasm_bindgen]
pub fn parse_typescript_json_with_goal(source: &str, goal: &str) -> Result<String, JsError> {
    let goal = goal_from_str(goal)?;
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    let ast = tsv_ts::parse_with_goal(source, goal, &arena).map_err(err)?;
    Ok(tsv_ts::convert_ast_json_string(&ast, source))
}

/// `format_typescript` against an explicit goal (`"script"` / `"module"`).
#[cfg(feature = "format")]
#[wasm_bindgen]
pub fn format_typescript_with_goal(source: &str, goal: &str) -> Result<String, JsError> {
    let goal = goal_from_str(goal)?;
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    let ast = tsv_ts::parse_with_goal(source, goal, &arena).map_err(err)?;
    Ok(tsv_ts::format(&ast, source))
}
