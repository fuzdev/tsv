//! The writer's unit tests, over its JSON copy: mounted beside it (`mod.rs`) rather than inside
//! `write/`, whose tape copy would compile them against the tape.

use super::write::{Ctx, node_header, node_header_wide_end, write_identifier_parts};
use super::{CommentAttach, CommentMode, IslandComments, write_identifier_plain};
use crate::ast;
use tsv_lang::{JsonWriter, Span, Wire, WireTables};

/// The wire variants a test runs: both, unless the build lacks the loc-bearing one
/// (`tsv_lang`'s `_locations` feature, off in a bare `cargo test -p tsv_ts`).
const WIRES: &[Wire] = &[
    #[cfg(feature = "locations")]
    Wire::Loc,
    Wire::Span,
];

/// Run `emit` under a standalone-TypeScript context over `source`, for one
/// wire variant and one comment role.
fn emitted(
    source: &str,
    wire: Wire,
    comments: CommentMode<'_>,
    view: ast::AstView<'_>,
    emit: impl FnOnce(&mut JsonWriter, &Ctx<'_>),
) -> String {
    let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
    let ctx = Ctx::new(source, tables.positions(), comments, view);
    let mut w = JsonWriter::with_capacity(0);
    emit(&mut w, &ctx);
    String::from_utf8(w.into_bytes()).expect("the wire is UTF-8")
}

/// The sole statement of `source` as the `Identifier` its expression is.
fn sole_identifier<'a>(view: ast::AstView<'a>, program: &'a ast::Program) -> &'a ast::Identifier {
    let [statement] = view.stmt_list(program.body) else {
        panic!("one statement expected");
    };
    let ast::StatementKind::ExpressionStatement(statement) = &view.stmt(*statement).kind else {
        panic!("an expression statement expected");
    };
    let ast::ExpressionKind::Identifier(id) = &view.expr(statement.expression).kind else {
        panic!("an identifier expected");
    };
    id
}

/// The staged header against `node_header_wide_end`, which writes the same
/// header through the direct emitters — over a span whose two ends sit on
/// different lines behind a multibyte character, and at node-type lengths
/// either side of the staged run's inline copy (a real node type is at most
/// 31 bytes; a longer one takes the run's out-of-line copy).
#[test]
fn node_header_matches_the_direct_emitters() {
    const TYPE: &str = "TSConstructSignatureDeclarationAndThenSomeMore";
    let source = "é;\n\n  abc(\n);\n";
    let start = source.find("abc").expect("present") as u32;
    let span = Span::new(start, source.len() as u32 - 2);
    let stores = ast::AstStores::new();
    for len in [1, 31, 32, 33, TYPE.len()] {
        let node_type: &'static str = &TYPE[..len];
        for &wire in WIRES {
            let staged = emitted(source, wire, CommentMode::Off, stores.view(), |w, ctx| {
                node_header!(w, @dyn node_type => (), span, ctx);
            });
            let direct = emitted(source, wire, CommentMode::Off, stores.view(), |w, ctx| {
                node_header_wide_end(w, node_type, (), span, span.end, ctx);
            });
            assert_eq!(staged, direct, "type length {len}, {wire:?}");
            if wire != Wire::Span {
                assert_eq!(
                    staged,
                    format!(
                        "{{\"type\":\"{node_type}\",\"start\":6,\"end\":12,\"loc\":{{\"start\":\
                         {{\"line\":3,\"column\":2}},\"end\":{{\"line\":4,\"column\":1}}}}"
                    )
                );
            }
        }
    }
}

/// `write_identifier_plain`'s single-run arm against the general identifier
/// emission it stands in for, and against the bytes spelled out, at each edge
/// of its gate: the name length either side of `StageRun::SHORT_MAX`, a
/// non-ASCII name (whose byte length is what the gate reads) either side of
/// it too, a name longer than the whole staging scratch, and an escaped
/// name, which carries its decoded form.
#[test]
fn plain_identifier_run_matches_the_general_emission() {
    let ascii = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    // Two bytes and one UTF-16 unit a character.
    let wide = "éèêëàâäîïôöùûüçñ".repeat(2);
    let cases: Vec<(String, String)> = [
        &ascii[..1],
        &ascii[..2],
        &ascii[..7],
        &ascii[..8],
        &ascii[..16],
        &ascii[..31],
        &ascii[..32],
        &ascii[..33],
        &ascii[..40],
        "$",
        "_$1",
        &wide[..2],
        &wide[..30],
        &wide[..32],
        &wide[..34],
    ]
    .into_iter()
    .map(|name| (name.to_owned(), name.to_owned()))
    .chain([(ascii[..50].repeat(8), ascii[..50].repeat(8))])
    .chain([
        ("\\u0061bc".to_owned(), "abc".to_owned()),
        ("a\\u{62}".to_owned(), "ab".to_owned()),
    ])
    .collect();
    for (written, name) in &cases {
        let source = format!("\n  {written};\n");
        let program = crate::__private::parse(&source).expect("parses");
        let id = sole_identifier(program.view(), &program);
        let end = 3 + written.encode_utf16().count();
        let column = end - 1;
        for &wire in WIRES {
            let plain = emitted(&source, wire, CommentMode::Off, program.view(), |w, ctx| {
                write_identifier_plain(w, id, ctx);
            });
            let general = emitted(&source, wire, CommentMode::Off, program.view(), |w, ctx| {
                write_identifier_parts(w, id.span, id.ident_name(), false, None, None, ctx);
            });
            assert_eq!(plain, general, "{written:?}, {wire:?}");
            let loc = if wire != Wire::Span {
                format!(
                    ",\"loc\":{{\"start\":{{\"line\":2,\"column\":2}},\"end\":\
                     {{\"line\":2,\"column\":{column}}}}}"
                )
            } else {
                String::new()
            };
            assert_eq!(
                plain,
                format!(
                    "{{\"type\":\"Identifier\",\"start\":3,\"end\":{end}{loc},\
                     \"name\":\"{name}\"}}"
                ),
                "{written:?}, {wire:?}"
            );
        }
    }
}

/// Under a comment attach the identifier keeps the general emission: its open
/// and close are what hand it its comments, and the single-run arm reports
/// neither.
#[test]
fn plain_identifier_under_a_comment_attach_emits_its_comments() {
    let source = "/* c */ name";
    let mut stores = ast::AstStores::new();
    let (expression, comments) = crate::__private::parse_expression_with_comments(
        source,
        0,
        0,
        &mut stores,
        "end of input",
        |_, _| Ok(()),
    )
    .expect("parses");
    let ast::ExpressionKind::Identifier(id) = &stores.view().expr(expression).kind else {
        panic!("an identifier expected");
    };
    let attach = || {
        CommentAttach::new(
            source,
            IslandComments {
                queue: stores.view().comments(comments).iter().collect(),
                root_parent_end: None,
                root_fallback: true,
                html_leading: None,
                removes_parens: false,
                root_parens: Vec::new(),
            },
        )
    };
    for &wire in WIRES {
        let island = attach();
        let plain = emitted(source, wire, island.mode(), stores.view(), |w, ctx| {
            write_identifier_plain(w, id, ctx);
        });
        let island = attach();
        let general = emitted(source, wire, island.mode(), stores.view(), |w, ctx| {
            write_identifier_parts(w, id.span, id.ident_name(), false, None, None, ctx);
        });
        assert_eq!(plain, general, "{wire:?}");
        assert!(
            plain.contains(",\"name\":\"name\",\"leadingComments\":[{\"type\":\"Block\""),
            "the leading comment is emitted: {plain}"
        );
    }
}
