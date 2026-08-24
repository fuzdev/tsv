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
// The goal-axis macros come from the same crate, so the three bindings share ONE
// definition of which languages have a goal rather than three hand-synced copies.
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;
#[cfg(any(feature = "parse", feature = "format"))]
use tsv_arena::{goal_allowed, parse_ast};

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
// `console_error_panic_hook`: the binding is three lines, and `console` lives in
// `web_sys` (not the `js-sys` the options readers already use), so either would
// be a new dependency on every package for those three lines.
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
    #[expect(clippy::new_without_default)] // wasm-bindgen exports the constructor
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

    /// The argument error for an explicitly named **file** whose extension tsv
    /// doesn't format, delegating to `tsv_discover::unsupported_extension_error`.
    /// Returns `undefined` (the JS view of `None`) when the extension is
    /// formattable. A method (not a free function) so it rides the `IgnoreStack`
    /// class re-export through the package facade; the receiver is unused — an
    /// argument check runs before any matcher exists. Single source of truth with
    /// the native CLI, including the rendered extension list, so `npm/cli.js`
    /// never hand-mirrors `FORMATTABLE_EXTENSIONS`.
    pub fn unsupported_extension_error(&self, path: &str) -> Option<String> {
        tsv_discover::unsupported_extension_error(path)
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
///
/// Spelled `./tsv_ast.js`, here and in every `import(...)` below: under
/// `moduleResolution: node16`/`nodenext` a relative specifier inside a
/// declaration file must carry the runtime extension or the consumer gets
/// TS2834, and TypeScript resolves `./tsv_ast.js` to `./tsv_ast.d.ts`. The
/// extensionless form only survives `skipLibCheck` or the legacy resolver.
#[cfg(feature = "parse")]
#[wasm_bindgen(typescript_custom_section)]
const TS_AST_REEXPORT: &'static str = r#"
export type * from "./tsv_ast.js";
"#;

/// Hand-written declarations for the parse exports, which are all
/// `#[wasm_bindgen(skip_typescript)]`: wasm-bindgen can't express an
/// options-dependent return type, and `{locations: false}` deliberately
/// returns a shape `tsv_ast.d.ts` can't name (its interfaces declare `loc`
/// required), so that overload returns `any` and must come first (the more
/// specific signature). A signature change in `lang_bindings!` must update
/// this block too; `ParseOptions` / `TypeScriptParseOptions` are re-exported
/// through the npm facade (`scripts/patch_npm_package.ts`).
///
/// Every option key spells `| undefined` on top of `?`, here and in
/// `TS_FORMAT_DECLS`. That is not redundant: under a consumer's
/// `exactOptionalPropertyTypes`, a bare `?` accepts an ABSENT key but rejects
/// one explicitly set to `undefined` — and setting it to `undefined` is the
/// documented forwarding idiom (`npm/cli.js` builds `{goal: <maybe undefined>}`
/// and hands it to whichever export). Dropping the `| undefined` would leave
/// the types contradicting the runtime for exactly the caller the option's
/// leniency exists to serve.
#[cfg(feature = "parse")]
#[wasm_bindgen(typescript_custom_section)]
const TS_PARSE_DECLS: &'static str = r#"
/**
 * Options accepted by `parse_svelte` / `parse_css` (and their `_json` /
 * `_internal` siblings). The parse goal is TypeScript's alone, so it is
 * declared here as `undefined`-only rather than omitted: a set `goal` throws,
 * but spelling the inapplicable goal `undefined` forwards one bag to whichever
 * parser, exactly as the runtime does.
 */
export interface ParseOptions {
	/**
	 * Emit per-node `loc` (line/column) — the drop-in acorn/svelte wire.
	 * `false` emits the span-only wire (~46% smaller; Svelte also omits
	 * `name_loc`): `loc` stays derivable from `start`/`end` plus the source —
	 * see `reconstruct_locations` / `create_locator`. Inert for CSS (its wire
	 * has no `loc`) and for `parse_internal_*` (no wire at all).
	 * @default true
	 */
	locations?: boolean | undefined;
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `goal` throws. See `TypeScriptParseOptions`.
	 */
	goal?: undefined;
}

/** The TypeScript parsers' bag: the same keys, with `goal` settable. */
export interface TypeScriptParseOptions {
	/** As `ParseOptions.locations`. @default true */
	locations?: boolean | undefined;
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module' | undefined;
}

export function parse_svelte(source: string, options: ParseOptions & { locations: false }): any;
export function parse_svelte(source: string, options?: ParseOptions): import('./tsv_ast.js').Root;
export function parse_svelte_json(source: string, options?: ParseOptions): string;
export function parse_internal_svelte(source: string, options?: ParseOptions): void;

export function parse_typescript(
	source: string,
	options: TypeScriptParseOptions & { locations: false }
): any;
export function parse_typescript(
	source: string,
	options?: TypeScriptParseOptions
): import('./tsv_ast.js').Program;
export function parse_typescript_json(source: string, options?: TypeScriptParseOptions): string;
export function parse_internal_typescript(
	source: string,
	options?: TypeScriptParseOptions
): void;

export function parse_css(source: string, options?: ParseOptions): import('./tsv_ast.js').StyleSheetFile;
export function parse_css_json(source: string, options?: ParseOptions): string;
export function parse_internal_css(source: string, options?: ParseOptions): void;
"#;

/// Hand-written declarations for the format exports, which are all
/// `#[wasm_bindgen(skip_typescript)]`: a `JsValue` parameter generates as a
/// **required** `options: any`, which would both untype the bag and break every
/// existing arity-1 `format_<lang>(source)` call at compile time. A signature
/// change in `lang_bindings!` must update this block too; `FormatOptions` /
/// `TypeScriptFormatOptions` are re-exported through the npm facade
/// (`scripts/patch_npm_package.ts`).
///
/// `FormatOptions` declares `goal?: undefined` rather than being `{}`, and
/// `ParseOptions` declares it rather than omitting it. Two reasons — the first
/// bites `FormatOptions` alone, the second both:
///
/// 1. An EMPTY interface opts out of both excess-property checking and
///    weak-type detection, so `{}` accepts every non-nullish value —
///    `{locatons: false}`, `'script'`, `42` — leaving `format_svelte` /
///    `format_css` with no compile-time guard at all while the runtime rejects
///    each. (`ParseOptions` was never empty; it has `locations`.)
/// 2. A declared `goal?: undefined` is what makes the FORWARDING idiom
///    type-check: `npm/cli.js` builds `{goal: <maybe undefined>}` and hands it
///    to whichever export, so a bag with a `goal` key set to `undefined` must
///    be legal on the languages that reject a *set* goal. Omitting the key
///    rejects that bag (excess property), `never` rejects it under
///    `exactOptionalPropertyTypes`; `undefined` is the spelling that works.
///
/// The TypeScript interfaces therefore do NOT `extends` their base — a settable
/// `goal` is incompatible with the undefined-only one — at the cost of one
/// duplicated `locations` line. The assignability `extends` would buy is
/// unsound anyway (`format_svelte(src, {goal: 'script'})` throws).
#[cfg(feature = "format")]
#[wasm_bindgen(typescript_custom_section)]
const TS_FORMAT_DECLS: &'static str = r#"
/**
 * Options accepted by `format_svelte` / `format_css`. Formatting itself is
 * non-configurable and the parse goal is TypeScript's alone, so these carry no
 * settable key. Every unknown key throws, `locations` included: that option
 * shapes the parse wire, and format emits no wire.
 */
export interface FormatOptions {
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `goal` throws. Declared (as `undefined`) rather than
	 * omitted so one bag still forwards to whichever formatter: spell the
	 * inapplicable goal `undefined` and this type accepts it, exactly as the
	 * runtime does.
	 */
	goal?: undefined;
}

/** The TypeScript formatter's bag: the same key, settable. */
export interface TypeScriptFormatOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module' | undefined;
}

export function format_svelte(source: string, options?: FormatOptions): string;
export function format_typescript(source: string, options?: TypeScriptFormatOptions): string;
export function format_css(source: string, options?: FormatOptions): string;
"#;

/// Parse a goal string (`"script"` / `"module"`), mirroring `tsv_cli`'s
/// `parse_goal_arg`. Used by the `goal` option of both export families — the
/// parse goal (`Script` vs `Module`) is a TypeScript-only axis, since Svelte
/// `<script>` is always a module and CSS has no goal. See `tsv format --goal`.
#[cfg(any(feature = "parse", feature = "format"))]
fn goal_from_str(goal: &str) -> Result<tsv_ts::Goal, JsError> {
    tsv_ts::Goal::from_source_type(goal).ok_or_else(|| {
        err(format!(
            "invalid goal '{goal}' (expected 'script' or 'module')"
        ))
    })
}

/// Which options bag one export accepts — the noun that names it in every
/// error, plus the supported key set. Both families read `{goal?}`; only parse
/// reads `{locations?}`, because that option selects a **wire** and format
/// emits none. So `locations` is not accepted-and-ignored on a format export,
/// it is an unknown key: an inert-but-accepted spelling would let a caller
/// believe they had asked a formatter for a narrower product. Nothing forwards
/// a parse bag into a format call (`npm/cli.js` builds each at its own call
/// site), so the "one bag, whichever function" property the `goal` arm exists
/// for is untouched by rejecting it.
#[cfg(any(feature = "parse", feature = "format"))]
struct OptionsSpec {
    noun: &'static str,
    locations: bool,
    goal: bool,
}

#[cfg(any(feature = "parse", feature = "format"))]
impl OptionsSpec {
    /// The parse family's bag: `{locations?, goal?}`.
    #[cfg(feature = "parse")]
    const fn parse(goal: bool) -> Self {
        Self {
            noun: "parse",
            locations: true,
            goal,
        }
    }

    /// The format family's bag: `{goal?}` — no wire, so no `locations`.
    #[cfg(feature = "format")]
    const fn format(goal: bool) -> Self {
        Self {
            noun: "format",
            locations: false,
            goal,
        }
    }
}

/// The parsed options bag: `{locations?, goal?}` for the parse exports,
/// `{goal?}` for the format exports.
///
/// `locations` (default `true`) selects the wire: the loc-bearing drop-in
/// contract, or the span-only variant (the language crates'
/// `convert_ast_json_string_no_locations`). It is accepted by every parse
/// export and inert where nothing reads it (CSS emits no `loc`;
/// `parse_internal_*` emits no wire), and rides the `parse` feature — the
/// format-only build has no wire for it to shape. `goal` (default `module`) is
/// TypeScript-only — Svelte hard-wires `Module` and CSS has no goal — so the
/// other languages reject the key rather than silently ignoring a semantic
/// axis. Unknown keys are an error: a typo like `{locatons: false}` silently
/// succeeding would hand back the full wire while the caller believes they
/// opted out.
#[cfg(any(feature = "parse", feature = "format"))]
struct Options {
    #[cfg(feature = "parse")]
    locations: bool,
    goal: tsv_ts::Goal,
}

/// Read an `Options` off the raw `options` argument against `spec`
/// (`undefined`/`null` mean all-defaults; a supported key explicitly set to
/// `undefined` means that key's default, matching the omitted-key JS
/// convention — unknown keys error whatever their value).
///
/// One reader serves both export families so the two can't drift: the parse
/// exports' documented semantics are the format exports' semantics, key for key.
#[cfg(any(feature = "parse", feature = "format"))]
fn read_options(options: &JsValue, spec: OptionsSpec) -> Result<Options, JsError> {
    let mut parsed = Options {
        #[cfg(feature = "parse")]
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
        return Err(err(format!("{} options must be an object", spec.noun)));
    }
    let object: &js_sys::Object = options.unchecked_ref();
    for key in js_sys::Object::keys(object).iter() {
        // `Object.keys` yields only string keys.
        let Some(name) = key.as_string() else {
            return Err(err(format!("{} option keys must be strings", spec.noun)));
        };
        let value = js_sys::Reflect::get(options, &key)
            .map_err(|_| err(format!("failed to read {} option '{name}'", spec.noun)))?;
        // A supported key explicitly set to `undefined` means that key's default
        // (the omitted-key JS convention) — decided per arm, AFTER the key match,
        // so an unknown key errors whatever its value (`{locatons: undefined}` is
        // the same typo as `{locatons: false}`). `goal`'s check runs before its
        // language rejection: that's what lets one bag serve whichever parser —
        // or whichever formatter — with the inapplicable goal spelled `undefined`
        // (`npm/cli.js` does both).
        match name.as_str() {
            // A key the spec doesn't carry falls to the unknown arm, which is
            // how `locations` reads on a format export.
            #[cfg(feature = "parse")]
            "locations" if spec.locations => {
                if value.is_undefined() {
                    continue;
                }
                parsed.locations = value.as_bool().ok_or_else(|| {
                    err(format!(
                        "{} option 'locations' must be a boolean",
                        spec.noun
                    ))
                })?;
            }
            "goal" => {
                if value.is_undefined() {
                    continue;
                }
                if !spec.goal {
                    return Err(err(format!(
                        "{} option 'goal' is only supported for TypeScript",
                        spec.noun
                    )));
                }
                let goal = value.as_string().ok_or_else(|| {
                    err(format!(
                        "{} option 'goal' must be 'script' or 'module'",
                        spec.noun
                    ))
                })?;
                parsed.goal = goal_from_str(&goal)?;
            }
            other => {
                let noun = spec.noun;
                let detail = match (spec.locations, spec.goal) {
                    (true, true) => "expected 'locations' or 'goal'",
                    (true, false) => "expected 'locations'",
                    (false, true) => "expected 'goal'",
                    // The non-TypeScript formatters: formatting is
                    // non-configurable and the goal is TypeScript's alone.
                    (false, false) => "this export takes no options",
                };
                return Err(err(format!("unknown {noun} option '{other}' ({detail})")));
            }
        }
    }
    Ok(parsed)
}

/// Generate `parse_<lang>` / `parse_<lang>_json` / `parse_internal_<lang>` /
/// `format_<lang>` WASM functions for one language module. The parse exports
/// are gated on `parse` (so the format-only build excludes the convert layer)
/// and `format_*` on `format` (so the parse-only build drops the printers at
/// link time). Every export — parse and format alike — shares one uniform
/// signature, `(source, options?)`: the bag read by `read_options`
/// (`{locations?, goal?}` for parse, `{goal?}` for format), with `$goalness`
/// (`goal` / `nogoal`) selecting whether the TypeScript-only `goal` key is
/// accepted and threaded. One package must not teach two calling conventions,
/// so a caller holding a `{goal}` bag hands it to either family. Their `.d.ts`
/// is the hand-written `TS_PARSE_DECLS` / `TS_FORMAT_DECLS` block above (each
/// export is `skip_typescript`), so a signature change here must update those
/// blocks too.
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
        /// see `TS_PARSE_DECLS` / `read_options`).
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
            let opts = read_options(&options, OptionsSpec::parse(goal_allowed!($goalness)))?;
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
            let opts = read_options(&options, OptionsSpec::parse(goal_allowed!($goalness)))?;
            with_ast_arena(|arena| {
                let ast = parse_ast!($goalness, $lang, source, opts.goal, arena).map_err(err)?;
                std::hint::black_box(&ast);
                Ok(())
            })
        }

        /// Format source (`options`: `{goal?}`, see `TS_FORMAT_DECLS` /
        /// `read_options`).
        #[cfg(feature = "format")]
        #[wasm_bindgen(skip_typescript)]
        pub fn $format_fn(source: &str, options: JsValue) -> Result<String, JsError> {
            let opts = read_options(&options, OptionsSpec::format(goal_allowed!($goalness)))?;
            // The format path's line-terminator fold, ahead of the parse — see
            // `tsv_lang::printing::normalize_carriage_returns`. The parse exports
            // deliberately skip it: the wire's offsets are a drop-in contract over the
            // author's own bytes.
            let normalized = tsv_lang::printing::normalize_carriage_returns(source);
            let source = normalized.as_ref();
            with_ast_arena(|arena| {
                let ast = parse_ast!($goalness, $lang, source, opts.goal, arena).map_err(err)?;
                Ok(with_doc_arena(|doc_arena| {
                    $lang::format_in(&ast, source, doc_arena)
                }))
            })
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
