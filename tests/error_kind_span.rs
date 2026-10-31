// helper fns here aren't `#[test]`, so clippy.toml's allow-expect/panic-in-tests don't reach them
#![expect(clippy::expect_used)]

//! `Error::kind` and `Error::span` through the real entry points, and the two `Display`
//! forms.
//!
//! `span` is the error's byte range in the text the CALLER handed over: a format entry
//! point's error is mapped back through its `<CR>` fold, an embedded `<script>`'s is
//! absolute in the host document, and a leading byte-order mark is counted in every
//! language — unlike `report()`, whose points are UTF-16 and whose Svelte and CSS wires
//! elide the mark. `Display` is one line (`summary (line:col)`); `{:#}` is the full frame
//! the CLI prints.

use tsv_lang::{Error, ErrorKind};

fn ts_parse_error(source: &str) -> Error {
    let arena = bumpalo::Bump::new();
    tsv_ts::__private::parse(source, &arena).expect_err("the source must not parse")
}

fn svelte_parse_error(source: &str) -> Error {
    let arena = bumpalo::Bump::new();
    tsv_svelte::__private::parse(source, &arena).expect_err("the source must not parse")
}

fn css_parse_error(source: &str) -> Error {
    let arena = bumpalo::Bump::new();
    tsv_css::__private::parse(source, &arena).expect_err("the source must not parse")
}

#[test]
fn a_syntax_error_spans_its_point() {
    let source = "let a = 1;\nlet x = ;\n";
    let error = ts_parse_error(source);
    assert_eq!(error.kind(), ErrorKind::Syntax);
    let semicolon = source.rfind(';').expect("a `;`");
    assert_eq!(error.span(), Some(semicolon..semicolon));
    assert_eq!(error.span().map(|span| span.start), error.position());
}

#[test]
fn a_format_error_spans_the_callers_bytes_across_the_cr_fold() {
    // Each `<CR><LF>` folds to one byte ahead of the parse; the span is mapped back.
    let source = "let a = 1;\r\nlet b = 2;\r\nlet x = ;\r\n";
    let error = tsv_ts::format(source).expect_err("the source must not format");
    assert_eq!(error.kind(), ErrorKind::Syntax);
    let semicolon = source.rfind(';').expect("a `;`");
    assert_eq!(error.span(), Some(semicolon..semicolon));
    // the parse of the same bytes agrees: no fold, no map
    assert_eq!(ts_parse_error(source).span(), error.span());

    let svelte = "<p>a</p>\r\n<p>b</p>\r\n{a b}\r\n";
    let error = tsv_svelte::format(svelte).expect_err("the source must not format");
    assert_eq!(error.kind(), ErrorKind::Syntax);
    assert_eq!(error.span(), svelte_parse_error(svelte).span());
    let b = svelte.find(" b}").expect("the `b`") + 1;
    assert_eq!(error.span(), Some(b..b));
}

#[test]
fn a_leading_bom_is_a_counted_byte_in_every_language() {
    // TypeScript: the wire counts the mark too, so the report's UTF-16 start is the span's
    // byte offset less the mark's two extra UTF-8 bytes.
    let ts = "\u{FEFF}let x = ;";
    let error = ts_parse_error(ts);
    let semicolon = ts.find(';').expect("a `;`");
    assert_eq!(error.span(), Some(semicolon..semicolon));
    let report = error.report().expect("a located error reports");
    assert_eq!(report.start.offset as usize, semicolon - 2);

    // Svelte: the wire elides the mark, so the report's start is one UTF-16 unit lower still
    // — the span is not.
    let svelte = "\u{FEFF}<p>{a b}</p>";
    let error = svelte_parse_error(svelte);
    let b = svelte.find(" b}").expect("the `b`") + 1;
    assert_eq!(error.span(), Some(b..b));
    let report = error.report().expect("a located error reports");
    assert_eq!(report.start.offset as usize, b - 3);

    // CSS: elided on the wire as Svelte's is.
    let css = "\u{FEFF}a { color: red; } }";
    let error = css_parse_error(css);
    let brace = css.rfind('}').expect("a `}`");
    assert_eq!(error.span(), Some(brace..brace));
    let report = error.report().expect("a located error reports");
    assert_eq!(report.start.offset as usize, brace - 3);
}

#[test]
fn an_embedded_script_error_spans_host_absolute_bytes() {
    let source = "<p>before</p>\n<script lang=\"ts\">\n\tlet x = ;\n</script>\n";
    let error = svelte_parse_error(source);
    assert_eq!(error.kind(), ErrorKind::Syntax);
    let semicolon = source.find(';').expect("a `;`");
    assert_eq!(error.span(), Some(semicolon..semicolon));

    let style = "<p>x</p>\n<style>\n\ta { color: red; } }\n</style>\n";
    let error = svelte_parse_error(style);
    let brace = style.rfind("} }").expect("the stray `}`") + 2;
    assert_eq!(error.span(), Some(brace..brace));
}

#[test]
fn a_lone_cr_refusal_spans_the_cr() {
    let source = "<p>a</p>\r\n<div // c\rclass=\"x\"\n>hi</div>\n";
    let error = tsv_svelte::format(source).expect_err("the format must refuse");
    assert_eq!(error.kind(), ErrorKind::Refusal);
    let cr = source
        .match_indices('\r')
        .map(|(at, _)| at)
        .find(|&at| source.as_bytes().get(at + 1) != Some(&b'\n'))
        .expect("a lone `<CR>`");
    assert_eq!(error.span(), Some(cr..cr + 1));
    // the refusal's one line is its summary and the `<CR>`'s point
    assert!(error.to_string().ends_with(" (2:10)"), "{error}");
}

#[test]
fn a_depth_refusal_spans_where_the_limit_tripped() {
    let source = format!("x = {}a{};", "(".repeat(400), ")".repeat(400));
    // a debug build's parse keeps its depth contract on a 16 MiB stack
    let error = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || ts_parse_error(&source))
        .expect("spawn")
        .join()
        .unwrap_or_else(|_| panic!("the parse panicked"));
    assert_eq!(error.kind(), ErrorKind::TooDeep);
    let at = error.position().expect("located");
    assert_eq!(error.span(), Some(at..at));
    assert!(error.wire_point().is_some());
}

#[test]
fn display_is_one_line_and_the_alternate_is_the_frame() {
    let source = "let a = 1;\nlet x = ;\n";
    let error = ts_parse_error(source);
    let one = error.to_string();
    let full = format!("{error:#}");
    assert!(!one.contains('\n'), "{one:?}");
    assert_eq!(one, "Expected expression, found ';' (2:9)");
    assert_eq!(
        full,
        "Expected expression, found ';'\n2:9 let x = ;\n            ^ here"
    );
    // the one line is the frame's summary line plus its header's point, exactly
    let mut lines = full.lines();
    let summary = lines.next().expect("a summary");
    let header = lines
        .next()
        .and_then(|line| line.split_once(' '))
        .map(|(at, _)| at)
        .expect("a header");
    assert_eq!(one, format!("{summary} ({header})"));
    // and `report()` holds the same two parts apart
    let report = error.report().expect("a located error reports");
    assert_eq!(full, format!("{}\n{}", report.summary, report.frame));

    // the strip's parse error forwards both forms
    let strip = {
        let arena = bumpalo::Bump::new();
        tsv_ts::__private::strip_in(source, None, &arena).expect_err("the source must not strip")
    };
    assert_eq!(strip.kind(), ErrorKind::Syntax);
    assert_eq!(strip.to_string(), one);
    assert_eq!(format!("{strip:#}"), full);
    assert_eq!(strip.message(source), full);

    // a strip refusal is a refusal, and its message keeps the frame
    let enumerated = "enum E { A }\n";
    let refusal = {
        let arena = bumpalo::Bump::new();
        tsv_ts::__private::strip_in(enumerated, None, &arena).expect_err("an enum is refused")
    };
    assert_eq!(refusal.kind(), ErrorKind::Refusal);
    assert_eq!(refusal.to_string(), "cannot strip an enum");
    assert!(
        refusal
            .message(enumerated)
            .starts_with("cannot strip an enum\n1:1 "),
        "{}",
        refusal.message(enumerated)
    );
}
