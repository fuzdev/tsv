//! The substrate tsv's three bindings share: the goal axis their exports are generated over,
//! and the format path that prints into the thread's parked doc arena.
//!
//! Both are here for one reason — `tsv_ffi`, `tsv_napi` and `tsv_wasm` would otherwise
//! hand-sync them, and each encodes a contract too subtle to keep three copies of honest (see
//! the crate's CLAUDE.md §Why this crate exists). The goal macros ([`parse_ast!`],
//! [`parse_ast_for_format!`], [`goal_allowed!`]), the export bodies built on them
//! ([`parse_convert!`], [`parse_internal!`], [`parse_imports_json!`], the format path's
//! `parse_format!` and the tape's `parse_tape!`), the export [`Family`] and the string-axis decoder
//! ([`decode_source_type`]) are below, with [`format_panic`], the one text both native
//! bindings report a caught panic with.
//!
//! # The doc arena
//!
//! The format path prints into the thread's one parked `DocArena`, which `tsv_lang` owns
//! (`tsv_lang::doc::arena::with_doc_arena`: take it, reset it, run, park it back uncapped),
//! so the bindings and the language crates' free `format` functions share one slot per
//! thread. Its reuse rationale, its take/park protocol (why no borrow guard is held across a
//! call, the WASM trap case) and its soundness contract — nothing borrowed from the arena
//! outlives the next call's reset, so a call returns an owned `String` — are on that module.
//!
//! No parse takes an arena: every language parses into stores of its own, which each
//! language crate parks per thread itself (`tsv_ts::__private::with_stores`).
//!
//! # The tape
//!
//! Behind the `tape` feature, `parse_tape!` writes the binary parse wire (`docs/tape_format.md`)
//! with the language crate's own wire writer — its tape copy, `__private::write_tape` — into the
//! thread's parked `tsv_lang::tape::TapeSink` (`tape::with_tape_sink`), and lends the binding the
//! finished words.

#[cfg(feature = "tape")]
pub mod tape;

/// `tsv_lang`, re-exported for [`parse_format!`]'s expansion alone: a macro's paths
/// resolve in the *caller's* crate, so without this every binding would need its own
/// `tsv_lang` edge just to spell the line-terminator fold and the doc-arena slot. Not API.
#[cfg(feature = "format")]
#[doc(hidden)]
pub use tsv_lang as __tsv_lang;

/// The text a binding reports a caught panic with: `panic: <payload>`, the payload a
/// `&str` or a `String` (what `panic!` carries), `panic: <unknown>` for any other
/// (`std::panic::panic_any` with another type).
///
/// The two native bindings catch a panic where it crosses into the host, under their
/// unwinding profiles: `tsv_ffi` returns this text as its error payload and `tsv_napi`
/// throws it as the JS error's message. One spelling, so a caller that classifies a
/// failure as a crash (the corpus tools' panic gate) reads the same prefix whichever
/// binding it loaded.
pub fn format_panic(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("panic: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("panic: {s}")
    } else {
        "panic: <unknown>".to_owned()
    }
}

//
// The goal axis
//
// The second thing all three bindings would otherwise hand-sync. Each one
// spells the parse goal in its host's idiom — `tsv_ffi` a `u32` code, `tsv_napi`
// and `tsv_wasm` a trailing optional string — but the axis underneath is one
// question asked three times, so it is answered once here. The macros carry no
// arena; they live beside the helpers because this crate is where the bindings'
// shared substrate goes rather than in any one of them (see the crate's
// CLAUDE.md §Why this crate exists).

/// Which export family a binding call belongs to — the one fact that decides what an
/// **unset** source type means (a parse reads it as `Module`, a format, a strip or an
/// imports parse as "none named", the module-then-script fallback), and the noun every
/// source-type refusal names.
///
/// One type for all three bindings' decoders: `tsv_ffi`'s `ffi_source_type` reads it to
/// decide whether its unspecified code is accepted (format only), and the two string-axis
/// bindings pass it through [`decode_source_type`] for the refusal's wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `parse_*` / `parse_tape_*` / `parse_internal_*` — a wire whose
    /// `Program.sourceType` one settled grammar must produce.
    Parse,
    /// `format_*` — whose source type shapes only the parse the formatter runs.
    Format,
    /// `strip_typescript` — whose source type, like a format's, shapes only the parse the
    /// strip runs.
    Strip,
    /// `parse_imports_*` — whose source type, like a strip's, shapes only the parse the
    /// collection runs: its records carry no `sourceType`, so unset is the fallback.
    Imports,
}

impl Family {
    /// The family's name as the refusals spell it: `parse` / `format` / `strip` / `imports`.
    pub const fn noun(self) -> &'static str {
        match self {
            Family::Parse => "parse",
            Family::Format => "format",
            Family::Strip => "strip",
            Family::Imports => "imports",
        }
    }
}

/// Decode a string-spelled source type — `tsv_wasm` and `tsv_napi`'s trailing optional
/// argument — into the language's goal; `None` (omitted, `undefined`, `null`) stays
/// **unset**, and what that means is the `family`'s (a parse reads it as `Module`, a
/// format, a strip or an imports parse as "none named").
///
/// `allowed` is the language's goal axis ([`goal_allowed!`]): a source type named on a
/// language with none (Svelte hard-wires `Module`; CSS has no goal) is refused rather
/// than ignored, `"module"` included, so a caller cannot believe it selected a goal that
/// was silently dropped; the refusal names the export `family`. A value naming neither
/// goal is refused with the value echoed. `parse_goal` is the language crate's own
/// spelling table (`tsv_ts::Goal`'s `FromStr`), passed in so this crate depends on no
/// language crate.
///
/// One spelling of both refusals for the two string-axis bindings — the npm facade both
/// package sets publish through (`crates/tsv_wasm/npm/api.js`) restates them by hand in
/// JS, the one copy this crate cannot reach — so a caller reads the same text past the
/// facade or through it, and swapping `@fuzdev/tsv-wasm` for `@fuzdev/tsv` changes
/// nothing. The C FFI spells the axis as a code, not a string, and words its own; the
/// CLI's `--source-type` spells the flag instead of the key and keeps its own copy.
///
/// # Errors
///
/// The refusal message, for a source type on a goalless language or one naming neither
/// goal.
pub fn decode_source_type<G>(
    source_type: Option<&str>,
    allowed: bool,
    family: Family,
    parse_goal: impl FnOnce(&str) -> Option<G>,
) -> Result<Option<G>, String> {
    let Some(source_type) = source_type else {
        return Ok(None);
    };
    if !allowed {
        return Err(format!(
            "{} option 'sourceType' is only supported for TypeScript",
            family.noun()
        ));
    }
    parse_goal(source_type).map(Some).ok_or_else(|| {
        format!("invalid sourceType '{source_type}' (expected 'script' or 'module')")
    })
}

/// The per-language parse call behind each binding's uniform exports.
///
/// The `$goalness` tag is the language's goal axis, fixed at the
/// `lang_bindings!` invocation: `goal` (TypeScript) threads the decoded goal
/// into `__private::parse_with_goal_into`; `nogoal` (Svelte, CSS) ignores it and calls
/// `__private::parse_into` — a set goal is already rejected by the binding's own goal
/// decoder, which reads the same tag through [`goal_allowed!`].
///
/// `$lang` resolves in the caller's scope, so this crate needs no dependency on the
/// language crates. Every language parses into the stores [`with_lang_stores!`] hands over as
/// `$stores`.
///
/// ```ignore
/// let ast = parse_ast!(goal, tsv_ts, source, goal, stores)?;
/// ```
#[macro_export]
macro_rules! parse_ast {
    (goal, $lang:ident, $source:expr, $goal:expr, $stores:expr) => {{ $lang::__private::parse_with_goal_into($source, $goal, $stores) }};
    (nogoal, $lang:ident, $source:expr, $goal:expr, $stores:expr) => {{
        // Consume the (always-`Module`) goal so the binding stays used in every
        // expansion.
        let _ = $goal;
        $lang::__private::parse_into($source, $stores)
    }};
}

/// Run `$body` with `$stores` bound to this thread's parked stores for the language, taken
/// and parked through its own `__private::with_stores` (Svelte's takes a TypeScript half
/// and a CSS half from those crates' slots, and its own half from its own). `tsv_arena`
/// cannot park them itself, since it names no language crate's types.
#[macro_export]
#[doc(hidden)]
macro_rules! with_lang_stores {
    ($lang:ident, |$stores:ident| $body:expr) => {
        $lang::__private::with_stores(|$stores| $body)
    };
}

/// What a language's reading entry points take for a tree `$ast` a warm parse left in
/// `$stores`: the tree and the stores it is read through.
#[macro_export]
#[doc(hidden)]
macro_rules! read_ast {
    ($lang:ident, $ast:ident, $stores:ident) => {
        $lang::__private::parsed_ref(&$ast, $stores)
    };
}

/// A binding's whole **JSON parse** export body — the C FFI's `tsv_parse_<lang>`; the two JS
/// bindings emit no JSON wire: parse `$source` through [`parse_ast!`] into the per-thread stores
/// and convert it with the language's `__private::$convert` writer (`convert_ast_json_bytes`), a
/// parse error mapped through `$map_err`, the binding's own error type.
///
/// `$goal` is the binding's decoded `Option` goal, and an unset one is read here as the
/// grammar's default, `Module` (`Goal::default()`): a parse has no fallback to run, since
/// its wire carries `Program.sourceType`, a claim one settled grammar has to produce.
/// The parse reads the caller's own bytes — no line-terminator fold, unlike
/// `parse_format!` — because the wire's offsets are a drop-in contract over them.
///
/// Expands to `Result<_, _>` inside the caller's context, so the caller's `?`-conversion
/// rules apply; `$lang` resolves in the caller's scope, as in [`parse_ast!`].
///
/// ```ignore
/// parse_convert!(goal, tsv_ts, convert_ast_json_string, source, goal, |e| format!("{e:#}"))
/// ```
#[macro_export]
macro_rules! parse_convert {
    ($goalness:ident, $lang:ident, $convert:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let source = $source;
        let goal = $goal.unwrap_or_default();
        $crate::with_lang_stores!($lang, |stores| {
            let ast =
                $crate::parse_ast!($goalness, $lang, source, goal, stores).map_err($map_err)?;
            let parsed = $crate::read_ast!($lang, ast, stores);
            Ok($lang::__private::$convert(parsed, source))
        })
    }};
}

/// [`parse_convert!`] without the convert: the benchmark-only `parse_internal_*` body,
/// which parses and throws the AST away so the timing isolates the parse from the
/// convert/serialize layers. Expands to `Result<(), _>`; the goal reads as
/// [`parse_convert!`]'s does.
///
/// `black_box(&ast)` sits *inside* the stores closure on purpose — that is the only scope
/// where the AST still exists, so nothing further out can stand in for it. Drop it and
/// the whole parse becomes dead code the optimizer may delete, leaving a benchmark that
/// got faster by measuring nothing.
///
/// ```ignore
/// parse_internal!(goal, tsv_ts, source, goal, |e| format!("{e:#}"))
/// ```
#[macro_export]
macro_rules! parse_internal {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let source = $source;
        let goal = $goal.unwrap_or_default();
        $crate::with_lang_stores!($lang, |stores| {
            let ast =
                $crate::parse_ast!($goalness, $lang, source, goal, stores).map_err($map_err)?;
            ::std::hint::black_box(&ast);
            Ok(())
        })
    }};
}

/// A binding's whole **tape** export body: parse through [`parse_ast!`] into the per-thread
/// stores and write the span-only wire's tape (`docs/tape_format.md`) with the language's own
/// wire writer — its tape copy, `$lang::__private::write_tape`, the same walk that writes the
/// JSON wire — into this thread's parked sink ([`tape::with_tape_sink`]), finished as words
/// (`TapeSink::finish_words`): the binary parse wire a JS host decodes to the tree `JSON.parse`
/// makes of the JSON wire. Its strings are true-offset refs into the text the wire's offsets
/// index (a Svelte or CSS document less its leading byte-order mark — the writer's sink skips
/// it).
///
/// The words are the sink's own buffer, laid out where they were written, lent to `$take` —
/// `|words: &[u32], len: usize|`, `len` the tape's length in bytes — which takes what the
/// binding hands its host: a copy it gives away whole, or the address a view over the parked
/// buffer reads (valid until the thread's next tape). A parse error goes through `$map_err`,
/// the goal read as [`parse_convert!`] reads it (unset is `Module`; no fallback, no fold); a
/// sink error (`tsv_lang::tape::SinkError`), through `$map_tape_err` — a tape the writers
/// write is refused only on size (a source too long for a string ref's signed offset), or for
/// a writer bug the sink caught. Expands to `Result<R, _>`, `R` what `$take` returns. Behind
/// the `tape` feature.
///
/// ```ignore
/// parse_tape!(
///     goal,
///     tsv_ts,
///     source,
///     goal,
///     |words: &[u32], _len: usize| words.to_vec(),
///     |e| Failure::parse(&e),
///     |e| internal(e)
/// )
/// ```
#[cfg(feature = "tape")]
#[macro_export]
macro_rules! parse_tape {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr, $take:expr, $map_err:expr, $map_tape_err:expr) => {{
        let source: &str = $source;
        let goal = $goal.unwrap_or_default();
        $crate::with_lang_stores!($lang, |stores| {
            let ast =
                $crate::parse_ast!($goalness, $lang, source, goal, stores).map_err($map_err)?;
            let parsed = $crate::read_ast!($lang, ast, stores);
            $crate::tape::with_tape_sink(|sink| {
                $lang::__private::write_tape(parsed, source, sink);
                sink.finish_words().map(|(words, len)| ($take)(words, len))
            })
            .map_err($map_tape_err)
        })
    }};
}

/// [`parse_ast!`]'s **format-path** twin, over the folded document
/// (`tsv_lang::printing::FoldedSource`): it calls the language's `parse_folded`, which maps
/// a parse error back onto the caller's own source — and, for Svelte, refuses the one lone
/// `<CR>` the fold cannot rewrite without changing meaning (`tsv_svelte::__private::parse_folded`).
/// The goal arrives as an `Option`, and an unset one means "no source type named" rather
/// than `Module`.
///
/// Every binding's format export reads its source type the same way — a caller
/// that names one gets exactly that grammar, a caller that names none gets
/// `tsv_ts::__private::parse_with_goal_or_fallback_into`'s module-then-script retry, which is what
/// lets `tsv format <path>` and an editor's bare `format_typescript(source)` format
/// a legacy sloppy script. The parse exports keep [`parse_ast!`], whose goal is a
/// settled `Goal`: their product is a wire carrying `Program.sourceType`, a claim
/// no retry may make depend on the input.
///
/// `nogoal` ignores the option exactly as [`parse_ast!`] ignores the goal — Svelte
/// hard-wires `Module` and CSS has no goal axis, so neither has a fallback to run.
///
/// ```ignore
/// let ast = parse_ast_for_format!(goal, tsv_ts, &folded, source_type, stores)?;
/// ```
#[macro_export]
macro_rules! parse_ast_for_format {
    (goal, $lang:ident, $folded:expr, $goal:expr, $stores:expr) => {{ $lang::__private::parse_folded_into($folded, $goal, $stores) }};
    (nogoal, $lang:ident, $folded:expr, $goal:expr, $stores:expr) => {{
        // Consume the (always-unset) goal so the binding stays used in every
        // expansion.
        let _ = $goal;
        $lang::__private::parse_folded_into($folded, $stores)
    }};
}

/// The whole format export: fold the source's line terminators, parse the folded text
/// through [`parse_ast_for_format!`] into the per-thread stores, and format it in the
/// per-thread doc arena — a parse error mapped back onto the caller's source
/// (`tsv_lang::printing::FoldedSource::parse_with`) and then through `$map_err`, each
/// binding's own error type (a message string, a located failure, a JS value). A Svelte
/// refusal (`tsv_lang::Error::refusal`) takes the same road; a binding that reports a
/// span reads the refusal's own (`Error::to_report`, `kind` `Refusal`) and marks it a refusal,
/// never a parse failure.
///
/// Expands to `Result<String, _>` inside the caller's context, so the
/// caller's `?`-conversion rules apply; `$lang` resolves in the caller's scope, as in
/// [`parse_ast!`], while `tsv_lang` is reached through this crate (`$crate::__tsv_lang`),
/// so a binding needs no `tsv_lang` edge of its own. The parse exports skip the fold: their wire's offsets are
/// a drop-in contract over the author's own bytes.
///
/// ```ignore
/// parse_format!(goal, tsv_ts, source, source_type, |e| format!("{e:#}"))
/// ```
#[cfg(feature = "format")]
#[macro_export]
macro_rules! parse_format {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let folded = $crate::__tsv_lang::printing::normalize_carriage_returns($source);
        $crate::with_lang_stores!($lang, |stores| {
            let ast = $crate::parse_ast_for_format!($goalness, $lang, &folded, $goal, stores)
                .map_err($map_err)?;
            let parsed = $crate::read_ast!($lang, ast, stores);
            Ok($crate::__tsv_lang::doc::arena::with_doc_arena(
                |doc_arena| $lang::__private::format_folded_in(parsed, &folded, doc_arena),
            ))
        })
    }};
}

/// A binding's whole **imports** export body: collect the import records of `$source` through
/// the language's `__private::parse_imports_into` into the per-thread stores, and write them
/// as the JSON every binding returns — `tsv_ts::__private::write_imports_json_string`, the one
/// writer every language's records go through, in the language's own `WIRE_COORDINATES` — a
/// parse error mapped through `$map_err`, the binding's own error type.
///
/// `$goal` is the binding's decoded `Option` goal, read as the strip reads it: `goal`
/// (TypeScript) hands it on, so an unset one is the module-then-script fallback; `nogoal`
/// (Svelte, whose scripts are modules) ignores it — a set goal is already refused by the
/// binding's own decoder ([`goal_allowed!`]). The source is the caller's own bytes, with no
/// line-terminator fold, as a parse export's: the records' offsets index them.
///
/// Expands to `Result<String, _>`, the error `$map_err`'s; `$lang` and `tsv_ts` resolve in
/// the caller's scope, as in [`parse_ast!`].
///
/// ```ignore
/// parse_imports_json!(goal, tsv_ts, source, goal, |e| Failure::parse(&e))
/// ```
#[macro_export]
macro_rules! parse_imports_json {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let source = $source;
        let goal = $goal;
        $crate::with_lang_stores!($lang, |stores| {
            $crate::parse_imports_json!(@collect $goalness, $lang, source, goal, stores)
                .map(|imports| {
                    tsv_ts::__private::write_imports_json_string(
                        &imports,
                        source,
                        $lang::WIRE_COORDINATES,
                    )
                })
                .map_err($map_err)
        })
    }};
    (@collect goal, $lang:ident, $source:expr, $goal:expr, $stores:expr) => {{
        $lang::__private::parse_imports_into($source, $goal, $stores)
    }};
    (@collect nogoal, $lang:ident, $source:expr, $goal:expr, $stores:expr) => {{
        // Consume the (always-unset) goal so the binding stays used in every expansion.
        let _ = $goal;
        $lang::__private::parse_imports_into($source, $stores)
    }};
}

/// Whether the binding's goal decoder accepts a set goal for this language —
/// the same `$goalness` tag [`parse_ast!`] reads, so the two can never disagree
/// about which languages have a goal axis.
///
/// A language with no axis **rejects** a set goal rather than ignoring it, so a
/// caller cannot believe it selected one that was silently dropped. Each binding
/// spells the refusal in its host's idiom; this macro only says which languages
/// owe one.
#[macro_export]
macro_rules! goal_allowed {
    (goal) => {
        true
    };
    (nogoal) => {
        false
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // The goal axis: one `$goalness` tag driving two macros, tested here rather
    // than three times over in the bindings. A stand-in language module gives
    // `parse_ast!` the hidden entry points every language crate exposes, so the
    // DISPATCH is pinned without this crate depending on any of them.

    mod fake_lang {
        pub(super) mod __private {
            /// The goal-axis language parses into its stores alone.
            pub(crate) fn parse_with_goal_into(source: &str, goal: &str, stores: &str) -> String {
                format!("parse_with_goal_into({source}, {goal}, {stores})")
            }
            /// The goal-axis language's format-path parse (`tsv_ts::__private::parse_folded`).
            pub(crate) fn parse_folded_into(
                folded: &str,
                goal: Option<&str>,
                stores: &str,
            ) -> String {
                match goal {
                    Some(goal) => format!("exact({folded}, {goal}, {stores})"),
                    None => format!("fallback({folded}, {stores})"),
                }
            }
        }
    }

    /// A goalless language (Svelte, CSS): its entry points hold the stores alone and take no
    /// goal at all.
    mod fake_goalless_lang {
        pub(super) mod __private {
            pub(crate) fn parse_into(source: &str, stores: &str) -> String {
                format!("parse_into({source}, {stores})")
            }
            pub(crate) fn parse_folded_into(folded: &str, stores: &str) -> String {
                format!("parse_folded_into({folded}, {stores})")
            }
        }
    }

    #[test]
    fn parse_ast_dispatches_on_the_goalness_tag() {
        assert_eq!(
            parse_ast!(goal, fake_lang, "src", "script", "stores"),
            "parse_with_goal_into(src, script, stores)",
            "`goal` must thread the goal into `parse_with_goal_into`"
        );
        assert_eq!(
            parse_ast!(nogoal, fake_goalless_lang, "src", "script", "stores"),
            "parse_into(src, stores)",
            "`nogoal` must drop the goal and take the goalless entry point"
        );
    }

    #[test]
    fn parse_ast_for_format_dispatches_on_the_goalness_tag() {
        assert_eq!(
            parse_ast_for_format!(goal, fake_lang, "folded", Some("script"), "stores"),
            "exact(folded, script, stores)",
            "a NAMED source type must be exact — the same grammar `parse_ast!` runs"
        );
        assert_eq!(
            parse_ast_for_format!(goal, fake_lang, "folded", None, "stores"),
            "fallback(folded, stores)",
            "an unset source type must take the module-then-script fallback"
        );
        assert_eq!(
            parse_ast_for_format!(nogoal, fake_goalless_lang, "folded", None::<&str>, "stores"),
            "parse_folded_into(folded, stores)",
            "`nogoal` must drop the option and take the goalless entry point"
        );
    }

    /// A language whose parse can fail, for the export-body macros: its goal type is `&str`,
    /// whose `Default` (`""`) stands in for `Goal::Module`.
    mod fake_fallible_lang {
        pub(super) mod __private {
            /// What `with_lang_stores!` calls: hands over one marker.
            pub(crate) fn with_stores<R>(f: impl FnOnce(&str) -> R) -> R {
                f("marker")
            }
            /// The goalless language's parse.
            pub(crate) fn parse_into(source: &str, stores: &str) -> Result<String, String> {
                parse_with_goal_into(source, "goalless", stores)
            }
            pub(crate) fn parse_with_goal_into(
                source: &str,
                goal: &str,
                stores: &str,
            ) -> Result<String, String> {
                debug_assert_eq!(stores, "marker");
                if source == "bad" {
                    Err(format!("bad source at {goal:?}"))
                } else {
                    Ok(format!("ast({source}, {goal:?})"))
                }
            }
            /// What `read_ast!` calls: the tree, the stores spent.
            pub(crate) fn parsed_ref<'a>(ast: &'a String, _stores: &str) -> &'a String {
                ast
            }
            pub(crate) fn convert(ast: &String, source: &str) -> String {
                format!("convert({ast}, {source})")
            }
        }
    }

    #[test]
    fn parse_convert_parses_converts_and_maps_the_error() {
        let run = |source: &str, goal: Option<&str>| -> Result<String, String> {
            parse_convert!(
                goal,
                fake_fallible_lang,
                convert,
                source,
                goal,
                |e| format!("mapped {e}")
            )
        };
        assert_eq!(
            run("src", Some("script")),
            Ok(r#"convert(ast(src, "script"), src)"#.to_owned()),
            "a named goal threads through"
        );
        assert_eq!(
            run("src", None),
            Ok(r#"convert(ast(src, ""), src)"#.to_owned()),
            "an unset goal reads as the goal type's default (`Goal::Module`)"
        );
        assert_eq!(
            run("bad", None),
            Err(r#"mapped bad source at """#.to_owned()),
            "a parse error goes through `$map_err`"
        );
        let goalless: Result<String, String> = parse_convert!(
            nogoal,
            fake_fallible_lang,
            convert,
            "src",
            None::<&str>,
            |e| e
        );
        assert_eq!(
            goalless,
            Ok(r#"convert(ast(src, "goalless"), src)"#.to_owned()),
            "`nogoal` takes the goalless entry point, with the stores `with_stores` hands over"
        );
    }

    #[test]
    fn parse_internal_parses_and_returns_nothing() {
        let run = |source: &str| -> Result<(), String> {
            parse_internal!(goal, fake_fallible_lang, source, None::<&str>, |e| e)
        };
        assert_eq!(run("src"), Ok(()));
        assert_eq!(run("bad"), Err(r#"bad source at """#.to_owned()));
    }

    #[test]
    fn decode_source_type_decodes_and_words_every_refusal() {
        let from = |s: &str| match s {
            "module" => Some("M"),
            "script" => Some("S"),
            _ => None,
        };
        assert_eq!(
            decode_source_type(None, true, Family::Parse, from),
            Ok(None)
        );
        assert_eq!(
            decode_source_type(None, false, Family::Format, from),
            Ok(None)
        );
        assert_eq!(
            decode_source_type(Some("script"), true, Family::Parse, from),
            Ok(Some("S"))
        );
        assert_eq!(
            decode_source_type(Some("module"), true, Family::Format, from),
            Ok(Some("M"))
        );
        // a goalless language refuses the axis, `"module"` included, ahead of the value
        for (family, noun) in [
            (Family::Parse, "parse"),
            (Family::Format, "format"),
            (Family::Strip, "strip"),
            (Family::Imports, "imports"),
        ] {
            assert_eq!(family.noun(), noun);
            for value in ["script", "module", "sloppy"] {
                assert_eq!(
                    decode_source_type(Some(value), false, family, from),
                    Err(format!(
                        "{noun} option 'sourceType' is only supported for TypeScript"
                    ))
                );
            }
        }
        assert_eq!(
            decode_source_type(Some("sloppy"), true, Family::Parse, from),
            Err("invalid sourceType 'sloppy' (expected 'script' or 'module')".to_owned())
        );
        // the empty string is a string — refused by value
        assert_eq!(
            decode_source_type(Some(""), true, Family::Parse, from),
            Err("invalid sourceType '' (expected 'script' or 'module')".to_owned())
        );
    }

    #[test]
    fn format_panic_renders_payload_variants() {
        use std::any::Any;
        let as_str: Box<dyn Any + Send> = Box::new("boom");
        assert_eq!(format_panic(&*as_str), "panic: boom");
        let owned: Box<dyn Any + Send> = Box::new(String::from("kaboom"));
        assert_eq!(format_panic(&*owned), "panic: kaboom");
        let other: Box<dyn Any + Send> = Box::new(42i32);
        assert_eq!(format_panic(&*other), "panic: <unknown>");
    }

    #[test]
    fn goal_allowed_matches_the_same_tag() {
        // Bound to locals first: the macro expands to a literal, and an
        // `assert!(true)` is a clippy error under the workspace's `-D warnings`.
        let has_axis: bool = goal_allowed!(goal);
        let no_axis: bool = goal_allowed!(nogoal);
        assert!(has_axis, "TypeScript has a goal axis");
        assert!(!no_axis, "Svelte and CSS have none");
    }
}
