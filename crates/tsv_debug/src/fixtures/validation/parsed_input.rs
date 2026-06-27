//! Shared parse of a fixture input and the wire-path/typed-walk probes
//! consumed by the parser-side validation phases (2/2b/2c/2d).

use crate::fixtures::InputType;
use tsv_cli::json_utils::to_json_with_tabs;

/// A fixture input parsed once with our parser.
///
/// The parser-side validation phases (expected.json comparison, wire-path
/// identity, typed-walk parity probes) all need the same AST — sharing one
/// parse keeps `fixtures_validate` from re-parsing every fixture per phase.
pub(super) enum ParsedInput<'arena> {
    Svelte(tsv_svelte::Root<'arena>),
    Ts(tsv_ts::Program<'arena>),
    Css(tsv_css::CssStyleSheet<'arena>),
}

/// Parse fixture content once for the parser-side validation phases.
///
/// `arena` owns the internal AST and must outlive the returned `ParsedInput`
/// (caller-owns-`Bump`).
pub(super) fn parse_input<'arena>(
    content: &str,
    input_type: InputType,
    goal: tsv_ts::Goal,
    arena: &'arena bumpalo::Bump,
) -> Result<ParsedInput<'arena>, String> {
    match input_type {
        InputType::Svelte => tsv_svelte::parse(content, arena)
            .map(ParsedInput::Svelte)
            .map_err(|e| format!("Parse error: {e:?}")),
        InputType::SvelteTs | InputType::TypeScript => {
            tsv_ts::parse_with_goal(content, goal, arena)
                .map(ParsedInput::Ts)
                .map_err(|e| format!("Parse error: {e:?}"))
        }
        InputType::Css => tsv_css::parse(content, arena)
            .map(ParsedInput::Css)
            .map_err(|e| format!("Parse error: {e:?}")),
    }
}

/// Both JSON-AST outputs derived from one `convert_ast_json` call.
pub(super) struct InputAstPaths {
    /// `convert_ast_json`'s `Value` (for semantic comparison against
    /// `expected.json`, ignoring key-order differences).
    pub ast_json: serde_json::Value,
    /// The same `Value` serialized with tabs + trailing newline — the exact
    /// bytes `expected*.json` files store (matches `fixtures_update_parsed`).
    pub ast_json_tabs: String,
    /// Whether the compact wire path (`convert_ast_json_string` — what
    /// FFI/WASM/CLI-compact ship, with its own fast-path eligibility gates
    /// and multibyte offset translation) is byte-identical to the `Value`
    /// path. The expected.json comparisons go through `convert_ast_json`,
    /// so without this check the shipped path would be fixture-blind.
    pub wire_path_matches: bool,
}

/// Compute the `Value`-path AST and the wire-path identity check from an
/// already-parsed input, materializing `convert_ast_json` once.
#[allow(clippy::expect_used)] // Value serialization cannot fail
pub(super) fn input_ast_paths(
    parsed: &ParsedInput<'_>,
    content: &str,
) -> Result<InputAstPaths, String> {
    let (ast_json, wire) = match parsed {
        ParsedInput::Svelte(ast) => (
            tsv_svelte::convert_ast_json(ast, content),
            tsv_svelte::convert_ast_json_string(ast, content),
        ),
        ParsedInput::Ts(ast) => (
            tsv_ts::convert_ast_json(ast, content),
            tsv_ts::convert_ast_json_string(ast, content),
        ),
        ParsedInput::Css(ast) => (
            tsv_css::convert_ast_json(ast, content),
            tsv_css::convert_ast_json_string(ast, content),
        ),
    };
    let tabs = to_json_with_tabs(&ast_json)
        .map_err(|e| format!("Failed to serialize AST to JSON: {e}"))?;
    let value_compact = serde_json::to_string(&ast_json).expect("Value serialization cannot fail");
    Ok(InputAstPaths {
        ast_json,
        // Trailing newline matches the fixtures_update_parsed format
        ast_json_tabs: format!("{tabs}\n"),
        wire_path_matches: wire == value_compact,
    })
}

/// Multibyte comment prepended to synthesize probe variants — shifts every
/// downstream byte offset away from its UTF-16 offset, so the typed
/// offset-translation walk is exercised on the whole AST shape.
const TYPED_WALK_SYNTH_PREFIX: &str = "// 中文😀\n";

/// CSS counterpart of `TYPED_WALK_SYNTH_PREFIX` — a multibyte block comment (a
/// `//` line comment isn't valid CSS). Top-level CSS comments live on
/// `CssStyleSheet.comments`, not in `nodes`, so this shifts every node offset
/// without changing the AST shape.
const CSS_TYPED_WALK_SYNTH_PREFIX: &str = "/* 中文😀 */\n";

/// How a typed-walk parity probe failed.
#[derive(Debug)]
pub(super) enum TypedWalkParityFailure {
    /// The probe content failed to parse. This is an error (not a skip): a
    /// silently dropped probe would reopen the coverage hole the probes exist
    /// to close.
    Parse(String),
    /// `convert_ast_json_string` differs from the `Value` path.
    Diverged,
}

/// Outcome of the typed-walk parity probes for one fixture input.
#[derive(Debug, Default)]
pub(super) struct TypedWalkParity {
    /// Probes that ran and matched.
    pub checked: usize,
    /// Failed probes: (probe description, failure).
    pub failures: Vec<(String, TypedWalkParityFailure)>,
}

/// Probe `tsv_ts`'s typed offset-translation walk for parity with the `Value`
/// walk, beyond what the fixture's own content exercises.
///
/// The typed walk (`translate_byte_to_char_offsets_typed`) enumerates struct
/// fields manually, so a position-bearing field missing from it stays green on
/// every ASCII fixture (translation is a no-op on both paths) and on every
/// multibyte `.svelte` fixture (Svelte's gate routes those to the `Value`
/// fallback). These probes close that hole:
///
/// - `.ts` / `.svelte.ts` inputs get a synthesized multibyte variant (a
///   prepended multibyte comment shifts all downstream offsets). Inputs with
///   byte-0 features (hashbang, BOM) are skipped — prepending would change
///   their semantics.
/// - `.svelte` inputs have their `<script>` contents extracted and run
///   through `tsv_ts`'s two paths as standalone TS — as-is when already
///   multibyte, plus a synthesized multibyte variant — so every AST shape in
///   the corpus gets typed-walk coverage, not just the few standalone-TS
///   fixtures.
///
/// Each probe asserts `convert_ast_json_string` is byte-identical to
/// `serde_json::to_string(&convert_ast_json(..))`. Probes are independent of
/// `expected.json`, so they don't affect parser conformance. `.css` inputs get
/// a synthesized multibyte variant (the standalone `.css` fixtures are ASCII, so
/// their own content never exercises the CSS typed walk's translation branch);
/// broad CSS coverage comes from the `corpus:compare:parse --multibyte-only`
/// gate. Takes the already-parsed input so `.svelte` script-span extraction
/// reuses the fixture's one parse.
#[allow(clippy::expect_used)] // Value serialization cannot fail
pub(super) fn typed_walk_parity_probes(
    content: &str,
    parsed: &ParsedInput<'_>,
    goal: tsv_ts::Goal,
) -> TypedWalkParity {
    let mut parity = TypedWalkParity::default();

    // Parse `$content` as standalone `$lang` and assert its
    // `convert_ast_json_string` is byte-identical to
    // `to_string(&convert_ast_json(..))`, recording the result onto `parity`.
    // TypeScript probes take an explicit goal (the top-level arm uses the
    // fixture's; Svelte's extracted `<script>` content is always `Module`); CSS
    // has no goal axis. The `@record` arm holds the shared bookkeeping tail.
    macro_rules! probe {
        (@record $description:expr, $result:expr) => {{
            let description = $description;
            match $result {
                Ok((string_path, value_path)) => {
                    if string_path == value_path {
                        parity.checked += 1;
                    } else {
                        parity
                            .failures
                            .push((description.to_string(), TypedWalkParityFailure::Diverged));
                    }
                }
                Err(e) => {
                    parity.failures.push((
                        description.to_string(),
                        TypedWalkParityFailure::Parse(format!("{e:?}")),
                    ));
                }
            }
        }};
        (tsv_ts @ $goal:expr, $content:expr, $description:expr) => {{
            let content: &str = $content;
            let arena = bumpalo::Bump::new();
            let result = tsv_ts::parse_with_goal(content, $goal, &arena).map(|ast| {
                (
                    tsv_ts::convert_ast_json_string(&ast, content),
                    serde_json::to_string(&tsv_ts::convert_ast_json(&ast, content))
                        .expect("Value serialization cannot fail"),
                )
            });
            probe!(@record $description, result);
        }};
        (tsv_css, $content:expr, $description:expr) => {{
            let content: &str = $content;
            let arena = bumpalo::Bump::new();
            let result = tsv_css::parse(content, &arena).map(|ast| {
                (
                    tsv_css::convert_ast_json_string(&ast, content),
                    serde_json::to_string(&tsv_css::convert_ast_json(&ast, content))
                        .expect("Value serialization cannot fail"),
                )
            });
            probe!(@record $description, result);
        }};
    }

    match parsed {
        ParsedInput::Ts(_) => {
            // Byte-0 features (hashbang, BOM) can't take a prepended comment
            if content.starts_with("#!") || content.starts_with('\u{feff}') {
                return parity;
            }
            // The as-is input is already covered by the string-path identity
            // check; only the synthesized multibyte variant is new coverage.
            // Parse at the fixture's goal so standalone-script fixtures probe at
            // Script (where `await` is an identifier, `import.meta` rejects, …).
            let synthesized = format!("{TYPED_WALK_SYNTH_PREFIX}{content}");
            probe!(tsv_ts @ goal, &synthesized, "synthesized multibyte input");
        }
        ParsedInput::Svelte(root) => {
            for (i, (start, end)) in tsv_svelte::script_content_spans(root)
                .into_iter()
                .enumerate()
            {
                let script = &content[start as usize..end as usize];
                if !script.is_ascii() {
                    // Multibyte .svelte inputs take the Value fallback in
                    // tsv_svelte, so this standalone-TS run is the only
                    // typed-walk coverage their script content gets. Svelte
                    // `<script>` is always a module.
                    probe!(tsv_ts @ tsv_ts::Goal::Module, script, &format!("extracted script {i} (as-is)"));
                }
                let synthesized = format!("{TYPED_WALK_SYNTH_PREFIX}{script}");
                probe!(
                    tsv_ts @ tsv_ts::Goal::Module,
                    &synthesized,
                    &format!("extracted script {i} (synthesized multibyte)")
                );
            }
        }
        ParsedInput::Css(_) => {
            // Byte-0 BOM can't take a prepended comment (would change its semantics).
            if content.starts_with('\u{feff}') {
                return parity;
            }
            // This probe assumes a leading block comment is parse-inert — true for
            // every standalone `.css` fixture today. It would NOT hold for an
            // order-sensitive leading at-rule (`@charset`/`@import`, which must be
            // first): prepending the comment would shift parse semantics and the
            // string-vs-Value comparison would no longer reflect the fixture's own
            // shape. If such a `.css` fixture is ever added, gate it out here (like
            // the BOM case) and rely on `corpus:compare:parse --multibyte-only` for
            // its multibyte coverage instead.
            let synthesized = format!("{CSS_TYPED_WALK_SYNTH_PREFIX}{content}");
            probe!(tsv_css, &synthesized, "synthesized multibyte input");
        }
    }

    parity
}
