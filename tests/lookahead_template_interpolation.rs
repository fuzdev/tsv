// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used, clippy::panic)]

//! A template literal read by `tsv_ts`'s raw lookaheads.
//!
//! The scans that find an arrow head's `)` or a type-argument list's `>` ahead of the parse
//! step over a template literal whole, its interpolations scanned as the expressions they
//! are. Stepping from one backtick to the next instead pairs the wrong two when an
//! interpolation holds a backtick (`` `${'`'}` ``), and the quote left over opens a string
//! that runs on past the expression — so whether the head parsed turned on what followed
//! the statement.

/// Whether `source` parses as a module.
fn parses(source: &str) -> bool {
    tsv_ts::__private::parse(source).is_ok()
}

/// What a scan that ran past its statement could read: nothing, a quote, a quote before a
/// `)` and an `=>`, a quote before a `>` and a `(`, a backtick, a slash before a `)` and an
/// `=>`, a lone slash.
const TAILS: [&str; 7] = [
    "",
    "\nconst y = 'a';",
    "\nconst y = ') => 1';",
    "\nconst y = '>(1)';",
    "\nconst y = `a`;",
    "\ny = /) => 1/;",
    "; y / 2;",
];

#[test]
fn a_lookaheads_answer_never_depends_on_what_follows_the_statement() {
    for head in [
        // An arrow head's `)`.
        "const a = (x = `${'`'}`) => x;",
        "const a = (x = `${\"`\"}`) => x;",
        "const a = <T,>(x = `${'`'}`) => x;",
        "const a = (x = `${`)`}`) => x;",
        "const a = (x = `${`it's`}`) => x;",
        "const a = (x = `${y}${'`'}`) => x;",
        "const a = (x: `${'`'}`) => x;",
        "const a = (x): `${'`'}` => x;",
        "const a = <T extends `${'`'}`>(x: T) => x;",
        "const a = c ? (x = `${'`'}`): T => x : y;",
        // A type-argument list's `>`.
        "f<A, `${'`'}`>(c);",
        "new F<A, `${'`'}`>(c);",
        "f<A, B<`${'`'}`>>(c);",
        "f<{ a: `${'`'}` }>(c);",
        "f<(a: `${'`'}`) => void>(c);",
        "const a = f<A, `${'`'}`>;",
        // A `/` after an object literal inside the interpolation divides.
        "const a = (x = `${{} / 2}`) => x / 2;",
        // A template skipped inside one of the grammar's answers (the `>` before a `/`)
        // reads its interpolations under the grammar too: an object literal's `}` and a
        // type-argument list's `>` before a `/` divide there as well.
        "x = (a = [`${{} / 2}`, f<T>/2]) => a;",
        "x = (a = `${{} / 2}` + f<T>/2) => a;",
        "x = (a = `${g<T>/2}`, b = f<T>/2) => a;",
        "x = <T,>(a = `${{} / 2}`, b = f<T>/2) => a;",
        "x = (a = `${`${{} / 2}`}`, b = f<T>/2) => a;",
        "x = (a = `${(a = `${(a = A) => f<T>/2}`) => f<T>/2}`) => a;",
        "x = (a = `${(a = `${(a = `${(a = A) => f<T>/2}`) => f<T>/2}`) => f<T>/2}`) => a;",
        // Inside those answers a `}` is read as the grammar reads it too: the object
        // literal's `/` divides, so a quote or slash after it is no regex's body, and the
        // list's `>` is still found.
        "x = (a = `${{} / 2 + '/' + f<T>/2}`) => a;",
        "x = (a = `${{} / 2 + \"/\" + f<T>/2}`) => a;",
        "x = (a = `${{} / 2 + `/` + f<T>/2}`) => a;",
        "x = (a = `${{} / 2 + `${f<T>/2}` + f<T>/2}`) => a;",
        "x = (a = `${{} / 2 + '/' + {} / 2 + f<T>/2}`) => a;",
        "x = (a = {} / 2 + '/' + f<T>/2) => a;",
    ] {
        for tail in TAILS {
            let source = format!("{head}{tail}");
            assert!(parses(&source), "{source:?}");
        }
    }
}

#[test]
fn a_type_argument_list_holding_a_backtick_in_an_interpolation_is_a_call() {
    // A comparison chain would put a `SequenceExpression` at the statement's root.
    for tail in TAILS {
        let source = format!("f<A, `${{'`'}}`>(c);{tail}");
        let json = tsv_ts::parse_to_json(&source).expect("parse failed");
        assert!(
            json.contains(r#""expression":{"type":"CallExpression""#),
            "{source:?}: {json}"
        );
    }
}

/// `n` levels of `wrap`, innermost `A`: the `@` in `wrap` is the level inside it.
fn nest(wrap: &str, n: usize) -> String {
    (0..n).fold("A".to_owned(), |inner, _| wrap.replace('@', &inner))
}

/// Run `parse` on its own thread, and fail if it is not done within `seconds`.
fn within<T: Send + 'static>(
    seconds: u64,
    name: &str,
    parse: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .stack_size(tsv_ts::STACK_SIZE)
        .spawn(move || done.send(parse()))
        .expect("spawn");
    finished
        .recv_timeout(std::time::Duration::from_secs(seconds))
        .unwrap_or_else(|_| panic!("{name}: not parsed within {seconds}s"))
}

/// Each of the grammar's answers walks its region again, skipping the templates there, whose
/// interpolations ask the answers again a level deeper — exponential in the nesting unless
/// each answer is remembered — and the parser starts a lookahead at every level's arrow head
/// or list, each scanning the levels inside it: a memo kept for one scan alone works every
/// answer out again per scan around it, a power of the depth past any deadline here. Kept
/// for the whole parse, these nests cost a low polynomial in their depth, well within it.
///
/// The deepest nests are past the parser's depth limit, which refuses them only once its
/// descent reaches the limit — after the lookahead at the outermost arrow head has scanned
/// the whole nest.
#[test]
fn nested_templates_asking_the_grammar_cost_a_polynomial_in_their_depth() {
    const SECONDS: u64 = 30;
    for (name, wrap, depth, svelte, accepted) in [
        ("arrow", "`${(a = @) => f<T>/2}`", 48, false, true),
        ("type argument", "`${f<@>/2}`", 48, false, false),
        ("braced operand", "`${f<@>/2 + {} / 2}`", 48, false, false),
        ("statement header", "`${if_(c)!/2 + (@)}`", 48, false, true),
        (
            "arrow, object",
            "`${(a = @, b = {} / 2) => f<T>/2}`",
            128,
            false,
            false,
        ),
        ("svelte arrow", "`${(a = @) => f<T>/2}`", 48, true, true),
        (
            "svelte braced operand",
            "`${f<@>/2 + {} / 2}`",
            48,
            true,
            false,
        ),
        (
            "svelte arrow, object",
            "`${(a = @, b = {} / 2) => f<T>/2}`",
            128,
            true,
            false,
        ),
    ] {
        let nested = nest(wrap, depth);
        let parsed = if svelte {
            let source = format!("<script lang=\"ts\"></script><p>{{{nested}}}</p>\n");
            within(SECONDS, name, move || {
                tsv_svelte::parse_to_json(&source).is_ok()
            })
        } else {
            let source = format!("x = (a = {nested}) => a;\n");
            within(SECONDS, name, move || parses(&source))
        };
        assert_eq!(parsed, accepted, "{name}");
    }
}
