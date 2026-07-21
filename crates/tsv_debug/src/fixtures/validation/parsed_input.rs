//! Shared parse of a fixture input and the JSON-AST it feeds the parser-side
//! validation phases (2/2b).

use crate::fixtures::InputType;
use tsv_cli::json_utils::to_json_with_tabs;

/// A fixture input parsed once with our parser.
///
/// The parser-side validation phases (expected.json comparison, the tabbed
/// serialization) all need the same AST — sharing one parse keeps
/// `fixtures_validate` from re-parsing every fixture per phase.
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
    interner: &mut tsv_lang::Interner,
) -> Result<ParsedInput<'arena>, String> {
    match input_type {
        InputType::Svelte => tsv_svelte::parse(content, arena, interner)
            .map(ParsedInput::Svelte)
            .map_err(|e| format!("Parse error: {e:?}")),
        InputType::SvelteTs | InputType::TypeScript => {
            tsv_ts::parse_with_goal(content, goal, arena, interner)
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
    /// `convert_ast_json`'s `Value` — used to classify a byte mismatch against
    /// `expected.json` as field-order-only (key-order-insensitive `Value`
    /// equality) vs semantic.
    pub ast_json: serde_json::Value,
    /// The same `Value` serialized with tabs + trailing newline — the exact
    /// bytes `expected*.json` files store (matches `fixtures_update_parsed`);
    /// the byte-strict comparison the parser phases gate on.
    pub ast_json_tabs: String,
}

/// Compute the JSON-AST for the parser-side phases from an already-parsed
/// input. `convert_ast_json` parses the wire bytes the writer emits (the sole
/// emission path); `expected.json` — pinned to the canonical parser by the P1/P3
/// freshness checks — is the oracle these phases compare against.
pub(super) fn input_ast_paths(
    parsed: &ParsedInput<'_>,
    content: &str,
    interner: &tsv_lang::Interner,
) -> Result<InputAstPaths, String> {
    let ast_json = match parsed {
        ParsedInput::Svelte(ast) => tsv_svelte::convert_ast_json(ast, content, interner),
        ParsedInput::Ts(ast) => tsv_ts::convert_ast_json(ast, content, interner),
        ParsedInput::Css(ast) => tsv_css::convert_ast_json(ast, content),
    };
    let tabs = to_json_with_tabs(&ast_json)
        .map_err(|e| format!("Failed to serialize AST to JSON: {e}"))?;
    Ok(InputAstPaths {
        ast_json,
        // Trailing newline matches the fixtures_update_parsed format
        ast_json_tabs: format!("{tabs}\n"),
    })
}
