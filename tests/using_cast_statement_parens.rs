// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The `using` cast statement the fixture path cannot carry on the bare side.
//!
//! `using as T;` has two readings, one per edition of the grammar. The canonical parsers run
//! acorn-typescript at ES2025, where `using` is no keyword and the line is a cast of the
//! identifier `using` — the reading tsv's parse follows. tsc, and acorn at
//! `ecmaVersion: 'latest'`, read the head of an ES2026 `using` declaration binding a name
//! `as`, and reject the line for its missing initializer. tsv accepts the bare form and
//! REPAIRS it, printing `(using) as T;`, the one spelling every reader takes as the cast.
//!
//! The bare spelling is not a tsv fixed point, so it cannot be an `input.*`; and it cannot be
//! a variant either, since prettier's TypeScript parser is tsc's and throws on it, where the
//! variant rules need prettier to LAND on some form. The oracle-backed side of the same rule
//! — the pair as authored, which prettier keeps too — is the
//! `typescript_specific/using/cast` fixture.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

#[test]
fn a_bare_using_cast_gets_the_pair_back() {
    for (bare, repaired) in [
        ("using as T;\n", "(using) as T;\n"),
        ("using satisfies T;\n", "(using) satisfies T;\n"),
        ("using as T as U;\n", "(using) as T as U;\n"),
        ("if (a) using as T;\n", "if (a) (using) as T;\n"),
        ("for (using as T; ;) {}\n", "for ((using) as T; ;) {}\n"),
    ] {
        assert_eq!(
            format(bare),
            repaired,
            "bare `using` cast must gain the pair"
        );
        assert_eq!(
            format(repaired),
            repaired,
            "the repaired form is a fixed point"
        );
    }
}

#[test]
fn a_using_declaration_is_not_a_cast() {
    // The contrast that bounds the class: any other word after `using` binds, so the
    // statement is a declaration in every reader and there is no pair to add.
    let declaration = "using a = b;\n";
    assert_eq!(format(declaration), declaration);
}
