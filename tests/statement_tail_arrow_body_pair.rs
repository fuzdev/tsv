// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A redundant shell inside the pair an arrow adds around its conditional body reaches the
//! cataloged fixed point in one pass: the block stays inside the `??` operand's pair, and no
//! statement terminator takes it out past the arrow's pair
//! (`expressions/arrows/body_conditional_operand_paren_trailing_block_comment_prettier_divergence`,
//! which pins the fixed point and prettier's form of it).
//!
//! Not a variant of that fixture: prettier also reaches `input` from this authoring in one pass
//! (its own next pass moves the comment out), and a fixture whose `input` prettier rewrites
//! holds no `unformatted_*` variant.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

#[test]
fn a_redundant_shell_in_the_arrow_pair_keeps_its_comment() {
    let source = "x = () => c ? d : (a ?? (b /* c */));\n";
    let once = format(source);
    assert_eq!(once, "x = () => (c ? d : (a ?? b /* c */));\n");
    assert_eq!(format(&once), once, "not a fixed point");
}
