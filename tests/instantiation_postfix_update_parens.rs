// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The paren pair around an instantiation expression ahead of a POSTFIX update
//! operator: `(f<T>)++` keeps it, `++(f<T>)` strips it.
//!
//! A `++` starts an expression, so it cannot follow a type argument list (tsc's
//! `canFollowTypeArgumentsInExpression`): bare `f<T>++` does not parse, while the
//! prefix spelling `++f<T>` does (nothing follows the list). Every other follower in
//! the class lives in the
//! `typescript_specific/generics/instantiation_paren_follow_prettier_divergence`
//! fixture; this one cannot, because acorn-typescript rejects `(f<T>)++` outright
//! (`Assigning to rvalue`, a check tsc's parser leaves to its checker), so no
//! `expected.json` can be generated for it.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

#[test]
fn postfix_update_keeps_the_pair() {
    for source in ["(f<T>)++;\n", "(f<T>)--;\n", "(obj.f<T>)++;\n"] {
        let formatted = format(source);
        assert_eq!(formatted, source, "postfix update on an instantiation");
        // The output must reparse — the bare spelling does not.
        let arena = bumpalo::Bump::new();
        assert!(tsv_ts::parse(&formatted, &arena).is_ok());
    }
    let arena = bumpalo::Bump::new();
    assert!(tsv_ts::parse("f<T>++;\n", &arena).is_err());
}

#[test]
fn prefix_update_strips_the_pair() {
    assert_eq!(format("++(f<T>);\n"), "++f<T>;\n");
    assert_eq!(format("--(f<T>);\n"), "--f<T>;\n");
}
