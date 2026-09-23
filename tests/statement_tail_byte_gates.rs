// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The TypeScript printer asks two questions of every statement's TAIL — the stretch between
//! its content and its `;` — and answers almost all of them from the tail's last two bytes
//! instead of walking it: whether an author blank sits inside the tail
//! (`statement_content_tail_blank`) and where the trivia run before a `;` begins
//! (`trivia_run_start`). Each byte gate answers only where its walk provably would, and in a
//! debug build it also runs that walk and asserts the two agree — so these cases grade the
//! gates themselves, not only the output.
//!
//! The cases sit at every boundary a gate must get right: each byte that must send a tail to
//! the walk (a comment's `/`, a regex's closing `/`, whitespace, a line comment's line break,
//! `<LS>` / `<PS>`, a non-ASCII character) and each tail the gates answer themselves (ASI's
//! missing `;`, a statement ending at EOF), in the statement kinds the tail question reaches
//! (a plain list member, a header kind's clause body, a `switch` consequent, a Svelte
//! `<script>`). Each expected output is the formatter's settled answer and prettier's fixed
//! point, and must be a fixed point here too.

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

fn assert_formats(source: &str, parser: ParserType, expected: &str) {
    let formatted = format_ok(source, parser);
    assert_eq!(formatted, expected, "formatting {source:?}");
    assert_eq!(
        format_ok(&formatted, parser),
        formatted,
        "the formatted form of {source:?} must be a fixed point"
    );
}

fn format_ok(source: &str, parser: ParserType) -> String {
    let formatted = format_source(source, parser);
    assert!(formatted.is_ok(), "{source:?} must format: {formatted:?}");
    formatted.expect("asserted Ok above")
}

#[test]
fn comment_in_the_tail_takes_the_walk() {
    // an own-line block before a detached `;` leads the next statement
    assert_formats(
        "a()\n/* c */;\nb();\n",
        ParserType::TypeScript,
        "a();\n/* c */ b();\n",
    );
    // a line comment the `;` sits below trails the statement
    assert_formats(
        "a() // c\n;\nb();\n",
        ParserType::TypeScript,
        "a(); // c\nb();\n",
    );
    // a block glued to the `;` — the `/` it closes on sends the tail to the walk
    assert_formats(
        "a()/* c */;\nz = 1 /* c */;\n",
        ParserType::TypeScript,
        "a(); /* c */\nz = 1; /* c */\n",
    );
    // the same in a function body, where the list walk's comment seam also asks
    assert_formats(
        "function f() {\n\t// c\n\ta()\n\t/* d */;\n\tb() // e\n\t;\n\treturn x /* f */;\n}\n",
        ParserType::TypeScript,
        "function f() {\n\t// c\n\ta();\n\t/* d */ b(); // e\n\treturn x; /* f */\n}\n",
    );
}

#[test]
fn a_regex_ending_the_content_takes_the_walk() {
    assert_formats(
        "x = /re/;\ny = /a\\//;\n",
        ParserType::TypeScript,
        "x = /re/;\ny = /a\\//;\n",
    );
}

#[test]
fn whitespace_in_the_tail_takes_the_walk() {
    assert_formats("a()  ;\nb()\t;\n", ParserType::TypeScript, "a();\nb();\n");
    // an author blank inside the tail survives the `;` moving back up
    assert_formats("a()\n\n;\nb();\n", ParserType::TypeScript, "a();\n\nb();\n");
    // a non-ASCII byte before the `;` takes the walk whatever it is: whitespace (NBSP,
    // ZWNBSP) is trimmed, an identifier's last byte is not
    assert_formats(
        "a = x\u{e9};\nb = 1\u{a0};\nc = 2\u{feff};\nd();\n",
        ParserType::TypeScript,
        "a = x\u{e9};\nb = 1;\nc = 2;\nd();\n",
    );
}

#[test]
fn line_separators_in_the_tail_take_the_walk() {
    // one `<LS>` / `<PS>` is a line break, not a blank
    assert_formats(
        "a()\u{2028};\nb()\u{2029};\nc();\n",
        ParserType::TypeScript,
        "a();\nb();\nc();\n",
    );
    // two are a blank line, which the content-end arm keeps
    assert_formats(
        "a()\u{2028}\u{2028};\nb();\n",
        ParserType::TypeScript,
        "a();\n\nb();\n",
    );
}

#[test]
fn asi_and_eof_have_no_tail() {
    assert_formats("a()\nb()", ParserType::TypeScript, "a();\nb();\n");
}

#[test]
fn every_kind_the_tail_question_reaches() {
    // header kinds answer through their clause body; `debugger` through its keyword
    assert_formats(
        "label: a()\n\n;\nif (x) b()\n\n;\nfor (;;) c()\n\n;\ndebugger\n\n;\nd();\n",
        ParserType::TypeScript,
        "label: a();\n\nif (x) b();\n\nfor (;;) c();\n\ndebugger;\n\nd();\n",
    );
    // a switch consequent runs its own statement walk
    assert_formats(
        "switch (a) {\n\tcase 1:\n\t\tb()\n\n\t\t;\n\t\tc() /* e */;\n\t\td()\n\t\t// f\n\t\t;\n}\n",
        ParserType::TypeScript,
        "switch (a) {\n\tcase 1:\n\t\tb();\n\n\t\tc(); /* e */\n\t\td();\n\t// f\n}\n",
    );
}

#[test]
fn a_line_comment_the_terminator_defers() {
    // a clause body keeps its own gap, so its `//` defers past the `;` and the list reads it
    assert_formats(
        "function f() {\n\t// c\n\ta() // d\n\t;\n\tb();\n\tif (x) y()\n\t// e\n\t;\n\tz();\n}\n",
        ParserType::TypeScript,
        "function f() {\n\t// c\n\ta(); // d\n\tb();\n\tif (x) y();\n\t// e\n\tz();\n}\n",
    );
    // the member lists ask the same terminator-gap question of their own separators
    assert_formats(
        "class C {\n\t// c\n\ta = 1 // d\n\t;\n\tb = 2;\n\tc = 3 /* e */;\n\td = /re/;\n}\ntype T = {\n\t// c\n\ta: A // d\n\t;\n\tb: B;\n\tc: C /* e */,\n};\n",
        ParserType::TypeScript,
        "class C {\n\t// c\n\ta = 1; // d\n\tb = 2;\n\tc = 3; /* e */\n\td = /re/;\n}\ntype T = {\n\t// c\n\ta: A; // d\n\tb: B;\n\tc: C /* e */;\n};\n",
    );
}

#[test]
fn svelte_script_statements() {
    assert_formats(
        "<script lang=\"ts\">\n\ta()\n\n\t;\n\tb()\n\t/* c */;\n\tc() /* d */;\n\td()\u{2028};\n</script>\n",
        ParserType::Svelte,
        "<script lang=\"ts\">\n\ta();\n\n\tb();\n\t/* c */ c(); /* d */\n\td();\n</script>\n",
    );
    // a line comment ending where the script's text ends, with no `;` behind it
    assert_formats(
        "<script lang=\"ts\">a(); // c</script>\n",
        ParserType::Svelte,
        "<script lang=\"ts\">\n\ta(); // c\n</script>\n",
    );
}
