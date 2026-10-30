// the `utf8` helper isn't a `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach it
#![expect(clippy::expect_used)]

//! The public one-shot parse, `parse_to_json` / `parse_to_json_with(&JsonOptions)` in each
//! language crate: the same wire bytes the hidden AST-taking path writes
//! (`__private::parse*` + `__private::convert_ast_json_*`, what the bindings and the CLI
//! call), the same error, and `JsonOptions`'s defaults equal to the bare form's.

use bumpalo::Bump;
use tsv_ts::Goal;

const TS: [&str; 3] = [
    "const x: number = 42;\n",
    "// c\nexport function f<T>(a: T) { return a /* t */; }\n",
    "let s = '\u{1F600}';\r\nlet t = `a${s}`;\u{2028}x;\n",
];
const SCRIPT: &str = "with (a) { await = 010; }\n";
const CSS: [&str; 2] = [
    "div { color: red; }\n",
    "/* c */\r\n@media (min-width: 1px) { a::after { content: '\u{1F600}'; } }\n",
];
const SVELTE: [&str; 2] = [
    "<script lang=\"ts\">let x: number = 1;</script>\n<div>{x}</div>\n",
    "<div // c\n\tclass=\"a\">{'\u{1F600}'}</div>\r\n<style>a { color: red; }</style>\n",
];

fn utf8(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("the wire is UTF-8")
}

#[test]
fn ts_parse_to_json_is_the_module_span_wire() {
    for source in TS {
        let arena = Bump::new();
        let ast = tsv_ts::__private::parse(source, &arena).expect("parses");
        let span = utf8(tsv_ts::__private::convert_ast_json_bytes(&ast, source));
        let loc = utf8(tsv_ts::__private::convert_ast_json_bytes_with_locations(
            &ast, source,
        ));
        assert_eq!(
            tsv_ts::parse_to_json(source).as_deref(),
            Ok(span.as_str()),
            "{source:?}"
        );
        let options = tsv_ts::JsonOptions::new().locations(true);
        assert_eq!(
            tsv_ts::parse_to_json_with(source, &options),
            Ok(loc),
            "{source:?}"
        );
    }
}

#[test]
fn ts_json_options_goal_is_exact() {
    let arena = Bump::new();
    let ast = tsv_ts::__private::parse_with_goal(SCRIPT, Goal::Script, &arena).expect("a script");
    let span = utf8(tsv_ts::__private::convert_ast_json_bytes(&ast, SCRIPT));
    let loc = utf8(tsv_ts::__private::convert_ast_json_bytes_with_locations(
        &ast, SCRIPT,
    ));
    let script = tsv_ts::JsonOptions::new().goal(Goal::Script);
    assert_eq!(tsv_ts::parse_to_json_with(SCRIPT, &script), Ok(span));
    assert_eq!(
        tsv_ts::parse_to_json_with(SCRIPT, &script.locations(true)),
        Ok(loc)
    );
    // no fallback: the bare form is the module goal alone, and its error the module parse's
    let module_error = tsv_ts::__private::parse(SCRIPT, &arena).expect_err("not a module");
    let bare = tsv_ts::parse_to_json(SCRIPT).expect_err("the bare form is Module");
    assert_eq!(format!("{bare:#}"), format!("{module_error:#}"));
    let named = tsv_ts::JsonOptions::new().goal(Goal::Module);
    let exact = tsv_ts::parse_to_json_with(SCRIPT, &named).expect_err("Module is exact");
    assert_eq!(format!("{exact:#}"), format!("{module_error:#}"));
}

#[test]
fn json_options_default_is_new() {
    assert_eq!(tsv_ts::JsonOptions::default(), tsv_ts::JsonOptions::new());
    assert_eq!(
        tsv_ts::JsonOptions::new(),
        tsv_ts::JsonOptions::new()
            .goal(Goal::Module)
            .locations(false)
    );
    assert_eq!(tsv_css::JsonOptions::default(), tsv_css::JsonOptions::new());
    assert_eq!(
        tsv_css::JsonOptions::new(),
        tsv_css::JsonOptions::new().locations(false)
    );
    assert_eq!(
        tsv_svelte::JsonOptions::default(),
        tsv_svelte::JsonOptions::new()
    );
    assert_eq!(
        tsv_svelte::JsonOptions::new(),
        tsv_svelte::JsonOptions::new().locations(false)
    );
}

#[test]
fn css_parse_to_json_is_the_span_wire() {
    for source in CSS {
        let arena = Bump::new();
        let ast = tsv_css::__private::parse(source, &arena).expect("parses");
        let span = utf8(tsv_css::__private::convert_ast_json_bytes(&ast, source));
        let loc = utf8(tsv_css::__private::convert_ast_json_bytes_with_locations(
            &ast, source,
        ));
        assert_eq!(
            tsv_css::parse_to_json(source).as_deref(),
            Ok(span.as_str()),
            "{source:?}"
        );
        let options = tsv_css::JsonOptions::new().locations(true);
        assert_eq!(
            tsv_css::parse_to_json_with(source, &options),
            Ok(loc),
            "{source:?}"
        );
    }
}

#[test]
fn svelte_parse_to_json_is_the_span_wire() {
    for source in SVELTE {
        let arena = Bump::new();
        let ast = tsv_svelte::__private::parse(source, &arena).expect("parses");
        let span = utf8(tsv_svelte::__private::convert_ast_json_bytes(&ast, source));
        let loc = utf8(tsv_svelte::__private::convert_ast_json_bytes_with_locations(&ast, source));
        assert_eq!(
            tsv_svelte::parse_to_json(source).as_deref(),
            Ok(span.as_str()),
            "{source:?}"
        );
        let options = tsv_svelte::JsonOptions::new().locations(true);
        assert_eq!(
            tsv_svelte::parse_to_json_with(source, &options),
            Ok(loc),
            "{source:?}"
        );
    }
}

#[test]
fn a_parse_error_is_the_hidden_parse_error() {
    let arena = Bump::new();
    let cases: [(&str, Result<String, tsv_ts::Error>, tsv_ts::Error); 3] = [
        (
            "ts",
            tsv_ts::parse_to_json("let = ;"),
            tsv_ts::__private::parse("let = ;", &arena).expect_err("invalid"),
        ),
        (
            "css",
            tsv_css::parse_to_json("a { color: red;"),
            tsv_css::__private::parse("a { color: red;", &arena).expect_err("invalid"),
        ),
        (
            "svelte",
            tsv_svelte::parse_to_json("<div>{</div>"),
            tsv_svelte::__private::parse("<div>{</div>", &arena).expect_err("invalid"),
        ),
    ];
    for (lang, one_shot, hidden) in cases {
        let one_shot = one_shot.expect_err(lang);
        assert_eq!(format!("{one_shot:#}"), format!("{hidden:#}"), "{lang}");
    }
}
