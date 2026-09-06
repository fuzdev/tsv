//! Shared parse of a fixture input and the JSON-AST it feeds the parser-side
//! validation phases (2/2b).

use crate::fixtures::InputType;
use crate::json::wire_value;
use tsv_cli::json_utils::indent_json_with_tabs;

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

/// The parser phases' view of one input: the writer's wire and its tabbed form.
pub(super) struct InputAstPaths {
    /// The compact wire `convert_ast_json_bytes` emitted (the sole emission
    /// path). Read back into a `Value` only on a byte mismatch, to classify it
    /// as field-order-only vs semantic ([`Self::wire_value`]) — so the happy
    /// path deserializes nothing, like the CLI.
    pub wire: Vec<u8>,
    /// The same wire tab-indented + trailing newline — the exact bytes
    /// `expected*.json` files store (matches `fixtures_update_parsed`); the
    /// byte-strict comparison the parser phases gate on.
    pub ast_json_tabs: String,
}

impl InputAstPaths {
    /// The wire as a key-order-insensitive `Value`, through the unbounded reader.
    pub fn wire_value(&self) -> serde_json::Value {
        wire_value(&self.wire)
    }
}

/// Compute the JSON-AST for the parser-side phases from an already-parsed
/// input. The tabbed text derives from the wire through the CLI's
/// recursion-free re-indenter — the `--pretty` route, so the gate exercises it
/// on every fixture. `expected.json` — pinned to the canonical parser by the
/// P1/P3 freshness checks — is the oracle these phases compare against.
pub(super) fn input_ast_paths(parsed: &ParsedInput<'_>, content: &str) -> InputAstPaths {
    let wire = match parsed {
        ParsedInput::Svelte(ast) => tsv_svelte::convert_ast_json_bytes(ast, content),
        ParsedInput::Ts(ast) => tsv_ts::convert_ast_json_bytes(ast, content),
        ParsedInput::Css(ast) => tsv_css::convert_ast_json_bytes(ast, content),
    };
    let mut tabs = indent_json_with_tabs(&wire);
    // Trailing newline matches the fixtures_update_parsed format
    tabs.push(b'\n');
    #[expect(
        clippy::expect_used,
        reason = "the wire is UTF-8 by construction and re-indenting adds only ASCII"
    )]
    let ast_json_tabs = String::from_utf8(tabs).expect("re-indented wire is UTF-8");
    InputAstPaths {
        wire,
        ast_json_tabs,
    }
}
