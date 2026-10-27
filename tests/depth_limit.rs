//! The parsers' depth limits (`tsv_lang::NESTING_LIMIT`, `tsv_lang::SPINE_LIMIT`) and the
//! stack contract they exist for — `docs/cli.md` §Recursion Depth.
//!
//! For every shape the limits count — the named constructs, the catalog of grammar
//! positions, and the generated sweeps over link, prefix and decorator combinations
//! (`tsv_debug::depth_shapes`) — generated at its limit:
//!
//! - **no path through its wire passes either limit**: the parse charges at least what the
//!   walks recurse through, so a construct it leaves uncounted — a level the wire puts on
//!   the path each repetition and the count does not see — fails here, whether or not its
//!   walks happen to fit the stack ([`check_wire`]);
//! - the deepest accepted input runs **parse, both wire writers (span-only and `loc`),
//!   format and — TypeScript — the type strip** on a thread nine tenths of the contract's
//!   size ([`GATE_STACK`]), which it must survive: a stack overflow aborts this test
//!   binary, which fails the gate;
//! - its format's output **parses again and is its own format** (F1): the pairs the printer
//!   adds are levels of the output's nesting, which the parse charges as it reads the input
//!   (`tsv_lang::SpineCount::try_pair`), so an accepted input's output is never refused;
//! - one level deeper is refused with the depth error, the limit it names included;
//! - the shapes the wire records exactly are **calibrated**: the accepted input's wire depth
//!   also sits AT the limit, so the count does not overcharge them either.
//!
//! The contract's thread is **2 MiB in an optimized build** and **16 MiB in a debug build**
//! (`cfg!(debug_assertions)` is the proxy), so the stack legs grade the shipped contract
//! only where this file builds optimized, and only in the optimization it builds in:
//! `deno task test:depth:napi` runs it in the shipped one (the `napi` profile) over the full
//! sweep (`TSV_DEPTH_SWEEP=full`, [`Scope::Full`]), `deno task test:depth` in `check`'s
//! `corpus` world over `check`'s scope ([`Scope::Check`]), and a plain `cargo test` grades
//! the debug contract, over the named shapes and a stride of `check`'s sweep. Frame sizes move with
//! every printer change, which is why the limit has a standing gate at all: a frame that
//! grows past the margin fails here before it ships. The WASM engine's two stacks are
//! graded apart, over the full sweep's shapes at the same depths (`write_shape_limits`, read by
//! `deno task test:depth:wasm`).

#![expect(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::thread;

use tsv_lang::{NESTING_FLOOR, NESTING_LIMIT, NESTING_SPINE_WEIGHT, SPINE_LIMIT};

/// The stack the contract promises the limits fit (`docs/cli.md` §Recursion Depth): 2 MiB
/// optimized, 16 MiB debug — an unoptimized build's frames are several times larger, and
/// larger again with debuginfo, cargo's `dev` default.
const CONTRACT_STACK: usize = if cfg!(debug_assertions) {
    16 << 20
} else {
    2 << 20
};

/// The thread the gate runs every walk on: the contract's, less a tenth kept in hand, so a
/// frame that grows fails here while every accepted input still fits the contract — the
/// minimum margin the gate holds is the contract over this.
const GATE_STACK: usize = CONTRACT_STACK / 10 * 9;

/// The thread the walks run on: [`GATE_STACK`], or the bytes `TSV_DEPTH_GATE_STACK` names —
/// for measuring the margin a build keeps (the smallest thread every shape survives). Only
/// a smaller thread is taken, so a value left set can make a gate run stricter, never
/// looser; one larger than [`GATE_STACK`], or one that is no byte count, fails the run.
fn gate_stack() -> usize {
    static STACK: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| {
        let Some(bytes) = knob("TSV_DEPTH_GATE_STACK") else {
            return GATE_STACK;
        };
        let stack: usize = bytes
            .parse()
            .unwrap_or_else(|_| panic!("TSV_DEPTH_GATE_STACK={bytes:?} is not a byte count"));
        assert!(
            stack <= GATE_STACK,
            "TSV_DEPTH_GATE_STACK={stack} is larger than the gate's own stack ({GATE_STACK}): \
             it can only make the gate stricter"
        );
        stack
    });
    *STACK
}

/// A measuring knob's value, if set — with one warning line on stderr, since a knob left set
/// changes what a gate run grades. Written to the stream itself, past the harness's capture
/// of a passing test's output, so a passing run shows it too.
fn knob(name: &str) -> Option<String> {
    use std::io::Write as _;
    let value = std::env::var(name).ok()?;
    let _ = writeln!(
        std::io::stderr(),
        "warning: {name}={value:?} is set — a measuring run, not a gate run"
    );
    Some(value)
}

/// The stack the limit searches run on: deep enough that the search's own probes past the
/// limit never overflow. Only the parse runs there.
const SEARCH_STACK: usize = 256 << 20;

use tsv_debug::depth_shapes as shapes;

use shapes::{Lang, Limit, Scope, Shape, Wire, rep, shapes};

/// Run `f` on a thread of `stack` bytes, named `name` — the name is what the runtime's
/// stack-overflow abort prints, the one diagnostic an overflow leaves.
fn on_thread<T: Send + 'static>(
    name: String,
    stack: usize,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    thread::Builder::new()
        .name(name)
        .stack_size(stack)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the thread panicked")
}

/// The parse of `source` — `Ok` or its error's message.
fn parse(lang: Lang, source: &str) -> Result<(), String> {
    let arena = bumpalo::Bump::new();
    match lang {
        Lang::Ts => tsv_ts::parse(source, &arena).map(drop),
        Lang::Css => tsv_css::parse(source, &arena).map(drop),
        Lang::Svelte => tsv_svelte::parse(source, &arena).map(drop),
    }
    .map_err(|e| e.to_string())
}

/// The wire's depth counts for `source` ([`WireCounts`]). TypeScript keeps its grouping
/// parens for the count (`parse_preserve_parens`), since each one is a level of the
/// parse's recursion though the default wire drops it.
fn wire_counts(lang: Lang, source: &str) -> WireCounts {
    let arena = bumpalo::Bump::new();
    let wire = match lang {
        Lang::Ts => tsv_ts::convert_ast_json_bytes(
            &tsv_ts::parse_preserve_parens(source, &arena).expect("parses"),
            source,
        ),
        Lang::Css => tsv_css::convert_ast_json_bytes(
            &tsv_css::parse(source, &arena).expect("parses"),
            source,
        ),
        Lang::Svelte => tsv_svelte::convert_ast_json_bytes(
            &tsv_svelte::parse(source, &arena).expect("parses"),
            source,
        ),
    };
    walk_wire(&wire)
}

/// What the walks over a wire recurse through, maximized over its root-to-node paths.
#[derive(Clone, Copy, Debug, Default)]
struct WireCounts {
    /// The deepest path, left-spine links taken out.
    nesting: u32,
    /// The most left-spine links on one path.
    spine: u32,
    /// The most a path costs in spine units: its spine links, at the one unit the cheapest
    /// link counts, and its nesting at `NESTING_SPINE_WEIGHT` units a level — the sum
    /// every link's spine check bounds.
    combined: u32,
}

/// Whether the link from a `parent` node's `key` — its `slot`th element where the key holds
/// an array — is a left-spine link: one the parsers count against `SPINE_LIMIT` as they
/// build the parent, because the operand was parsed before the parent was known to hold
/// it — a chain link's operand and a binary's left operand, whatever node the operand is
/// (the operand, a paren or a call's argument list, is charged as its own level where it
/// is one), and the left operands the other wrappers take the same way: a conditional's
/// test, a sequence's first expression, a conditional type's check type, a tuple
/// element's `?`.
fn is_spine_link(parent: &str, key: &str, slot: u32) -> bool {
    match parent {
        "BinaryExpression" | "LogicalExpression" | "TSQualifiedName" => key == "left",
        "MemberExpression" => key == "object",
        "CallExpression" => key == "callee",
        "TaggedTemplateExpression" => key == "tag",
        "TSNonNullExpression"
        | "TSAsExpression"
        | "TSSatisfiesExpression"
        | "TSInstantiationExpression" => key == "expression",
        "TSArrayType" => key == "elementType",
        "TSIndexedAccessType" => key == "objectType",
        "ConditionalExpression" => key == "test",
        "SequenceExpression" => key == "expressions" && slot == 0,
        "TSConditionalType" => key == "checkType",
        "TSOptionalType" => key == "typeAnnotation",
        _ => false,
    }
}

/// Whether a wire node is the writer's alone, emitted inline by the iterative walk that
/// writes its child and absent from the tree every other walk reads — so no walk recurses
/// through it: an optional chain's `ChainExpression`.
fn is_transparent(ty: &str) -> bool {
    ty == "ChainExpression"
}

/// A streaming walk of compact wire JSON: for each `type`-bearing object, its depth with
/// spine links taken out and the spine links on its path, maximized over the document.
fn walk_wire(bytes: &[u8]) -> WireCounts {
    #[derive(Clone)]
    struct Node {
        ty: String,
        nesting: u32,
        spine: u32,
        // a transparent node stands in for its parent: the parent's link to it, read for
        // its children
        link: Option<(String, u32)>,
    }
    struct Frame {
        object: bool,
        // the nearest node above this container, the key it sits under there, and the
        // container's element index there when that key holds an array
        above: Option<(Node, String, u32)>,
        node: Option<Node>,
        key: String,
        // an array's current element index
        index: u32,
    }
    let mut counts = WireCounts::default();
    let mut stack: Vec<Frame> = Vec::new();
    let mut expect_key = false;
    let mut i = 0;
    let read_string = |i: &mut usize| -> String {
        *i += 1;
        let start = *i;
        while bytes[*i] != b'"' {
            if bytes[*i] == b'\\' {
                *i += 1;
            }
            *i += 1;
        }
        let s = String::from_utf8_lossy(&bytes[start..*i]).into_owned();
        *i += 1;
        s
    };
    while i < bytes.len() {
        match bytes[i] {
            b',' => {
                if let Some(frame) = stack.last_mut() {
                    expect_key = frame.object;
                    frame.index += 1;
                }
                i += 1;
            }
            b':' | b' ' | b'\n' => i += 1,
            b'{' | b'[' => {
                let above = stack.last().and_then(|f| match &f.node {
                    Some(node) => Some((node.clone(), f.key.clone(), 0)),
                    None => f.above.clone().map(|(node, key, slot)| {
                        (node, key, if f.object { slot } else { f.index })
                    }),
                });
                stack.push(Frame {
                    object: bytes[i] == b'{',
                    above,
                    node: None,
                    key: String::new(),
                    index: 0,
                });
                expect_key = bytes[i] == b'{';
                i += 1;
            }
            b'}' | b']' => {
                stack.pop();
                expect_key = false;
                i += 1;
            }
            b'"' => {
                let s = read_string(&mut i);
                let frame = stack.last_mut().expect("inside a container");
                if expect_key {
                    frame.key = s;
                    expect_key = false;
                } else if frame.object && frame.key == "type" && frame.node.is_none() {
                    let node = match &frame.above {
                        None => Node {
                            ty: s,
                            nesting: 1,
                            spine: 0,
                            link: None,
                        },
                        Some((parent, key, slot)) => {
                            let (key, slot) =
                                parent.link.clone().unwrap_or_else(|| (key.clone(), *slot));
                            if is_transparent(&s) {
                                Node {
                                    link: Some((key, slot)),
                                    ..parent.clone()
                                }
                            } else {
                                let link = is_spine_link(&parent.ty, &key, slot);
                                Node {
                                    nesting: parent.nesting + u32::from(!link),
                                    spine: parent.spine + u32::from(link),
                                    ty: s,
                                    link: None,
                                }
                            }
                        }
                    };
                    counts.nesting = counts.nesting.max(node.nesting);
                    counts.spine = counts.spine.max(node.spine);
                    counts.combined = counts
                        .combined
                        .max(node.spine + node.nesting * NESTING_SPINE_WEIGHT);
                    frame.node = Some(node);
                }
            }
            _ => {
                while i < bytes.len() && !matches!(bytes[i], b',' | b'}' | b']') {
                    i += 1;
                }
            }
        }
    }
    counts
}

/// What a parse made of a source.
enum Verdict {
    Accepted,
    /// Refused by a depth limit, with the error's message.
    TooDeep(String),
    /// Refused by the grammar.
    Invalid(String),
}

fn verdict(lang: Lang, source: &str) -> Verdict {
    let arena = bumpalo::Bump::new();
    let result = match lang {
        Lang::Ts => tsv_ts::parse(source, &arena).map(drop),
        Lang::Css => tsv_css::parse(source, &arena).map(drop),
        Lang::Svelte => tsv_svelte::parse(source, &arena).map(drop),
    };
    match result {
        Ok(()) => Verdict::Accepted,
        Err(e) if e.is_too_deep() => Verdict::TooDeep(e.to_string()),
        Err(e) => Verdict::Invalid(e.to_string()),
    }
}

/// The largest `n` the parse of `shape` accepts — and the refusal one past it, which must
/// be the depth error naming the shape's limit. `None` for a generated combination the
/// grammar rejects at its shallowest repetitions (`a?.b`t``, `a ?? a || a`), which the sweep
/// skips; a named shape must parse, and a refusal past those repetitions must be the depth
/// limit's.
fn accepted_limit(shape: &'static Shape) -> Option<usize> {
    on_thread(format!("search {}", shape.name), SEARCH_STACK, move || {
        let accepts = |n: usize| match verdict(shape.lang, &shape.make(n)) {
            Verdict::Accepted => true,
            Verdict::TooDeep(_) => false,
            Verdict::Invalid(error) => panic!(
                "{}: n={n} is refused by the grammar, not a depth limit:\n{error}",
                shape.name
            ),
        };
        for n in [1, 2] {
            if let Verdict::Invalid(error) = verdict(shape.lang, &shape.make(n)) {
                assert!(
                    shape.generated(),
                    "{}: n={n} must parse:\n{error}",
                    shape.name
                );
                return None;
            }
        }
        // every unit holds a level or a spine unit, so no limit passes this: a shape that
        // reaches it never meets one, and searching on would parse ever larger inputs
        let mut hi = 2;
        while accepts(hi) {
            hi *= 2;
            assert!(hi <= 1 << 14, "{}: no depth refused", shape.name);
        }
        let mut lo = hi / 2;
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if accepts(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        assert!(lo >= 1, "{}: the shallowest input is refused", shape.name);
        // the search ends on a depth refusal: `accepts` panics on any other
        let Verdict::TooDeep(error) = verdict(shape.lang, &shape.make(lo + 1)) else {
            panic!("{}: one past the limit is accepted", shape.name)
        };
        let nesting = format!("depth limit of {NESTING_LIMIT} levels");
        let refused_by_limit = match shape.limit {
            Limit::Nesting => error.contains(&nesting),
            Limit::Spine => error.contains("length limit"),
            Limit::Either | Limit::Any | Limit::PairedChain => true,
        };
        assert!(
            refused_by_limit,
            "{}: one past the limit ({}) must be refused by the shape's own limit, not:\n{error}",
            shape.name,
            lo + 1,
        );
        Some(lo)
    })
}

/// The shapes a run grades: every one in an optimized build, where the stack legs grade
/// the shipped contract; in a debug build the named shapes and every
/// [`DEBUG_SWEEP_STRIDE`]th generated one, since the debug contract's margin is wide and
/// an unoptimized format of the whole sweep would hold `cargo test` for minutes.
///
/// `TSV_DEPTH_FILTER` narrows a run to the shapes whose name holds it — for chasing one
/// shape, never for a gate run; it must name at least one.
fn graded_shapes() -> Vec<&'static Shape> {
    // the stack knob is read up front, so a bad value fails the run before any shape
    gate_stack();
    let mut generated = 0;
    let filter = knob("TSV_DEPTH_FILTER");
    let graded: Vec<_> = shapes(Scope::from_env())
        .iter()
        .filter(|shape| {
            filter
                .as_ref()
                .is_none_or(|f| shape.name.contains(f.as_str()))
        })
        .filter(|shape| {
            !shape.generated() || {
                generated += 1;
                !cfg!(debug_assertions) || generated % DEBUG_SWEEP_STRIDE == 0
            }
        })
        .collect();
    assert!(
        !graded.is_empty(),
        "TSV_DEPTH_FILTER={filter:?} names no shape (a debug build grades only every \
         {DEBUG_SWEEP_STRIDE}th generated shape, so a generated name may be one it skips)"
    );
    graded
}

/// The stride of the debug build's sample of the generated sweep.
const DEBUG_SWEEP_STRIDE: usize = 16;

/// Run `grade` over `shapes` on a few worker threads, each shape's own walks on threads of
/// their own; every failure is collected and reported together. A stack overflow still
/// aborts the binary, and the runtime names the thread it overflowed (`contract <shape>`).
fn grade_in_parallel<T: Send + 'static>(
    shapes: Vec<&'static Shape>,
    grade: fn(&'static Shape) -> T,
) -> Vec<(&'static Shape, T)> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let workers = thread::available_parallelism()
        .map_or(4, std::num::NonZero::get)
        .clamp(1, 8);
    let shapes: &'static [&'static Shape] = Box::leak(shapes.into_boxed_slice());
    let next: &'static AtomicUsize = Box::leak(Box::new(AtomicUsize::new(0)));
    let results: &'static Mutex<Vec<(&'static Shape, T)>> =
        Box::leak(Box::new(Mutex::new(Vec::new())));
    let failures: &'static Mutex<Vec<String>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let handles: Vec<_> = (0..workers)
        .map(|_| {
            thread::spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&shape) = shapes.get(i) else { break };
                    match std::panic::catch_unwind(|| grade(shape)) {
                        Ok(value) => results.lock().expect("results").push((shape, value)),
                        Err(panic) => {
                            let message = panic
                                .downcast_ref::<String>()
                                .cloned()
                                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                                .unwrap_or_default();
                            failures
                                .lock()
                                .expect("failures")
                                .push(format!("{}: {message}", shape.name));
                        }
                    }
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("worker");
    }
    let failures = failures.lock().expect("failures");
    assert!(
        failures.is_empty(),
        "{} shapes failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    let mut results = std::mem::take(&mut *results.lock().expect("results"));
    results.sort_by_key(|(shape, _)| std::ptr::from_ref(*shape) as usize);
    results
}

/// Every walk over the accepted tree, on the contract's stack.
fn run_every_phase(shape: &'static Shape, n: usize) {
    run_walks(shape.name.clone(), shape.lang, shape.make(n));
}

/// Parse, both wire writers, format and — TypeScript — the type strip over `source`, on a
/// thread of the contract's stack named `contract <name>`.
fn run_walks(name: String, lang: Lang, source: String) {
    on_thread(format!("contract {name}"), gate_stack(), move || {
        let arena = bumpalo::Bump::new();
        match lang {
            Lang::Ts => {
                let program = tsv_ts::parse(&source, &arena).expect("parses");
                assert!(!tsv_ts::convert_ast_json_bytes(&program, &source).is_empty());
                assert!(
                    !tsv_ts::convert_ast_json_bytes_with_locations(&program, &source).is_empty()
                );
                let formatted = tsv_ts::format_str(&source).expect("formats");
                assert_output_reparses(&name, Lang::Ts, &formatted);
                let strip_arena = bumpalo::Bump::new();
                // a refusal (a value namespace has runtime semantics) is a verdict, not a
                // failure: the walk reached it
                let stripped = tsv_ts::strip(&source, &strip_arena);
                assert!(
                    matches!(
                        stripped,
                        Ok(_) | Err(tsv_ts::StripError::Unsupported { .. })
                    ),
                    "{name}: the strip failed: {stripped:?}",
                );
            }
            Lang::Css => {
                let sheet = tsv_css::parse(&source, &arena).expect("parses");
                assert!(!tsv_css::convert_ast_json_bytes(&sheet, &source).is_empty());
                assert!(
                    !tsv_css::convert_ast_json_bytes_with_locations(&sheet, &source).is_empty()
                );
                let formatted = tsv_css::format_str(&source).expect("formats");
                assert_output_reparses(&name, Lang::Css, &formatted);
            }
            Lang::Svelte => {
                let root = tsv_svelte::parse(&source, &arena).expect("parses");
                assert!(!tsv_svelte::convert_ast_json_bytes(&root, &source).is_empty());
                assert!(
                    !tsv_svelte::convert_ast_json_bytes_with_locations(&root, &source).is_empty()
                );
                let formatted = tsv_svelte::format_str(&source).expect("formats");
                assert_output_reparses(&name, Lang::Svelte, &formatted);
            }
        }
    });
}

/// The format of an accepted input must parse again and be its own format (F1): every pair
/// the printer adds is a level of nesting in the output, so the parse charges each one
/// while reading the input (`tsv_lang::SpineCount::try_pair`) — and an output past the
/// limit, refused by its own reparse, fails here.
fn assert_output_reparses(name: &str, lang: Lang, formatted: &str) {
    if let Err(error) = parse(lang, formatted) {
        panic!("{name}: the format's output does not parse again:\n{error}");
    }
    let again = match lang {
        Lang::Ts => tsv_ts::format_str(formatted),
        Lang::Css => tsv_css::format_str(formatted),
        Lang::Svelte => tsv_svelte::format_str(formatted),
    }
    .expect("the output formats");
    assert!(
        again == formatted,
        "{name}: the format's output is not its own format"
    );
}

/// Every shape: accepted at its limit, refused past it, calibrated against its wire, and
/// every walk of the accepted tree within the contract's stack.
#[test]
fn every_shape_holds_its_limit_on_the_contract_stack() {
    let graded = grade_in_parallel(graded_shapes(), |shape| {
        if std::env::var_os("TSV_DEPTH_REPORT").is_some() {
            eprintln!("start {}", shape.name);
        }
        let n = accepted_limit(shape)?;
        check_wire(shape, n);
        run_every_phase(shape, n);
        if std::env::var_os("TSV_DEPTH_REPORT").is_some() {
            eprintln!("done {}", shape.name);
        }
        Some(n)
    });
    let mut skipped = 0;
    for (shape, n) in &graded {
        match n {
            Some(n) if !shape.generated() => eprintln!("{:40} n={n:5}", shape.name),
            Some(_) => {}
            None => skipped += 1,
        }
    }
    let generated = graded.iter().filter(|(shape, _)| shape.generated()).count();
    eprintln!(
        "generated sweep: {} combinations graded, {skipped} the grammar rejects",
        generated - skipped
    );
    // the grammar rejects a minority of the combinations (an optional chain's tag, `??`
    // beside `||`, an `as` read on into a type): a skip count near the sweep's size would
    // mean the shapes stopped parsing, and the sweep stopped grading anything. A run with
    // no generated shape at all graded none — only a narrowed one may
    let narrowed = std::env::var_os("TSV_DEPTH_FILTER").is_some();
    assert!(
        (generated > 0 || narrowed) && skipped * 3 < generated.max(1),
        "{skipped} of {generated} generated combinations do not parse"
    );
}

/// The levels of document frame a wire may hold above or below what the count charges —
/// the root and the statement around the shape, the leaf at its bottom.
const FRAME_LEVELS: u32 = 4;

/// The wire of a shape at its accepted depth, against the count ([`Wire`]).
///
/// Every shape: the count charges at least what the walks recurse through, so no path
/// through the wire nests deeper than [`NESTING_LIMIT`], holds more spine links than
/// [`SPINE_LIMIT`] or costs more than it in spine units, nesting charged at
/// [`NESTING_SPINE_WEIGHT`] a level ([`WireCounts`]) — [`FRAME_LEVELS`] in hand. A construct
/// the parser leaves uncounted puts a wire level on the path each repetition and passes
/// this many times over.
///
/// A calibrated shape's count must also sit AT the limit: the window is the levels the
/// document's frame adds around the shape on one side; on the spine's, the nesting every
/// chain is charged, which its budget pays for too, less one repetition of the shape's unit
/// (the search steps a whole repetition at a time, up to five links), give or take a link
/// (the head may count none, or count a cheaper link than the rest, which the wire count,
/// at the shape's units a link, does not see).
fn check_wire(shape: &'static Shape, n: usize) {
    let source = shape.make(n);
    let counts = on_thread(format!("wire {}", shape.name), SEARCH_STACK, move || {
        wire_counts(shape.lang, &source)
    });
    if std::env::var_os("TSV_DEPTH_REPORT").is_some() {
        eprintln!(
            "{:60} n={n:5} wire nesting={:4} spine={:5} units={:5}",
            shape.name, counts.nesting, counts.spine, counts.combined
        );
    }
    let bounds = [
        ("nesting", counts.nesting, NESTING_LIMIT + FRAME_LEVELS),
        ("spine links", counts.spine, SPINE_LIMIT),
        (
            "spine units",
            counts.combined,
            SPINE_LIMIT + FRAME_LEVELS * NESTING_SPINE_WEIGHT,
        ),
    ];
    for (what, counted, bound) in bounds {
        assert!(
            counted <= bound,
            "{}: the count charges less than the walks recurse through — accepted at \
             n={n}, whose wire's {what} reaches {counted}, past {bound}",
            shape.name,
        );
    }
    let charged = NESTING_FLOOR * NESTING_SPINE_WEIGHT;
    let (units, links, low, high) = match shape.wire {
        Wire::Nesting => (
            1,
            1,
            NESTING_LIMIT - FRAME_LEVELS,
            NESTING_LIMIT + FRAME_LEVELS,
        ),
        Wire::Spine(units, links) => (
            units,
            links,
            SPINE_LIMIT - charged - units * 6,
            SPINE_LIMIT - charged + units,
        ),
        Wire::Bounded => return,
    };
    let counted = if shape.wire == Wire::Nesting {
        counts.nesting
    } else {
        counts.spine * units / links
    };
    assert!(
        (low..=high).contains(&counted),
        "{}: the counter has drifted from the wire — accepted at n={n}, whose wire \
         counts {counted}, outside {low}..={high}",
        shape.name,
    );
}

/// The spec line of every shape at its accepted depth, written to the path
/// `TSV_DEPTH_SHAPES` names — what `scripts/depth_wasm.ts` (`deno task test:depth:wasm`)
/// regenerates each shape from, to run it through the WASM packages at that depth and one
/// past it. A tab-separated line per accepted shape: name, language, `n`, then the spec's
/// five parts, the name and the parts with `\\`, `\t` and `\n` escaped. Ignored by a plain
/// run, which has no path to write to.
#[test]
#[ignore = "run by `deno task test:depth:wasm`, which names the output path"]
fn write_shape_limits() {
    let path = std::env::var("TSV_DEPTH_SHAPES").expect("TSV_DEPTH_SHAPES names the output");
    let limits = grade_in_parallel(shapes(Scope::Full).iter().collect(), accepted_limit);
    let escape = |s: &str| {
        s.replace('\\', "\\\\")
            .replace('\t', "\\t")
            .replace('\n', "\\n")
    };
    let mut out = String::new();
    for (shape, n) in limits {
        let Some(n) = n else { continue };
        writeln!(
            out,
            "{}\t{}\t{n}\t{}\t{}\t{}\t{}\t{}",
            escape(&shape.name),
            shape.lang.name(),
            escape(&shape.pre),
            escape(&shape.unit),
            escape(&shape.mid),
            escape(&shape.close),
            escape(&shape.post),
        )
        .expect("write to a string");
    }
    std::fs::write(&path, out).expect("write the shape limits");
}

/// A nested `<script>` or `<style>` is raw text to the parser — the printer parses its
/// body — so no parse refuses one, however deep. The printer's parse starts at the markup's
/// depth, and a body past the budget keeps its author's bytes rather than recursing.
#[test]
fn a_nested_raw_text_body_past_the_budget_prints_verbatim_on_the_contract_stack() {
    on_thread("contract raw text".to_owned(), gate_stack(), || {
        // 200 wire levels of markup, which the document parse accepts; each body takes its
        // island past the budget
        let elements = 100;
        for (open, body, close) in [
            (
                "<style>",
                format!("{}color: red;{}", rep("a { ", 100), rep("}", 100)),
                "</style>",
            ),
            (
                "<script>",
                format!("x = {}1{};", rep("[", 200), rep("]", 200)),
                "</script>",
            ),
        ] {
            let source = format!(
                "{}{open}{body}{close}{}\n",
                rep("<div>", elements),
                rep("</div>", elements)
            );
            let formatted = tsv_svelte::format_str(&source).expect("formats");
            assert!(formatted.contains(&body), "{open}: the body kept its bytes");
        }
    });
}

/// The spine count bounds a path, not a document: a long list of short chains — a data
/// table, a minified sequence, a long body — is charged one element's chains, never their
/// total. Each source holds thousands of siblings of a few links each, several times the
/// spine limit if summed — and every walk over it, which iterates a list rather than
/// recursing down it, fits the contract's stack.
#[test]
fn sibling_chains_are_charged_apart() {
    let n = 10_000;
    let item = "a.b.c()";
    let list = |sep: &str| vec![item; n].join(sep);
    for (name, lang, source) in [
        ("array", Lang::Ts, format!("x = [{}];\n", list(", "))),
        ("arguments", Lang::Ts, format!("f({});\n", list(", "))),
        (
            "object",
            Lang::Ts,
            format!("x = {{{}}};\n", vec!["k: a.b.c()"; n].join(", ")),
        ),
        ("sequence", Lang::Ts, format!("x = ({});\n", list(", "))),
        (
            "declarators",
            Lang::Ts,
            format!("var {};\n", vec!["k = a.b.c()"; n].join(", ")),
        ),
        // an operator chain is itself a spine, so it stays under the limit: its operands'
        // chains, summed, would pass it several times over
        (
            "operands",
            Lang::Ts,
            format!("x = {};\n", vec![item; 3_000].join(" + ")),
        ),
        ("statements", Lang::Ts, list(";\n")),
        (
            "class members",
            Lang::Ts,
            format!("class A {{ {} }}\n", vec!["k = a.b.c();"; n].join(" ")),
        ),
        (
            "type members",
            Lang::Ts,
            format!("type T = {{ {} }};\n", vec!["k: A.B.C;"; n].join(" ")),
        ),
        (
            "union",
            Lang::Ts,
            format!("type T = {};\n", vec!["A.B.C"; n].join(" | ")),
        ),
        (
            "switch cases",
            Lang::Ts,
            format!(
                "switch (x) {{ {} }}\n",
                vec!["case a.b.c(): break;"; n].join(" ")
            ),
        ),
        (
            "enum members",
            Lang::Ts,
            format!("enum E {{ {} }}\n", vec!["K = a.b.c()"; n].join(", ")),
        ),
        (
            "template expressions",
            Lang::Svelte,
            format!("<p>{}</p>\n", vec!["{a.b.c()}"; n].join(" ")),
        ),
        (
            "template literal",
            Lang::Ts,
            format!("x = `{}`;\n", vec!["${a.b.c()}"; n].join(" ")),
        ),
        (
            "tagged template",
            Lang::Ts,
            format!("x = t`{}`;\n", vec!["${a.b.c()}"; n].join(" ")),
        ),
        (
            "parameter defaults",
            Lang::Ts,
            format!(
                "function f({}) {{}}\n",
                (0..n)
                    .map(|i| format!("p{i} = a.b.c()"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        (
            "arrow parameter defaults",
            Lang::Ts,
            format!(
                "x = ({}) => 1;\n",
                (0..n)
                    .map(|i| format!("p{i} = a.b.c()"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        (
            "call type arguments",
            Lang::Ts,
            format!("f<{}>();\n", vec!["A.B.C[]"; n].join(", ")),
        ),
        (
            "type arguments",
            Lang::Ts,
            format!("type T = G<{}>;\n", vec!["A.B.C[]"; n].join(", ")),
        ),
        (
            "tuple",
            Lang::Ts,
            format!("type T = [{}];\n", vec!["A.B.C[]"; n].join(", ")),
        ),
        (
            "intersection",
            Lang::Ts,
            format!("type T = {};\n", vec!["A.B.C[]"; n].join(" & ")),
        ),
        (
            "implements",
            Lang::Ts,
            format!("class A implements {} {{}}\n", vec!["A.B.C"; n].join(", ")),
        ),
        (
            "interface extends",
            Lang::Ts,
            format!("interface A extends {} {{}}\n", vec!["A.B.C"; n].join(", ")),
        ),
        (
            "type parameter defaults",
            Lang::Ts,
            format!(
                "type T<{}> = 1;\n",
                (0..n)
                    .map(|i| format!("P{i} = A.B.C[]"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        (
            "function type parameters",
            Lang::Ts,
            format!(
                "type T = ({}) => 1;\n",
                (0..n)
                    .map(|i| format!("p{i}: A.B.C[]"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        (
            "template literal type",
            Lang::Ts,
            format!("type T = `{}`;\n", vec!["${A.B.C[]}"; n].join(" ")),
        ),
    ] {
        let parsed = on_thread(format!("siblings {name}"), SEARCH_STACK, {
            let source = source.clone();
            move || parse(lang, &source)
        });
        assert!(
            parsed.is_ok(),
            "{name}: siblings were charged together: {parsed:?}"
        );
        // and the walks over a long list iterate it: every one fits the contract's stack
        run_walks(format!("siblings {name}"), lang, source);
    }
}

/// The nesting a chain is charged is its own statement's: a deep statement ahead of a long
/// chain costs the chain nothing, so a document parses whatever order its statements take.
/// The chain holds more links than the deepest nesting leaves room for beside it, and fewer
/// than a chain on a shallow path may hold.
#[test]
fn the_nesting_charged_to_a_chain_is_its_own_statements() {
    let deep = format!("x = {}1{};\n", rep("[", 240), rep("]", 240));
    let long = format!("x = {};\n", shapes::chain("a", " + ", 3_000));
    // a deep element ahead of a long one, as siblings in a long list
    let elements = format!(
        "x = [{}, {}, {}];\n",
        ["0"; 10].join(", "),
        format_args!("{}1{}", rep("[", 240), rep("]", 240)),
        shapes::chain("a", " + ", 3_000)
    );
    for (name, source) in [
        ("deep then long", format!("{deep}{long}")),
        ("long then deep", format!("{long}{deep}")),
        (
            "deep then long, in a body",
            format!("function f() {{ {deep}{long} }}\n"),
        ),
        ("deep element then long", elements),
    ] {
        let parsed = on_thread(format!("order {name}"), SEARCH_STACK, move || {
            parse(Lang::Ts, &source)
        });
        assert!(parsed.is_ok(), "{name}: {parsed:?}");
    }
    // the same chain on the deep path is still charged that path's nesting
    let nested = format!(
        "x = {}{}{};\n",
        rep("[", 240),
        shapes::chain("a", " + ", 3_000),
        rep("]", 240)
    );
    let parsed = on_thread("order nested".to_owned(), SEARCH_STACK, move || {
        parse(Lang::Ts, &nested)
    });
    assert!(
        parsed.is_err_and(|e| e.contains("length limit")),
        "a chain on a deep path is charged its nesting"
    );
}

/// A CSS value's functions are counted as its parser enters them (the parser cannot fail,
/// so the count rides beside it), and the refusal is placed at the `(` of the function that
/// passes the limit, not at the value's start — an unquoted `url(`'s interior parens
/// included, which the lexer's url-token steps over whole.
#[test]
fn a_css_value_past_the_limit_is_refused_at_the_paren_that_passes_it() {
    for (pre, unit, mid) in [("", "calc(", "1px"), ("url(", "(", "x")] {
        let source = format!(
            "a {{ width: {pre}{}{mid}{}{}; }}\n",
            rep(unit, 300),
            rep(")", 300),
            if pre.is_empty() { "" } else { ")" }
        );
        let arena = bumpalo::Bump::new();
        let error = tsv_css::parse(&source, &arena).expect_err("refused");
        assert!(error.is_too_deep(), "{error}");
        let at = error.position().expect("located");
        let value_start = source.find(": ").expect("value") + 2;
        assert_eq!(&source[at..=at], "(", "{error}");
        // the rule and its block take two levels, so the paren that passes the limit is
        // the one that leaves fewer than two in hand
        let open_parens = source[..=at].matches('(').count();
        assert!(
            at > value_start && open_parens == NESTING_LIMIT as usize - 1,
            "{pre}{unit}: refused at paren {open_parens}: {error}"
        );
    }
}

/// A module the depth limit refuses is not retried as a script: the script grammar reads
/// every construct the module one accepts as the same tree but `await`, so a retry could
/// only succeed by reading the source differently (`await <T>a` as `await < T > a`) — a
/// reading chosen by the limit, which the same text one level shallower never gets.
#[test]
fn a_module_past_the_limit_is_not_read_as_a_script() {
    let source = format!("x = {}a;\n", rep("await <T>", 200));
    let error = on_thread("fallback".to_owned(), SEARCH_STACK, move || {
        tsv_ts::format_str(&source).expect_err("refused")
    });
    assert!(error.is_too_deep(), "{error}");
}
