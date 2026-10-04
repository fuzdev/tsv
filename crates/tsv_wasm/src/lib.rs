//! WebAssembly bindings for tsv.
//!
//! Three builds from two features:
//! - default (`@fuzdev/tsv-wasm`): everything — `format_*` plus the parse exports.
//! - `--no-default-features --features format` (`@fuzdev/tsv-format-wasm`):
//!   `format_*` exports only.
//! - `--no-default-features --features parse` (`@fuzdev/tsv-parse-wasm`):
//!   `parse_*`, `parse_*_json`, and `parse_internal_*` plus the convert layer
//!   that serializes ASTs to JS; the printers drop out at link time.
//!
//! The AST crosses the JS boundary as a single JSON string — the span-only wire,
//! the one every binding emits: `parse_*` calls the engine's native `JSON.parse` on
//! it (via `js_sys`) and returns the object; `parse_*_json` returns the string
//! itself for consumers that forward the wire format without materializing.
//! Building the JS object graph node-by-node with `serde_wasm_bindgen` is
//! measurably slower.
//!
//! Every export is flat, `(source, source_type?)`. The published packages wrap them
//! in a hand-written JS facade (`npm/api.js`, shared with the native `@fuzdev/tsv`)
//! that owns the options bag, its errors, and the `{locations: true}` sugar.

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
// The goal-axis macros and every export body (`parse_convert!`, `parse_internal!`,
// `parse_format!`) come from the same crate, so the three bindings share ONE definition
// of which languages have a goal and of what each export does, rather than three
// hand-synced copies.
#[cfg(feature = "format")]
use tsv_arena::parse_format;
#[cfg(any(feature = "parse", feature = "format"))]
use tsv_arena::{Family, goal_allowed};
#[cfg(feature = "parse")]
use tsv_arena::{parse_convert, parse_internal};

// The one parse-error type, shared by every language crate: each re-exports
// `tsv_lang::ParseError` (and the `WirePoint` its `wire_point` returns), so the shared
// error helpers below take a Svelte or CSS failure as readily as a TypeScript one. Named
// through `tsv_ts` only because this crate takes no `tsv_lang` edge of its own.
#[cfg(any(feature = "parse", feature = "format"))]
use tsv_ts::ParseError;

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
// `web_sys` (not the `js-sys` the parse exports use), so either would be a new
// dependency on every package for those three lines.
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

/// A refusal or an internal failure — a source type the export cannot take, a source over
/// the size cap, a format refusal — as a plain JS `Error` carrying `message`.
fn err(message: impl ToString) -> JsValue {
    JsError::new(&message.to_string()).into()
}

// The JS `Error` a parse failure is thrown as, with its point set on it as own properties.
// Declared by hand rather than taken from `js_sys::Error`: the format-only build carries no
// `js-sys`, and a constructor, three setters and `Reflect.deleteProperty` are all this
// needs (the `console_error` binding above is the same trade).
#[cfg(any(feature = "parse", feature = "format"))]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = Error)]
    type PointedError;

    #[wasm_bindgen(constructor, js_class = "Error")]
    fn new(message: &str) -> PointedError;

    #[wasm_bindgen(method, setter)]
    fn set_start(this: &PointedError, value: u32);

    #[wasm_bindgen(method, setter)]
    fn set_line(this: &PointedError, value: u32);

    #[wasm_bindgen(method, setter)]
    fn set_column(this: &PointedError, value: u32);

    #[wasm_bindgen(js_namespace = Reflect, js_name = deleteProperty)]
    fn delete_property(target: &PointedError, key: &str) -> bool;
}

/// A parse failure as the JS `Error` the export throws: its message, plus — for every
/// located error — own enumerable numeric `start`, `line` and `column` properties, the
/// error's point in the document's wire coordinates (`ParseError::wire_point`). The npm
/// facade (`npm/api.js`) reads those three to rethrow it as the published `SyntaxError`, so
/// these names are its contract, shared with `tsv_napi`. A positionless error (a source over
/// the size cap, a format refusal) stays a plain `Error`.
///
/// Each property is deleted before it is set, so all three are own ENUMERABLE data
/// properties — what the facade requires of a point. Bun gives every `Error` its own
/// non-enumerable `line` and `column` at construction, and a plain set writes through
/// that property and keeps it non-enumerable; with nothing to delete (Node, Deno, a
/// browser) the delete is a no-op.
#[cfg(any(feature = "parse", feature = "format"))]
fn parse_err(e: &ParseError) -> JsValue {
    let message = e.to_string();
    let Some(point) = e.wire_point() else {
        return err(message);
    };
    let error = PointedError::new(&message);
    for key in ["start", "line", "column"] {
        delete_property(&error, key);
    }
    error.set_start(point.start);
    error.set_line(point.line);
    error.set_column(point.column);
    error.into()
}

/// The hierarchical, git-faithful matcher for tsv's discovery ignore files, wrapping
/// `tsv_ignore::IgnoreStack`. Built up by the caller from a repo's `.gitignore` files plus
/// one tsv file per directory (`.formatignore`, or a `.prettierignore` where no sibling
/// `.formatignore` shadows it), pushed shallowest-first, then queried per path — for the
/// raw ignore status (`is_ignored`) and for the discovery verdicts and warning strings it
/// delegates to `tsv_discover` (`classify_dir`, `should_format_file`, `is_path_pruned`,
/// `excluded_argument_warning`, …). Exposed so every JS surface (`npm/cli.js`, the VS Code
/// extension) shares the exact same matcher **and** prune decision as the native CLI —
/// agreement by construction. Format-capable builds only: discovery exists to feed the
/// formatter.
///
/// The N-API twin (`tsv_napi`'s `IgnoreStack`) mirrors it — same method names, argument
/// order and return shapes — so `npm/cli.js` drives either package's copy unchanged. Built
/// only into `@fuzdev/tsv-format-wasm` and `@fuzdev/tsv-wasm`; the parse-only package omits it.
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
    /// `is_dir` marks directories so trailing-`/` patterns apply — a symbolic link is
    /// not one, whatever it points at, as git reads one.
    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        self.inner.is_ignored(path, is_dir)
    }

    /// The discovery verdict for one child **directory**, delegating to
    /// `tsv_discover::classify_dir` — the safety-net / build-output-heuristic / matcher
    /// decision shared with the native CLI — as its tag (`DirVerdict::as_tag`):
    /// `"descend"`, `"prune"`, or `"prune_warn"`. `name` is the directory's final path
    /// segment, `child_rel` its format-root-relative `/`-separated path, and
    /// `heuristic_active` is true while no `.gitignore` governs this level. On
    /// `"prune_warn"` the caller fetches the message via
    /// [`shadow_warning`](IgnoreStack::shadow_warning).
    ///
    /// A string tag rather than an enum or a returned struct: it allocates no JS object on
    /// the common descend path, and both packages' declarations name the three tags, so
    /// the two type-check interchangeably.
    #[wasm_bindgen(unchecked_return_type = "'descend' | 'prune' | 'prune_warn'")]
    pub fn classify_dir(&self, name: &str, child_rel: &str, heuristic_active: bool) -> String {
        tsv_discover::classify_dir(name, child_rel, heuristic_active, &self.inner)
            .as_tag()
            .to_string()
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

    /// The shadow warning a walk raises on the way down to `rel` (a format-root-relative
    /// file path), delegating to `tsv_discover::path_shadow_warning`: `shadow_warning`'s
    /// text for the first ancestor directory `is_path_pruned` stops at, when it was pruned
    /// — by the build-output heuristic or by an ignore rule — under a tsv-layer
    /// re-include; `undefined` (the JS view of `None`) otherwise. The per-file companion to `classify_dir`'s `"prune_warn"` for a
    /// consumer with no top-down traversal. `loose_root` is the format root's display path
    /// outside a git repo (`undefined` inside one).
    pub fn path_shadow_warning(&self, rel: &str, loose_root: Option<String>) -> Option<String> {
        tsv_discover::path_shadow_warning(rel, loose_root.as_deref(), &self.inner)
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

    /// The shadow warning text for a pruned directory `dir` (format-root relative) — pruned
    /// by the build-output heuristic or by an ignore rule, which the text says —
    /// delegating to `tsv_discover::shadow_warning`; `undefined` (the JS view of `None`)
    /// when no tsv-layer re-include is written under `dir`. The text names the file holding that rule, which it reads from this stack.
    /// `loose_root` is the format root's display path outside a git repo (`undefined`
    /// inside one). Single source of truth with the native CLI — the JS CLI never
    /// templates this string.
    pub fn shadow_warning(&self, dir: &str, loose_root: Option<String>) -> Option<String> {
        tsv_discover::shadow_warning(dir, loose_root.as_deref(), &self.inner)
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
}

/// What a flat export's `source_type` argument held, read off the raw `JsValue`.
///
/// The argument is a `JsValue` rather than an `Option<String>` because wasm-bindgen's
/// release glue does not refuse a wrong-typed one: it coerces a number, a boolean or an
/// object to `""`, which then reads as `invalid sourceType ''` (and an array throws from
/// inside the glue, `arg.charCodeAt is not a function`). Read here, the type is named.
#[cfg(any(feature = "parse", feature = "format"))]
#[derive(Debug, PartialEq, Eq)]
enum SourceTypeArg {
    /// Omitted, `undefined` or `null`.
    Unset,
    /// A string, not yet checked against the two goals.
    Text(String),
    /// Any other value, by its kind (`typeof`, or `array`).
    Other(String),
}

#[cfg(any(feature = "parse", feature = "format"))]
impl SourceTypeArg {
    fn read(value: &JsValue) -> Self {
        if value.is_null_or_undefined() {
            return Self::Unset;
        }
        if let Some(text) = value.as_string() {
            return Self::Text(text);
        }
        let kind = if value.is_array() {
            "array".to_owned()
        } else {
            value.js_typeof().as_string().unwrap_or_default()
        };
        Self::Other(kind)
    }
}

/// Decode the flat exports' optional `source_type` argument (`"script"` /
/// `"module"`); omitted, `undefined` or `null` stays **unset**.
///
/// The npm packages' facade (`npm/api.js`) validates its options bag before it calls
/// here, so this is the decoder a caller importing the raw wasm-bindgen module past
/// the facade meets — and it still refuses rather than defaulting: a value that is not
/// a string, by its kind (where N-API refuses at its own conversion); then, through
/// the decoder `tsv_napi` shares (`tsv_arena::decode_source_type`, in the facade's own
/// words), a source type on a language with no goal axis (Svelte hard-wires `Module`,
/// CSS has none), and a value naming neither goal.
///
/// What an unset one means is the caller's: a parse reads it as `Module` (its wire's
/// `Program.sourceType` is a claim one settled grammar has to produce), a format as
/// "none named" — the module grammar retried as a script
/// (`tsv_ts::parse_with_goal_or_fallback`).
#[cfg(any(feature = "parse", feature = "format"))]
fn wasm_source_type(
    source_type: SourceTypeArg,
    allowed: bool,
    family: Family,
) -> Result<Option<tsv_ts::Goal>, String> {
    let source_type = match source_type {
        SourceTypeArg::Unset => None,
        SourceTypeArg::Text(text) => Some(text),
        SourceTypeArg::Other(kind) => {
            let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            };
            return Err(format!(
                "invalid sourceType: expected a string ('script' or 'module'), \
                 got {article} {kind}"
            ));
        }
    };
    tsv_arena::decode_source_type(
        source_type.as_deref(),
        allowed,
        family,
        tsv_ts::Goal::from_source_type,
    )
}

/// Generate `parse_<lang>` / `parse_<lang>_json` / `parse_internal_<lang>` /
/// `format_<lang>` WASM functions for one language module. The parse exports
/// are gated on `parse` (so the format-only build excludes the convert layer)
/// and `format_*` on `format` (so the parse-only build drops the printers at
/// link time). Every export shares one flat signature, `(source, source_type?)` —
/// the same one `tsv_napi`'s addon takes — with `$goalness` (`goal` / `nogoal`)
/// selecting whether a source type is accepted. The options bag, the
/// `{locations: true}` sugar and the published declarations are the npm
/// packages' hand-written facade (`npm/api.js` + `facade_format.d.ts`), shared with the
/// native `@fuzdev/tsv`; the declarations wasm-bindgen generates here type only the
/// raw module.
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
        /// Parse source into the span-only AST object — `JSON.parse` of
        /// `parse_<lang>_json`'s wire, run engine-side from Rust.
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_fn(source: &str, source_type: JsValue) -> Result<JsValue, JsValue> {
            let json = $parse_json_fn(source, source_type)?;
            js_sys::JSON::parse(&json)
                .map_err(|_| err("internal error: AST serialized to invalid JSON"))
        }

        /// Parse source into the span-only JSON wire as a compact string —
        /// `start`/`end` offsets, no per-node `loc` (Svelte also no `name_loc`) —
        /// skipping JS object materialization (for consumers forwarding the wire).
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_json_fn(source: &str, source_type: JsValue) -> Result<String, JsValue> {
            let goal = wasm_source_type(
                SourceTypeArg::read(&source_type),
                goal_allowed!($goalness),
                Family::Parse,
            )
            .map_err(err)?;
            parse_convert!(
                $goalness,
                $lang,
                convert_ast_json_string,
                source,
                goal,
                |e| parse_err(&e)
            )
        }

        /// Parse only, no serialization — the benchmark coverage/throughput probe.
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_internal_fn(source: &str, source_type: JsValue) -> Result<(), JsValue> {
            let goal = wasm_source_type(
                SourceTypeArg::read(&source_type),
                goal_allowed!($goalness),
                Family::Parse,
            )
            .map_err(err)?;
            parse_internal!($goalness, $lang, source, goal, |e| parse_err(&e))
        }

        /// Format source. The source type shapes only the parse the formatter runs;
        /// omitted, it means none was named — the module grammar, retried as a script.
        #[cfg(feature = "format")]
        #[wasm_bindgen]
        pub fn $format_fn(source: &str, source_type: JsValue) -> Result<String, JsValue> {
            let goal = wasm_source_type(
                SourceTypeArg::read(&source_type),
                goal_allowed!($goalness),
                Family::Format,
            )
            .map_err(err)?;
            parse_format!($goalness, $lang, source, goal, |e| parse_err(&e))
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
    use super::{Family, SourceTypeArg, wasm_source_type};

    fn text(value: &str) -> SourceTypeArg {
        SourceTypeArg::Text(value.to_owned())
    }

    /// The raw module's decoder: an unset source type stays unset (each family reads
    /// it its own way), a set one decodes exactly, and both refusals are worded as the
    /// facade and `tsv_napi` word them.
    #[test]
    fn wasm_source_type_decodes_and_words_every_refusal() {
        assert_eq!(
            wasm_source_type(SourceTypeArg::Unset, true, Family::Parse),
            Ok(None)
        );
        assert_eq!(
            wasm_source_type(SourceTypeArg::Unset, false, Family::Format),
            Ok(None)
        );
        assert_eq!(
            wasm_source_type(text("script"), true, Family::Parse),
            Ok(Some(tsv_ts::Goal::Script))
        );
        assert_eq!(
            wasm_source_type(text("module"), true, Family::Format),
            Ok(Some(tsv_ts::Goal::Module))
        );
        // a goalless language refuses the axis, `"module"` included
        for (family, noun) in [(Family::Parse, "parse"), (Family::Format, "format")] {
            for value in ["script", "module"] {
                assert_eq!(
                    wasm_source_type(text(value), false, family),
                    Err(format!(
                        "{noun} option 'sourceType' is only supported for TypeScript"
                    ))
                );
            }
        }
        assert_eq!(
            wasm_source_type(text("sloppy"), true, Family::Parse),
            Err("invalid sourceType 'sloppy' (expected 'script' or 'module')".to_owned())
        );
        // the empty string is a string — refused by value, as the facade refuses it
        assert_eq!(
            wasm_source_type(text(""), true, Family::Parse),
            Err("invalid sourceType '' (expected 'script' or 'module')".to_owned())
        );
    }

    /// A value that is not a string is refused by its kind — never coerced to `""` and
    /// read as `invalid sourceType ''`, which is what the glue's `Option<String>` did —
    /// and ahead of the goal-axis refusal, as N-API's conversion refuses it first.
    #[test]
    fn wasm_source_type_names_a_wrong_typed_value() {
        for (kind, article) in [
            ("number", "a"),
            ("boolean", "a"),
            ("object", "an"),
            ("array", "an"),
            ("bigint", "a"),
            ("symbol", "a"),
            ("function", "a"),
        ] {
            for allowed in [true, false] {
                assert_eq!(
                    wasm_source_type(
                        SourceTypeArg::Other(kind.to_owned()),
                        allowed,
                        Family::Parse
                    ),
                    Err(format!(
                        "invalid sourceType: expected a string ('script' or 'module'), \
                         got {article} {kind}"
                    ))
                );
            }
        }
    }
}
