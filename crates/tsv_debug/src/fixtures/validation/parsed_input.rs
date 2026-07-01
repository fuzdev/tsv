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
    /// FFI/WASM/CLI-compact ship, with its typed comment attach and multibyte
    /// offset translation) is byte-identical to the `Value`
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

/// Svelte counterpart of `TYPED_WALK_SYNTH_PREFIX` — a multibyte HTML comment
/// (a `//` line comment isn't valid Svelte markup). It parses as an inert
/// leading `Comment` fragment node, shifting every downstream byte offset;
/// when the fixture opens with `<script>`/`<style>` it additionally becomes
/// that tag's preceding HTML comment (injected `leadingComments`), which is
/// fine — the probe compares the two paths on the same content, not against
/// `expected.json`.
const SVELTE_TYPED_WALK_SYNTH_PREFIX: &str = "<!-- 中文😀 -->\n";

/// Appended expression tag that forces the island-scoped template-comment
/// attach pass (`attach_template_expression_comments_typed`) to run on the
/// fixture's full AST shape and compares it against the `Value` dispatcher.
/// The three comments exercise leading attachment, the walk's same-line
/// trailing attachment, and the DFS's island-root trailing special case (the
/// second trailing comment survives the walk and lands via the root-node
/// path); the multibyte content makes the resulting `Attached` island go
/// through the typed translation walk's `Value`-island delegation too.
const SVELTE_TEMPLATE_COMMENT_SYNTH_SUFFIX: &str = "\n{/* 中文😀 */ probe /* t1 */ /* 😀 t2 */}";

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

/// Probe the typed walks for parity with their `Value` counterparts, beyond
/// what the fixture's own content exercises.
///
/// The typed translation walks (`translate_byte_to_char_offsets_typed` in
/// tsv_ts, tsv_css, and tsv_svelte) enumerate struct fields manually, so a
/// position-bearing field missing from one stays green on every ASCII fixture
/// (translation is a no-op on both paths); tsv_svelte's typed attach
/// dispatcher (`attach_typed.rs`) likewise stays green on every fixture
/// without template-expression comments. These probes close those holes:
///
/// - `.ts` / `.svelte.ts` inputs get a synthesized multibyte variant (a
///   prepended multibyte comment shifts all downstream offsets). Inputs with
///   byte-0 features (hashbang, BOM) are skipped — prepending would change
///   their semantics.
/// - `.svelte` inputs get a synthesized multibyte variant of the whole file
///   (a prepended multibyte HTML comment), exercising `tsv_svelte`'s typed
///   walk — the Svelte nodes, `name_loc` positions, embedded expression/CSS
///   subtrees, and `Value` islands. They also get a synthesized
///   template-comment variant (an appended expression tag carrying multibyte
///   comments), exercising the island-scoped attach pass against the `Value`
///   dispatcher on every fixture's AST shape. Their `<script>` contents are
///   also extracted and run through `tsv_ts`'s two paths as standalone TS —
///   as-is when already multibyte, plus a synthesized multibyte variant — so
///   every embedded-TS AST shape gets `tsv_ts` typed-walk coverage too.
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
        (tsv_svelte, $content:expr, $description:expr) => {{
            let content: &str = $content;
            let arena = bumpalo::Bump::new();
            let result = tsv_svelte::parse(content, &arena).map(|ast| {
                (
                    tsv_svelte::convert_ast_json_string(&ast, content),
                    serde_json::to_string(&tsv_svelte::convert_ast_json(&ast, content))
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
            // Whole-file synthesized multibyte variant: exercises
            // tsv_svelte's own typed walk on the fixture's full AST shape.
            // Byte-0 BOM can't take a prepended comment.
            if !content.starts_with('\u{feff}') {
                let synthesized = format!("{SVELTE_TYPED_WALK_SYNTH_PREFIX}{content}");
                probe!(tsv_svelte, &synthesized, "synthesized multibyte input");
            }
            // Template-comment variant: appending is byte-0-safe, and a
            // parsed document always accepts a trailing top-level tag.
            let synthesized = format!("{content}{SVELTE_TEMPLATE_COMMENT_SYNTH_SUFFIX}");
            probe!(
                tsv_svelte,
                &synthesized,
                "synthesized template-comment input"
            );
            for (i, (start, end)) in tsv_svelte::script_content_spans(root)
                .into_iter()
                .enumerate()
            {
                let script = &content[start as usize..end as usize];
                if !script.is_ascii() {
                    // A multibyte script embedded in Svelte goes through the
                    // Svelte typed walk's Value island for `Script.content`;
                    // this standalone-TS run is the only coverage the same
                    // content gets on tsv_ts's own typed walk. Svelte
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
