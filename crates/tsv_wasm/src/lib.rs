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
#[cfg(any(feature = "parse", feature = "format"))]
use tsv_arena::goal_allowed;
#[cfg(feature = "parse")]
use tsv_arena::parse_ast;
#[cfg(feature = "format")]
use tsv_arena::parse_ast_for_format;
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
/// `.gitignore` files plus one tsv file per directory (`.formatignore`, or a
/// `.prettierignore` where no sibling `.formatignore` shadows it), then queried
/// per path — both for the raw
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

    /// Push one directory's `.formatignore` as a tsv layer, applied after every
    /// `.gitignore`. `anchor` is the directory relative to the format root (`""` = root).
    pub fn push_formatignore(&mut self, anchor: &str, content: &str) {
        self.inner.push_formatignore(anchor, content);
    }

    /// Push one directory's `.prettierignore` as a tsv layer — the file read, inside a
    /// repo, in a directory with no `.formatignore`. Matched exactly as a `.formatignore`
    /// layer is; what differs is the file a warning about one of its rules names.
    pub fn push_prettierignore(&mut self, anchor: &str, content: &str) {
        self.inner.push_prettierignore(anchor, content);
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
            tsv_discover::DirVerdict::PruneWithWarning => "prune_warn".to_string(),
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

    /// The heuristic-shadow warning a walk raises on the way down to `rel` (a
    /// format-root-relative file path), delegating to
    /// `tsv_discover::path_heuristic_shadow_warning`: `heuristic_shadow_warning`'s text for
    /// the first ancestor directory `is_path_pruned` stops at, when the build-output
    /// heuristic pruned it under a tsv-layer re-include; `undefined` (the JS view of
    /// `None`) otherwise. The per-file companion to `classify_dir`'s `"prune_warn"` for a
    /// consumer with no top-down traversal. `loose_root` is the format root's display path
    /// outside a git repo (`undefined` inside one).
    pub fn path_heuristic_shadow_warning(
        &self,
        rel: &str,
        loose_root: Option<String>,
    ) -> Option<String> {
        tsv_discover::path_heuristic_shadow_warning(rel, loose_root.as_deref(), &self.inner)
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
    /// (format-root relative), delegating to `tsv_discover::heuristic_shadow_warning`;
    /// `undefined` (the JS view of `None`) when no tsv-layer re-include is written under
    /// `dir`. The text names the file holding that rule, which it reads from this stack.
    /// `loose_root` is the format root's display path outside a git repo (`undefined`
    /// inside one). Single source of truth with the native CLI — the JS CLI never
    /// templates this string.
    pub fn heuristic_shadow_warning(
        &self,
        dir: &str,
        loose_root: Option<String>,
    ) -> Option<String> {
        tsv_discover::heuristic_shadow_warning(dir, loose_root.as_deref(), &self.inner)
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

    /// The warning for an in-tree `.gitignore` that is a symbolic link — which git does
    /// not follow, so its rules are not applied — delegating to
    /// `tsv_discover::gitignore_symlink_warning`. `path` is the link's path. A method (not
    /// a free function) so it rides the `IgnoreStack` class re-export through the package
    /// facade; the receiver is unused. Single source of truth with the native CLI.
    pub fn gitignore_symlink_warning(&self, path: &str) -> String {
        tsv_discover::gitignore_symlink_warning(path)
    }

    /// The traversal error for a relative directory root that the working directory cannot
    /// resolve (it was deleted), delegating to `tsv_discover::unresolvable_root_error`. A
    /// method so it rides the class re-export; the receiver is unused — the refusal comes
    /// before any matcher is assembled. Single source of truth with the native CLI.
    pub fn unresolvable_root_error(&self, root: &str) -> String {
        tsv_discover::unresolvable_root_error(root)
    }

    /// The warning for a path an argument named — a file, or a directory root — that an
    /// ignore file puts out of scope, delegating to
    /// `tsv_discover::excluded_argument_warning`; `undefined` (the JS view of `None`) when
    /// no rule excludes it, and for a named file a `.formatignore` or `.prettierignore`
    /// excludes, which is skipped quietly. Whether the path is in scope is
    /// `is_ignored(rel, is_dir)`. `display` is the argument as given, `rel` its
    /// format-root-relative path, `loose_root` the format root's display path outside a git
    /// repo (`undefined` inside one). Single source of truth with the native CLI — the JS
    /// CLI never templates this string.
    pub fn excluded_argument_warning(
        &self,
        display: &str,
        rel: &str,
        is_dir: bool,
        loose_root: Option<String>,
    ) -> Option<String> {
        tsv_discover::excluded_argument_warning(
            display,
            rel,
            is_dir,
            loose_root.as_deref(),
            &self.inner,
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
/// documented forwarding idiom (`npm/cli.js` builds `{sourceType: <maybe undefined>}`
/// and hands it to whichever export). Dropping the `| undefined` would leave
/// the types contradicting the runtime for exactly the caller the option's
/// leniency exists to serve.
#[cfg(feature = "parse")]
#[wasm_bindgen(typescript_custom_section)]
const TS_PARSE_DECLS: &'static str = r#"
/**
 * Options accepted by `parse_svelte` / `parse_css` (and their `_json` /
 * `_internal` siblings). The parse goal is TypeScript's alone, so it is
 * declared here as `undefined`-only rather than omitted: a set `sourceType` throws,
 * but spelling the inapplicable source type `undefined` forwards one bag to whichever
 * parser, exactly as the runtime does.
 */
export interface ParseOptions {
	/**
	 * Emit per-node `loc` (line/column) — the drop-in acorn/svelte wire.
	 * `false` emits the span-only wire (much smaller; Svelte also omits
	 * `name_loc`): `loc` stays derivable from `start`/`end` plus the source,
	 * via this package's own `reconstruct_locations` / `create_locator` /
	 * `loc_of` (which throw on the two Svelte shapes the span-only wire can't
	 * disambiguate — see `locations.d.ts`). Inert for CSS (its wire has no `loc`).
	 * @default true
	 */
	locations?: boolean | undefined;
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `sourceType` throws. See `TypeScriptParseOptions`.
	 */
	sourceType?: undefined;
}

/** The TypeScript parsers' bag: the same keys, with `sourceType` settable. */
export interface TypeScriptParseOptions {
	/** As `ParseOptions.locations`. @default true */
	locations?: boolean | undefined;
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors. A script is also
	 * **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a
	 * module is always strict.
	 * @default 'module'
	 */
	sourceType?: 'script' | 'module' | undefined;
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
/// `FormatOptions` declares `sourceType?: undefined` rather than being `{}`, and
/// `ParseOptions` declares it rather than omitting it. Two reasons — the first
/// bites `FormatOptions` alone, the second both:
///
/// 1. An EMPTY interface opts out of both excess-property checking and
///    weak-type detection, so `{}` accepts every non-nullish value —
///    `{locatons: false}`, `'script'`, `42` — leaving `format_svelte` /
///    `format_css` with no compile-time guard at all while the runtime rejects
///    each. (`ParseOptions` was never empty; it has `locations`.)
/// 2. A declared `sourceType?: undefined` is what makes the FORWARDING idiom
///    type-check: `npm/cli.js` builds `{sourceType: <maybe undefined>}` and hands it
///    to whichever export, so a bag with a `sourceType` key set to `undefined` must
///    be legal on the languages that reject a *set* `sourceType`. Omitting the key
///    rejects that bag (excess property), `never` rejects it under
///    `exactOptionalPropertyTypes`; `undefined` is the spelling that works.
///
/// The TypeScript interfaces therefore do NOT `extends` their base — a settable
/// `sourceType` is incompatible with the undefined-only one — at the cost of one
/// duplicated `locations` line. The assignability `extends` would buy is
/// unsound anyway (`format_svelte(src, {sourceType: 'script'})` throws).
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
	 * goal, so a set `sourceType` throws. Declared (as `undefined`) rather than
	 * omitted so one bag still forwards to whichever formatter: spell the
	 * inapplicable source type `undefined` and this type accepts it, exactly as the
	 * runtime does.
	 */
	sourceType?: undefined;
}

/** The TypeScript formatter's bag: the same key, settable. */
export interface TypeScriptFormatOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors. A script is also
	 * **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a
	 * module is always strict.
	 *
	 * Omitted, the source is formatted as a **module, retried as a script** if
	 * that parse fails — so a legacy sloppy script formats without naming a
	 * grammar, while anything the module grammar accepts is never reinterpreted
	 * (the printer does not read the goal, so no output changes). A set value is
	 * exact: `'module'` refuses a script-only source rather than retrying.
	 * `parse_typescript` has no such fallback — its wire's `Program.sourceType`
	 * is a claim, and omitting the key there means `'module'`.
	 */
	sourceType?: 'script' | 'module' | undefined;
}

export function format_svelte(source: string, options?: FormatOptions): string;
export function format_typescript(source: string, options?: TypeScriptFormatOptions): string;
export function format_css(source: string, options?: FormatOptions): string;
"#;

/// Which options bag one export accepts — the noun that names it in every
/// error, plus the supported key set. Both families read `{sourceType?}`; only parse
/// reads `{locations?}`, because that option selects a **wire** and format
/// emits none. So `locations` is not accepted-and-ignored on a format export,
/// it is an unknown key: an inert-but-accepted spelling would let a caller
/// believe they had asked a formatter for a narrower product. Nothing forwards
/// a parse bag into a format call (`npm/cli.js` builds each at its own call
/// site), so the "one bag, whichever function" property the `sourceType` arm exists
/// for is untouched by rejecting it.
#[cfg(any(feature = "parse", feature = "format"))]
struct OptionsSpec {
    noun: &'static str,
    locations: bool,
    source_type: bool,
}

#[cfg(any(feature = "parse", feature = "format"))]
impl OptionsSpec {
    /// The parse family's bag: `{locations?, sourceType?}`.
    #[cfg(feature = "parse")]
    const fn parse(source_type: bool) -> Self {
        Self {
            noun: "parse",
            locations: true,
            source_type,
        }
    }

    /// The format family's bag: `{sourceType?}` — no wire, so no `locations`.
    #[cfg(feature = "format")]
    const fn format(source_type: bool) -> Self {
        Self {
            noun: "format",
            locations: false,
            source_type,
        }
    }
}

/// The parsed options bag: `{locations?, sourceType?}` for the parse exports,
/// `{sourceType?}` for the format exports.
///
/// `locations` (default `true`) selects the wire: the loc-bearing drop-in
/// contract, or the span-only variant (the language crates'
/// `convert_ast_json_string_no_locations`). It is accepted by every parse
/// export and inert where nothing reads it (CSS emits no `loc`;
/// `parse_internal_*` emits no wire), and rides the `parse` feature — the
/// format-only build has no wire for it to shape. `sourceType` is
/// TypeScript-only — Svelte hard-wires `Module` and CSS has no goal — so the
/// other languages reject the key rather than silently ignoring a semantic
/// axis. Unknown keys are an error: a typo like `{locatons: false}` silently
/// succeeding would hand back the full wire while the caller believes they
/// opted out.
///
/// An **unset** `sourceType` is `None` rather than `Module`, because the two
/// families answer it differently: a parse reads it as `module` (its wire's
/// `Program.sourceType` is a claim one settled grammar has to produce), while a
/// format reads it as "no source type named" and parses at `Module` with a `Script`
/// retry — which is what lets an editor's bare `format_typescript(source)` format a
/// legacy sloppy script. A SET value is exact on both.
#[cfg(any(feature = "parse", feature = "format"))]
#[cfg_attr(test, derive(Debug))]
struct Options {
    #[cfg(feature = "parse")]
    locations: bool,
    source_type: Option<tsv_ts::Goal>,
}

/// One option's value as the JS side handed it over, classified to the three shapes
/// the reader distinguishes. The `JsValue` stops here: [`resolve_options`] below
/// decides over this enum alone, which is what lets `cargo test` grade the decision
/// natively (a `JsValue` cannot be built off the wasm target).
#[cfg(any(feature = "parse", feature = "format"))]
#[cfg_attr(test, derive(Clone))]
enum OptionValue {
    Undefined,
    /// The only key that accepts one is `locations`, which is parse-only — so a
    /// format-only build classifies a boolean and then never reads the payload.
    /// Kept rather than cfg'd away: [`OptionValue::classify`] is the one place the
    /// three shapes are named, and a variant that appears under one feature would
    /// make the `Str(_) | Other` refusal arms read differently per build.
    #[cfg_attr(
        not(feature = "parse"),
        expect(
            dead_code,
            reason = "the payload is read only by the parse-only `locations` arm"
        )
    )]
    Bool(bool),
    Str(String),
    /// Anything else — a number, an object, `null` — which no key accepts.
    Other,
}

#[cfg(any(feature = "parse", feature = "format"))]
impl OptionValue {
    fn classify(value: &JsValue) -> Self {
        if value.is_undefined() {
            Self::Undefined
        } else if let Some(b) = value.as_bool() {
            Self::Bool(b)
        } else if let Some(s) = value.as_string() {
            Self::Str(s)
        } else {
            Self::Other
        }
    }
}

/// Read an `Options` off the raw `options` argument against `spec`
/// (`undefined`/`null` mean all-defaults; a supported key explicitly set to
/// `undefined` means that key's default, matching the omitted-key JS
/// convention — unknown keys error whatever their value).
///
/// One reader serves both export families so the two can't drift: the parse
/// exports' documented semantics are the format exports' semantics, key for key.
/// This half only reaches into the JS object — the shape test and the key/value
/// walk; every decision about a key is [`resolve_options`]'s.
#[cfg(any(feature = "parse", feature = "format"))]
fn read_options(options: &JsValue, spec: OptionsSpec) -> Result<Options, JsError> {
    if options.is_undefined() || options.is_null() {
        return resolve_options(std::iter::empty(), spec).map_err(err);
    }
    // An array is `typeof 'object'` and yields no keys, so without the second
    // test a positional-style `parse_typescript(src, ['script'])` would read as
    // all-defaults — the same silent-opt-out the unknown-key error exists to
    // prevent. (A keyless non-plain object, e.g. `new Date()`, still defaults;
    // ruling that out needs a prototype test this doesn't earn.)
    if !options.is_object() || js_sys::Array::is_array(options) {
        return Err(err(format!("{} options must be an object", spec.noun)));
    }
    let object: &js_sys::Object = options.unchecked_ref();
    let mut entries = Vec::new();
    for key in js_sys::Object::keys(object).iter() {
        // `Object.keys` yields only string keys.
        let Some(name) = key.as_string() else {
            return Err(err(format!("{} option keys must be strings", spec.noun)));
        };
        let value = js_sys::Reflect::get(options, &key)
            .map_err(|_| err(format!("failed to read {} option '{name}'", spec.noun)))?;
        entries.push((name, OptionValue::classify(&value)));
    }
    resolve_options(entries, spec).map_err(err)
}

/// The decision behind [`read_options`], over the bag's entries in key order: which
/// keys `spec` admits, what each accepts, and how each refusal is worded. Pure, so
/// the unit tests below grade it on the native target — the only Rust-side gate on
/// a reader the loader `crates/tsv_napi/npm/index.js` restates by hand.
#[cfg(any(feature = "parse", feature = "format"))]
fn resolve_options(
    entries: impl IntoIterator<Item = (String, OptionValue)>,
    spec: OptionsSpec,
) -> Result<Options, String> {
    let mut parsed = Options {
        #[cfg(feature = "parse")]
        locations: true,
        source_type: None,
    };
    for (name, value) in entries {
        // A supported key explicitly set to `undefined` means that key's default
        // (the omitted-key JS convention) — decided per arm, AFTER the key match,
        // so an unknown key errors whatever its value (`{locatons: undefined}` is
        // the same typo as `{locatons: false}`). `sourceType`'s check runs before
        // its language rejection: that's what lets one bag serve whichever parser —
        // or whichever formatter — with the inapplicable source type spelled `undefined`
        // (`npm/cli.js` does both).
        match name.as_str() {
            // A key the spec doesn't carry falls to the unknown arm, which is
            // how `locations` reads on a format export.
            #[cfg(feature = "parse")]
            "locations" if spec.locations => {
                parsed.locations = match value {
                    OptionValue::Undefined => continue,
                    OptionValue::Bool(b) => b,
                    OptionValue::Str(_) | OptionValue::Other => {
                        return Err(format!(
                            "{} option 'locations' must be a boolean",
                            spec.noun
                        ));
                    }
                };
            }
            "sourceType" => {
                if matches!(value, OptionValue::Undefined) {
                    continue;
                }
                if !spec.source_type {
                    return Err(tsv_arena::source_type_unsupported_message(spec.noun));
                }
                let OptionValue::Str(source_type) = value else {
                    return Err(format!(
                        "{} option 'sourceType' must be 'script' or 'module'",
                        spec.noun
                    ));
                };
                parsed.source_type = Some(
                    tsv_ts::Goal::from_source_type(&source_type)
                        .ok_or_else(|| tsv_arena::invalid_source_type_message(&source_type))?,
                );
            }
            other => {
                let noun = spec.noun;
                let detail = match (spec.locations, spec.source_type) {
                    (true, true) => "expected 'locations' or 'sourceType'",
                    (true, false) => "expected 'locations'",
                    (false, true) => "expected 'sourceType'",
                    // The non-TypeScript formatters: formatting is
                    // non-configurable and the source type is TypeScript's alone.
                    (false, false) => "this export takes no options",
                };
                return Err(format!("unknown {noun} option '{other}' ({detail})"));
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
/// (`{locations?, sourceType?}` for parse, `{sourceType?}` for format), with `$goalness`
/// (`goal` / `nogoal`) selecting whether the TypeScript-only `sourceType` key is
/// accepted and threaded. One package must not teach two calling conventions,
/// so a caller holding a `{sourceType}` bag hands it to either family. Their `.d.ts`
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
        /// Parse source into the typed JSON AST (`options`: `{locations?, sourceType?}`,
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
                let ast = parse_ast!(
                    $goalness,
                    $lang,
                    source,
                    opts.source_type.unwrap_or(tsv_ts::Goal::Module),
                    arena
                )
                .map_err(err)?;
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
                let ast = parse_ast!(
                    $goalness,
                    $lang,
                    source,
                    opts.source_type.unwrap_or(tsv_ts::Goal::Module),
                    arena
                )
                .map_err(err)?;
                std::hint::black_box(&ast);
                Ok(())
            })
        }

        /// Format source (`options`: `{sourceType?}`, see `TS_FORMAT_DECLS` /
        /// `read_options`).
        #[cfg(feature = "format")]
        #[wasm_bindgen(skip_typescript)]
        pub fn $format_fn(source: &str, options: JsValue) -> Result<String, JsError> {
            let opts = read_options(&options, OptionsSpec::format(goal_allowed!($goalness)))?;
            // The format path's line-terminator fold, ahead of the parse — see
            // `tsv_lang::printing::normalize_carriage_returns`. The parse exports
            // deliberately skip it: the wire's offsets are a drop-in contract over the
            // author's own bytes.
            let folded = tsv_lang::printing::normalize_carriage_returns(source);
            let source = folded.text();
            with_ast_arena(|arena| {
                let ast = parse_ast_for_format!($goalness, $lang, source, opts.source_type, arena)
                    .map_err(err)?;
                Ok(with_doc_arena(|doc_arena| {
                    $lang::format_folded_in(&ast, &folded, doc_arena)
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

#[cfg(all(test, any(feature = "parse", feature = "format")))]
mod tests {
    use super::{OptionValue, OptionsSpec, resolve_options};

    fn entries(pairs: &[(&str, OptionValue)]) -> Vec<(String, OptionValue)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    }

    /// The reader the loader (`npm/index.js`) restates by hand: every refusal's
    /// wording, and the two `undefined` conventions (an omitted or `undefined`
    /// supported key is its default; an unknown key errors whatever its value).
    #[test]
    fn resolve_options_words_every_refusal_and_reads_undefined_as_default() {
        #[cfg(feature = "parse")]
        {
            let spec = OptionsSpec::parse(true);
            let parsed = resolve_options(std::iter::empty(), spec).unwrap();
            assert!(parsed.locations);
            assert!(parsed.source_type.is_none());

            let parsed = resolve_options(
                entries(&[
                    ("locations", OptionValue::Bool(false)),
                    ("sourceType", OptionValue::Str("script".to_owned())),
                ]),
                OptionsSpec::parse(true),
            )
            .unwrap();
            assert!(!parsed.locations);
            assert_eq!(parsed.source_type, Some(tsv_ts::Goal::Script));

            // an explicit `undefined` is the omitted key
            let parsed = resolve_options(
                entries(&[
                    ("locations", OptionValue::Undefined),
                    ("sourceType", OptionValue::Undefined),
                ]),
                OptionsSpec::parse(false),
            )
            .unwrap();
            assert!(parsed.locations);
            assert!(parsed.source_type.is_none());

            let refused = |pairs: &[(&str, OptionValue)], source_type: bool| {
                resolve_options(entries(pairs), OptionsSpec::parse(source_type)).unwrap_err()
            };
            assert_eq!(
                refused(&[("locations", OptionValue::Str("no".to_owned()))], true),
                "parse option 'locations' must be a boolean"
            );
            assert_eq!(
                refused(
                    &[("sourceType", OptionValue::Str("module".to_owned()))],
                    false
                ),
                "parse option 'sourceType' is only supported for TypeScript"
            );
            assert_eq!(
                refused(&[("sourceType", OptionValue::Bool(true))], true),
                "parse option 'sourceType' must be 'script' or 'module'"
            );
            assert_eq!(
                refused(
                    &[("sourceType", OptionValue::Str("sloppy".to_owned()))],
                    true
                ),
                "invalid sourceType 'sloppy' (expected 'script' or 'module')"
            );
            assert_eq!(
                refused(&[("locatons", OptionValue::Undefined)], true),
                "unknown parse option 'locatons' (expected 'locations' or 'sourceType')"
            );
            assert_eq!(
                refused(
                    &[
                        ("sourceType", OptionValue::Undefined),
                        ("x", OptionValue::Other)
                    ],
                    false
                ),
                "unknown parse option 'x' (expected 'locations')"
            );
        }
        #[cfg(feature = "format")]
        {
            let refused = |pairs: &[(&str, OptionValue)], source_type: bool| {
                resolve_options(entries(pairs), OptionsSpec::format(source_type)).unwrap_err()
            };
            // `locations` selects a wire, and a format emits none: unknown, not inert
            assert_eq!(
                refused(&[("locations", OptionValue::Bool(false))], true),
                "unknown format option 'locations' (expected 'sourceType')"
            );
            assert_eq!(
                refused(&[("anything", OptionValue::Bool(true))], false),
                "unknown format option 'anything' (this export takes no options)"
            );
            assert_eq!(
                refused(
                    &[("sourceType", OptionValue::Str("script".to_owned()))],
                    false
                ),
                "format option 'sourceType' is only supported for TypeScript"
            );
            let parsed = resolve_options(
                entries(&[("sourceType", OptionValue::Str("module".to_owned()))]),
                OptionsSpec::format(true),
            )
            .unwrap();
            assert_eq!(parsed.source_type, Some(tsv_ts::Goal::Module));
        }
    }
}
