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
// next call's `reset()`, and both helpers park their arena outside the
// thread-local while it is in use, so a trap here leaves a callable instance
// (see `tsv_arena`'s §Abort safety — this is the target that made it necessary).
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;

// WASM global allocator: talc replaces std's default dlmalloc on wasm32. The
// format path is allocation-heavy (doc IR, output string, memo vecs) and
// dlmalloc's grow/memcpy behavior is the measured allocation wall there; talc
// is a pure-Rust no_std allocator tuned for WebAssembly. The `WasmGrowAndExtend`
// source (vs the default `WasmGrowAndClaim`) extends one contiguous heap on
// `memory.grow` instead of claiming fragmented new ones — it holds the
// long-lived reset()-reuse instance's linear-memory high-water at dlmalloc
// parity, where the claim-source fragments it. Single-threaded WASM only
// (`TalcSyncCell::new_wasm` panics under atomics, which tsv never builds
// with); native builds keep the system allocator via the target gate.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: talc::cell::TalcSyncCell<talc::wasm::WasmGrowAndExtend, talc::wasm::WasmBinning> =
    talc::cell::TalcSyncCell::new_wasm(talc::wasm::WasmGrowAndExtend::new());

// Panic reporting. The shipped profile is `panic = "abort"`, so a panic
// compiles to a WASM trap: the host sees a bare `RuntimeError: unreachable`
// with no message, no location, and nothing to report upstream — and with
// `strip = true` there is no symbol to recover it from either. `std` still runs
// the panic hook before aborting, which is the one place the message is still
// in hand, so the hook forwards it to `console.error`. Purely diagnostic: the
// call still traps, and the instance stays callable afterwards because the
// arena helpers park (see the `tsv_arena` note above).
//
// `console.error` is declared directly rather than pulled from `web_sys` /
// `console_error_panic_hook`: the binding is three lines, and `js-sys` itself
// rides only the `parse` feature — a dependency here would newly weigh down the
// format-only package.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Forward panic messages to `console.error` before the trap swallows them.
///
/// wasm-bindgen runs this once at module init.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| console_error(&info.to_string())));
}

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
    /// a `.prettierignore` (also hierarchical) shadowed by a sibling `.formatignore`.
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

    /// The heads-up when, inside a git repo, a directory holds both a
    /// `.formatignore` and a `.prettierignore` — the sibling `.formatignore`
    /// shadows the `.prettierignore`, so its rules go unread there. Thin wrapper
    /// over `tsv_discover::prettierignore_shadowed_warning`. Returns `undefined`
    /// (the JS view of `None`) unless both files are present inside a repo. A
    /// method (not a free function) so it rides the `IgnoreStack` class re-export;
    /// the receiver is unused. The JS CLI calls this per directory and pushes any
    /// returned string into its warnings channel — single source of truth with the
    /// native CLI, never templated in JS.
    pub fn prettierignore_shadowed_warning(
        &self,
        dir: &str,
        in_repo: bool,
        has_prettierignore: bool,
        has_formatignore: bool,
    ) -> Option<String> {
        tsv_discover::prettierignore_shadowed_warning(
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

/// Hand-written declarations for the parse exports, which are all
/// `#[wasm_bindgen(skip_typescript)]`: wasm-bindgen can't express an
/// options-dependent return type, and `{locations: false}` deliberately
/// returns a shape `tsv_ast.d.ts` can't name (its interfaces declare `loc`
/// required), so that overload returns `any` and must come first (the more
/// specific signature). A signature change in `lang_bindings!` must update
/// this block too; `ParseOptions` / `TypeScriptParseOptions` are re-exported
/// through the npm facade (`scripts/patch_npm_package.ts`).
#[cfg(feature = "parse")]
#[wasm_bindgen(typescript_custom_section)]
const TS_PARSE_DECLS: &'static str = r#"
/** Options accepted by every parse export. */
export interface ParseOptions {
	/**
	 * Emit per-node `loc` (line/column) — the drop-in acorn/svelte wire.
	 * `false` emits the span-only wire (~46% smaller; Svelte also omits
	 * `name_loc`): `loc` stays derivable from `start`/`end` plus the source —
	 * see `reconstruct_locations` / `create_locator`. Inert for CSS (its wire
	 * has no `loc`) and for `parse_internal_*` (no wire at all).
	 * @default true
	 */
	locations?: boolean;
}

/** `ParseOptions` plus the TypeScript-only parse goal. */
export interface TypeScriptParseOptions extends ParseOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module';
}

export function parse_svelte(source: string, options: ParseOptions & { locations: false }): any;
export function parse_svelte(source: string, options?: ParseOptions): import('./tsv_ast').Root;
export function parse_svelte_json(source: string, options?: ParseOptions): string;
export function parse_internal_svelte(source: string, options?: ParseOptions): void;

export function parse_typescript(
	source: string,
	options: TypeScriptParseOptions & { locations: false }
): any;
export function parse_typescript(
	source: string,
	options?: TypeScriptParseOptions
): import('./tsv_ast').Program;
export function parse_typescript_json(source: string, options?: TypeScriptParseOptions): string;
export function parse_internal_typescript(
	source: string,
	options?: TypeScriptParseOptions
): void;

export function parse_css(source: string, options?: ParseOptions): import('./tsv_ast').StyleSheetFile;
export function parse_css_json(source: string, options?: ParseOptions): string;
export function parse_internal_css(source: string, options?: ParseOptions): void;
"#;

/// The parsed options bag every parse export accepts: `{locations?, goal?}`.
///
/// `locations` (default `true`) selects the wire: the loc-bearing drop-in
/// contract, or the span-only variant (the language crates'
/// `convert_ast_json_string_no_locations`). It is accepted by every parse
/// export and inert where nothing reads it (CSS emits no `loc`;
/// `parse_internal_*` emits no wire). `goal` (default `module`) is
/// TypeScript-only — Svelte hard-wires `Module` and CSS has no goal — so the
/// other languages reject the key rather than silently ignoring a semantic
/// axis. Unknown keys are an error: a typo like `{locatons: false}` silently
/// succeeding would hand back the full wire while the caller believes they
/// opted out.
#[cfg(feature = "parse")]
struct ParseOptions {
    locations: bool,
    goal: tsv_ts::Goal,
}

/// Read a `ParseOptions` off the raw `options` argument (`undefined`/`null`
/// mean all-defaults; a supported key explicitly set to `undefined` means that
/// key's default, matching the omitted-key JS convention — unknown keys error
/// whatever their value).
#[cfg(feature = "parse")]
fn parse_parse_options(options: &JsValue, allow_goal: bool) -> Result<ParseOptions, JsError> {
    let mut parsed = ParseOptions {
        locations: true,
        goal: tsv_ts::Goal::Module,
    };
    if options.is_undefined() || options.is_null() {
        return Ok(parsed);
    }
    // An array is `typeof 'object'` and yields no keys, so without the second
    // test a positional-style `parse_typescript(src, [goal])` would read as
    // all-defaults — the same silent-opt-out the unknown-key error exists to
    // prevent. (A keyless non-plain object, e.g. `new Date()`, still defaults;
    // ruling that out needs a prototype test this doesn't earn.)
    if !options.is_object() || js_sys::Array::is_array(options) {
        return Err(err("parse options must be an object"));
    }
    let object: &js_sys::Object = options.unchecked_ref();
    for key in js_sys::Object::keys(object).iter() {
        // `Object.keys` yields only string keys.
        let Some(name) = key.as_string() else {
            return Err(err("parse option keys must be strings"));
        };
        let value = js_sys::Reflect::get(options, &key)
            .map_err(|_| err(format!("failed to read parse option '{name}'")))?;
        // A supported key explicitly set to `undefined` means that key's default
        // (the omitted-key JS convention) — decided per arm, AFTER the key match,
        // so an unknown key errors whatever its value (`{locatons: undefined}` is
        // the same typo as `{locatons: false}`). `goal`'s check runs before its
        // language rejection: that's what lets one bag serve whichever parser
        // with the inapplicable goal spelled `undefined` (`npm/cli.js` does).
        match name.as_str() {
            "locations" => {
                if value.is_undefined() {
                    continue;
                }
                parsed.locations = value
                    .as_bool()
                    .ok_or_else(|| err("parse option 'locations' must be a boolean"))?;
            }
            "goal" => {
                if value.is_undefined() {
                    continue;
                }
                if !allow_goal {
                    return Err(err("parse option 'goal' is only supported for TypeScript"));
                }
                let goal = value
                    .as_string()
                    .ok_or_else(|| err("parse option 'goal' must be 'script' or 'module'"))?;
                parsed.goal = goal_from_str(&goal)?;
            }
            other => {
                let expected = if allow_goal {
                    "'locations' or 'goal'"
                } else {
                    "'locations'"
                };
                return Err(err(format!(
                    "unknown parse option '{other}' (expected {expected})"
                )));
            }
        }
    }
    Ok(parsed)
}

/// The per-language parse call behind the uniform exports. `goal` (TypeScript)
/// threads `ParseOptions.goal` into `parse_with_goal`; `nogoal` (Svelte, CSS)
/// ignores it — the option key is already rejected by `parse_parse_options`.
#[cfg(feature = "parse")]
macro_rules! parse_ast {
    (goal, $lang:ident, $source:expr, $goal:expr, $arena:expr) => {
        $lang::parse_with_goal($source, $goal, $arena)
    };
    (nogoal, $lang:ident, $source:expr, $goal:expr, $arena:expr) => {{
        // Consume the (always-`Module`) goal so the options binding stays used
        // in every expansion.
        let _ = $goal;
        $lang::parse($source, $arena)
    }};
}

/// Whether `parse_parse_options` accepts the `goal` key for this language.
#[cfg(feature = "parse")]
macro_rules! goal_allowed {
    (goal) => {
        true
    };
    (nogoal) => {
        false
    };
}

#[cfg(feature = "format")]
macro_rules! parse_format {
    ($lang:ident, $source:expr) => {
        with_ast_arena(|arena| {
            let ast = $lang::parse($source, arena).map_err(err)?;
            Ok(with_doc_arena(|doc_arena| {
                $lang::format_in(&ast, $source, doc_arena)
            }))
        })
    };
}

/// Generate `parse_<lang>` / `parse_<lang>_json` / `parse_internal_<lang>` /
/// `format_<lang>` WASM functions for one language module. The parse exports
/// are gated on `parse` (so the format-only build excludes the convert layer)
/// and `format_*` on `format` (so the parse-only build drops the printers at
/// link time). Every parse export shares one uniform signature,
/// `(source, options?)` — the `{locations?, goal?}` bag read by
/// `parse_parse_options`, with `$goalness` (`goal` / `nogoal`) selecting
/// whether the TypeScript-only `goal` key is accepted and threaded. Their
/// `.d.ts` is the hand-written `TS_PARSE_DECLS` block above (each export is
/// `skip_typescript`), so a signature change here must update that block too.
// The bodies parse the source into a per-thread AST arena and run the
// conversion/format/no-op over it. Every language crate is interner-free
// (identifier and element/attribute names are span-identity), so these are
// uniform across svelte/typescript/css — no per-language arity split. WASM is
// single-threaded, so the arena thread-local is a module static.
macro_rules! lang_bindings {
    (
        $goalness:ident,
        $parse_fn:ident,
        $parse_json_fn:ident,
        $parse_internal_fn:ident,
        $format_fn:ident,
        $lang:ident $(,)?
    ) => {
        /// Parse source into the typed JSON AST (`options`: `{locations?, goal?}`,
        /// see `TS_PARSE_DECLS` / `parse_parse_options`).
        #[cfg(feature = "parse")]
        #[wasm_bindgen(skip_typescript)]
        pub fn $parse_fn(source: &str, options: JsValue) -> Result<JsValue, JsError> {
            let json = $parse_json_fn(source, options)?;
            js_sys::JSON::parse(&json)
                .map_err(|_| err("internal error: AST serialized to invalid JSON"))
        }

        /// Parse source into the JSON AST as a compact JSON string, skipping
        /// JS object materialization (for consumers forwarding the wire format).
        #[cfg(feature = "parse")]
        #[wasm_bindgen(skip_typescript)]
        pub fn $parse_json_fn(source: &str, options: JsValue) -> Result<String, JsError> {
            let opts = parse_parse_options(&options, goal_allowed!($goalness))?;
            with_ast_arena(|arena| {
                let ast = parse_ast!($goalness, $lang, source, opts.goal, arena).map_err(err)?;
                Ok(if opts.locations {
                    $lang::convert_ast_json_string(&ast, source)
                } else {
                    $lang::convert_ast_json_string_no_locations(&ast, source)
                })
            })
        }

        /// Parse only, no serialization — the benchmark coverage/throughput
        /// probe. `options.locations` is inert (no wire is emitted).
        #[cfg(feature = "parse")]
        #[wasm_bindgen(skip_typescript)]
        pub fn $parse_internal_fn(source: &str, options: JsValue) -> Result<(), JsError> {
            let opts = parse_parse_options(&options, goal_allowed!($goalness))?;
            with_ast_arena(|arena| {
                let ast = parse_ast!($goalness, $lang, source, opts.goal, arena).map_err(err)?;
                std::hint::black_box(&ast);
                Ok(())
            })
        }

        #[cfg(feature = "format")]
        #[wasm_bindgen]
        pub fn $format_fn(source: &str) -> Result<String, JsError> {
            parse_format!($lang, source)
        }
    };
}

lang_bindings!(
    nogoal,
    parse_svelte,
    parse_svelte_json,
    parse_internal_svelte,
    format_svelte,
    tsv_svelte,
);
lang_bindings!(
    goal,
    parse_typescript,
    parse_typescript_json,
    parse_internal_typescript,
    format_typescript,
    tsv_ts,
);
lang_bindings!(
    nogoal,
    parse_css,
    parse_css_json,
    parse_internal_css,
    format_css,
    tsv_css,
);

// --- TypeScript goal-aware format export ---
//
// The parse goal (`Script` vs `Module`) is a TypeScript-only axis — Svelte
// `<script>` is always a module and CSS has no goal. The parse exports take it
// as the `goal` option; format keeps a distinct flat export instead of an
// options bag because parsing one in Rust needs `js_sys`, which deliberately
// rides only the `parse` feature — an options object here would newly weigh
// down the format-only package. See `tsv format --goal`.

/// Parse a goal string (`"script"` / `"module"`), mirroring `tsv_cli`'s
/// `parse_goal_arg`. Used by the parse exports' `goal` option and by
/// `format_typescript_with_goal`.
#[cfg(any(feature = "parse", feature = "format"))]
fn goal_from_str(goal: &str) -> Result<tsv_ts::Goal, JsError> {
    tsv_ts::Goal::from_source_type(goal).ok_or_else(|| {
        err(format!(
            "invalid goal '{goal}' (expected 'script' or 'module')"
        ))
    })
}

/// `format_typescript` against an explicit goal (`"script"` / `"module"`).
#[cfg(feature = "format")]
#[wasm_bindgen]
pub fn format_typescript_with_goal(source: &str, goal: &str) -> Result<String, JsError> {
    let goal = goal_from_str(goal)?;
    with_ast_arena(|arena| {
        let ast = tsv_ts::parse_with_goal(source, goal, arena).map_err(err)?;
        Ok(with_doc_arena(|doc_arena| {
            tsv_ts::format_in(&ast, source, doc_arena)
        }))
    })
}
