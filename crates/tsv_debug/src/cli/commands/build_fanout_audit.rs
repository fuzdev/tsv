use argh::FromArgs;

use crate::cli::CliError;
use tsv_cli::cli::input::ParserType;
use tsv_lang::doc::arena::DocArena;
use tsv_lang::estimated_ast_arena_capacity;

/// Audit for super-linear doc-node fanout — the per-layout-candidate rebuild
/// blowup.
///
/// Builds synthetic nested inputs at increasing depth, formats each into a
/// fresh `DocArena`, and checks that the doc-node count grows roughly linearly
/// with nesting depth. A builder that assembles `conditional_group` candidates
/// by *re-invoking the recursive builder* on the same nodes (instead of
/// building once and reusing the `DocId`) makes the count grow exponentially in
/// depth; this catches that and guards against reintroduction. The doc-node
/// count is read directly via `format_in` into a caller-owned arena +
/// `borrow_nodes().len()` — no prod-code instrumentation. Pure Rust, no Deno.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "build_fanout_audit")]
pub struct BuildFanoutAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,
}

/// Stop deepening a construct once one format exceeds this many doc nodes, so
/// the audit itself never OOMs on the (currently exponential) code. Exceeding
/// it is treated as a failure (clearly super-linear).
const NODE_CAP: usize = 3_000_000;

/// Tolerated growth: `doc_nodes ~ depth^E`. A linear build is `E ≈ 1`; this
/// allows up to cubic. The rebuild blowup pushes `E` far past it, so the bound
/// is generous — a fixed (linear) build always passes.
const MAX_EXPONENT: f64 = 3.0;

type Gen = fn(usize) -> String;

struct Construct {
    name: &'static str,
    parser: ParserType,
    generate: Gen,
    depths: &'static [usize],
}

/// Nested block elements with mixed content (a `<span>` sibling per level) —
/// routes through `build_content_element_doc`'s `(Hard,Hard)` double-build.
fn gen_svelte_elements(depth: usize) -> String {
    let opens = "<div class=\"l\"><span>t</span>".repeat(depth);
    let closes = "</div>".repeat(depth);
    format!("<div>{opens}{closes}</div>\n")
}

/// Nested padded **inline** elements (`<b> <i> … </i> </b>`) — routes through
/// `build_content_element_doc`'s `(_, Hug)` / standard arms, where a padded inline
/// element (`is_inline && !trim_boundaries`) selects a `trim=true` children variant.
/// Distinct from `gen_svelte_elements` (block elements, `(Hard,Hard)`): the boundary
/// whitespace is what makes these inline, and a per-arm children rebuild here recurses
/// into children that ALSO rebuild — the element/inline-side twin of the block blowup.
fn gen_svelte_inline_elements(depth: usize) -> String {
    let opens = "<b> <i> ".repeat(depth);
    let closes = " </i> </b>".repeat(depth);
    format!("{opens}text{closes}\n")
}

/// Nested `{#if}` blocks (single-child consequents) — the if-block fast path
/// (`build_if_pieces` / `compose_if_tail`), which composes both expanding-construct
/// tails from one shared set of body pieces.
fn gen_svelte_if(depth: usize) -> String {
    let opens = "{#if c}".repeat(depth);
    let closes = "{/if}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// Nested `{#each}` blocks (single-child bodies) — the each-block fast path
/// (`build_each_pieces` / `compose_each_tail`).
fn gen_svelte_each(depth: usize) -> String {
    let opens = "{#each a as b}".repeat(depth);
    let closes = "{/each}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// Nested `{#await}` blocks (single-child pending sections) — the await-block fast path
/// (`build_await_pieces` / `compose_await_tail`).
fn gen_svelte_await(depth: usize) -> String {
    let opens = "{#await p}".repeat(depth);
    let closes = "{/await}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// An inline element immediately followed (no whitespace) by a control-flow block —
/// the sibling-`>` dangle path (`try_block_sibling_gt_dangle`), which probe-builds
/// the block for a `will_break` test and then rebuilds it folded with the `>`.
fn gen_svelte_block_sibling(depth: usize) -> String {
    let opens = "<span>t</span>{#if c}".repeat(depth);
    let closes = "{/if}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// A member chain whose call argument is itself the inner chain — the axis the
/// chain printer rebuilds once per `conditional_group` candidate state. Input
/// grows linearly in depth; the buggy build is exponential.
fn gen_ts_chain(depth: usize) -> String {
    let mut expr = String::from("x");
    for _ in 0..depth {
        expr = format!("obj.method({expr}).prop");
    }
    format!("const v = {expr};\n")
}

/// Nested ternaries in the alternate (`a ? b : (c ? d : …)`) — the conditional
/// expression printer's `conditional_group` (flat vs broken chain). A per-candidate
/// re-invoke of the recursive builder on the nested conditional would be exponential.
fn gen_ts_ternary(depth: usize) -> String {
    let mut expr = String::from("z");
    for i in 0..depth {
        expr = format!("cond{i} ? val{i} : {expr}");
    }
    format!("const v = {expr};\n")
}

/// Nested conditional TYPES in the false branch (`A extends B ? C : (D extends E ? …)`)
/// — the conditional-type printer's own flat/broken group.
fn gen_ts_conditional_type(depth: usize) -> String {
    let mut ty = String::from("never");
    for i in 0..depth {
        ty = format!("T{i} extends U{i} ? R{i} : {ty}");
    }
    format!("type X = {ty};\n")
}

/// Nested call args that are themselves object literals (`f({{ a: g({{ b: h(…) }}) }})`)
/// — call-argument wrapping (`arg_wrapping`/`call_formatting`) composed with the object
/// printer, each a `conditional_group`/group with a flat-vs-expanded choice.
fn gen_ts_nested_call_obj(depth: usize) -> String {
    let mut expr = String::from("leaf");
    for i in 0..depth {
        expr = format!("fn{i}({{ key{i}: {expr} }})");
    }
    format!("const v = {expr};\n")
}

/// Nested arrow last-argument hug (`a(x => b(y => c(…)))`) — the last-arg-arrow
/// expansion path, another per-call flat-vs-expanded candidate site.
fn gen_ts_nested_arrow(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg arrow whose body is another such call — a leading arg at every
/// level routes through `try_expand_last_function_arg` (the `build_args_split_last` /
/// `build_break_body_state` expand-last path). The multi-arg twin of `gen_ts_nested_arrow`:
/// with a leading argument the whole-arrow arg doc and the break-body state each recurse
/// into the body unless the body build is shared.
fn gen_ts_nested_arrow_multiarg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(lead{i}, p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg arrow via a MEMBER callee (`p.then(a, x => q.then(a, …))`) —
/// routes through `build_chain_args_multi` (chain_args.rs), the chain variant.
fn gen_ts_nested_arrow_multiarg_chain(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("obj{i}.then(lead{i}, p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg arrow via a `new` callee — routes through new_expression.rs.
fn gen_ts_nested_arrow_multiarg_new(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("new Cls{i}(lead{i}, p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg arrow whose body is an OBJECT literal containing the next call
/// (`f(lead, x => ({{ k: f(lead, y => ({{ … }}) ) }}))`) — the arrow-object-body
/// variant (`call_formatting.rs:960` / `chain_args.rs:1131`, `d.parens(build_expression_doc(body))`).
fn gen_ts_nested_arrow_obj_multiarg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(lead{i}, p{i} => ({{ k{i}: {expr} }}))");
    }
    format!("const v = {expr};\n")
}

/// SINGLE-arg last-arg arrow whose body is an OBJECT literal containing the next call
/// (`f(x => ({{ k: f(y => ({{ … }}) ) }}))`) — the single-arg arrow-object-body path
/// (`call_formatting.rs:629` object/array hug in `try_single_arg_hug`).
fn gen_ts_nested_arrow_obj_single(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} => ({{ k{i}: {expr} }}))");
    }
    format!("const v = {expr};\n")
}

/// Nested assignment RHS (`a = (b = (c = …))`) — the assignment-layout
/// `conditional_group`/fluid-layout candidate site.
fn gen_ts_assignment_nested(depth: usize) -> String {
    let mut expr = String::from("z");
    for i in 0..depth {
        expr = format!("(v{i} = {expr})");
    }
    format!("let z = {expr};\n")
}

/// Nested ternary as the sole call ARG (`f(a ? b : f(c ? d : …))`) — ternary-in-arg
/// routes through `build_arg_expression_doc`'s conditional-with-binary-indent path.
fn gen_ts_ternary_call_arg(depth: usize) -> String {
    let mut expr = String::from("z");
    for i in 0..depth {
        expr = format!("call{i}(c{i} ? v{i} : {expr})");
    }
    format!("const v = {expr};\n")
}

/// Nested arrays each holding a callback whose body is the next array
/// (`[x => [y => [ … ]]]`) — array-of-callback expansion candidate.
fn gen_ts_nested_array_callback(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("[p{i} => {expr}]");
    }
    format!("const v = {expr};\n")
}

/// Nested `{#snippet}` blocks (each body a single child) — the snippet piece-composer.
fn gen_svelte_snippet_nested(depth: usize) -> String {
    let opens = "{#snippet s()}".repeat(depth);
    let closes = "{/snippet}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// Nested `{#key}` blocks (each body a single child) — the key piece-composer.
fn gen_svelte_key_nested(depth: usize) -> String {
    let opens = "{#key e}".repeat(depth);
    let closes = "{/key}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// MULTI-arg last-arg arrow whose body is a CONDITIONAL (ternary) that recurses in a
/// branch (`f(lead, x => (c ? f(lead, y => …) : z))`) — the expand-last conditional-body
/// sub-branch (the sibling of the call-body branch, sharing `build_break_body_state`).
fn gen_ts_nested_arrow_cond_multiarg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(lead{i}, p{i} => (c{i} ? {expr} : z{i}))");
    }
    format!("const v = {expr};\n")
}

/// SINGLE-arg last-arg arrow with a recursive CONDITIONAL body — the single-arg ternary hug
/// (`build_ternary_arrow_hug_states`), which builds the body once and reuses it across states.
fn gen_ts_nested_arrow_cond_single(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} => (c{i} ? {expr} : z{i}))");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg FUNCTION expression (block body) that recurses in its `return`
/// (`f(lead, function (x) {{ return f(lead, function (y) {{ … }}); }})`) — the block-body
/// last-arg path (`build_inline_or_expand_all`).
fn gen_ts_nested_fn_expr_multiarg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(lead{i}, function (p{i}) {{ return {expr}; }})");
    }
    format!("const v = {expr};\n")
}

struct Point {
    depth: usize,
    nodes: usize,
}

struct ConstructResult {
    name: &'static str,
    parser: ParserType,
    points: Vec<Point>,
    exceeded_cap: bool,
    error: Option<String>,
    exponent: Option<f64>,
    pass: bool,
}

/// Format `source` into a fresh arena and return the number of doc nodes built
/// (including any wasted per-candidate rebuilds).
fn doc_node_count(source: &str, parser: ParserType) -> Result<usize, String> {
    let bump = bumpalo::Bump::with_capacity(estimated_ast_arena_capacity(source.len()));
    let doc_arena = DocArena::for_source(source);
    let mut interner = tsv_lang::Interner::new();
    match parser {
        ParserType::Svelte => {
            let ast =
                tsv_svelte::parse(source, &bump, &mut interner).map_err(|e| format!("{e}"))?;
            let _ = tsv_svelte::format_in(&ast, source, &doc_arena, &interner);
        }
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(source, &bump, &mut interner).map_err(|e| format!("{e}"))?;
            let _ = tsv_ts::format_in(&ast, source, &doc_arena, &interner);
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &bump).map_err(|e| format!("{e}"))?;
            let _ = tsv_css::format_in(&ast, source, &doc_arena);
        }
    }
    Ok(doc_arena.borrow_nodes().len())
}

/// Measure one construct across its depth sweep (ascending, aborting once the
/// node cap is hit) and decide pass/fail from the growth exponent.
fn measure(c: &Construct) -> ConstructResult {
    let mut points = Vec::new();
    let mut exceeded_cap = false;
    let mut error = None;
    for &d in c.depths {
        let src = (c.generate)(d);
        match doc_node_count(&src, c.parser) {
            Ok(n) => {
                points.push(Point { depth: d, nodes: n });
                if n > NODE_CAP {
                    exceeded_cap = true;
                    break;
                }
            }
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    // doc_nodes ~ depth^E  ⇒  E = log_(d_hi/d_lo)(n_hi/n_lo). Counts are bounded
    // by NODE_CAP and depths are tiny, so the f64 conversions are exact.
    #[allow(clippy::cast_precision_loss)]
    let exponent = match (points.first(), points.last()) {
        (Some(lo), Some(hi)) if points.len() >= 2 => {
            Some((hi.nodes as f64 / lo.nodes as f64).log(hi.depth as f64 / lo.depth as f64))
        }
        _ => None,
    };
    let pass = error.is_none() && !exceeded_cap && exponent.is_some_and(|e| e <= MAX_EXPONENT);
    ConstructResult {
        name: c.name,
        parser: c.parser,
        points,
        exceeded_cap,
        error,
        exponent,
        pass,
    }
}

fn parser_label(p: ParserType) -> &'static str {
    match p {
        ParserType::Svelte => "svelte",
        ParserType::TypeScript => "ts",
        ParserType::Css => "css",
    }
}

impl BuildFanoutAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let constructs = [
            Construct {
                name: "svelte_elements",
                parser: ParserType::Svelte,
                generate: gen_svelte_elements,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "svelte_inline_elements",
                parser: ParserType::Svelte,
                generate: gen_svelte_inline_elements,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "svelte_if_nested",
                parser: ParserType::Svelte,
                generate: gen_svelte_if,
                depths: &[3, 6, 9],
            },
            Construct {
                name: "svelte_each_nested",
                parser: ParserType::Svelte,
                generate: gen_svelte_each,
                depths: &[3, 6, 9],
            },
            Construct {
                name: "svelte_await_nested",
                parser: ParserType::Svelte,
                generate: gen_svelte_await,
                depths: &[3, 6, 9],
            },
            Construct {
                name: "svelte_block_sibling",
                parser: ParserType::Svelte,
                generate: gen_svelte_block_sibling,
                depths: &[3, 6, 9],
            },
            Construct {
                name: "ts_call_chain",
                parser: ParserType::TypeScript,
                generate: gen_ts_chain,
                depths: &[2, 4, 6],
            },
            Construct {
                name: "ts_ternary_nested",
                parser: ParserType::TypeScript,
                generate: gen_ts_ternary,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_conditional_type_nested",
                parser: ParserType::TypeScript,
                generate: gen_ts_conditional_type,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_call_obj",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_call_obj,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow_multiarg",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_multiarg,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow_multiarg_chain",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_multiarg_chain,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow_multiarg_new",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_multiarg_new,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow_obj_multiarg",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_obj_multiarg,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow_obj_single",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_obj_single,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_assignment_nested",
                parser: ParserType::TypeScript,
                generate: gen_ts_assignment_nested,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_ternary_call_arg",
                parser: ParserType::TypeScript,
                generate: gen_ts_ternary_call_arg,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_array_callback",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_array_callback,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "svelte_snippet_nested",
                parser: ParserType::Svelte,
                generate: gen_svelte_snippet_nested,
                depths: &[3, 6, 9],
            },
            Construct {
                name: "svelte_key_nested",
                parser: ParserType::Svelte,
                generate: gen_svelte_key_nested,
                depths: &[3, 6, 9],
            },
            Construct {
                name: "ts_nested_arrow_cond_multiarg",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_cond_multiarg,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_arrow_cond_single",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_arrow_cond_single,
                depths: &[4, 8, 12],
            },
            Construct {
                name: "ts_nested_fn_expr_multiarg",
                parser: ParserType::TypeScript,
                generate: gen_ts_nested_fn_expr_multiarg,
                depths: &[4, 8, 12],
            },
        ];

        let results: Vec<ConstructResult> = constructs.iter().map(measure).collect();
        let failed = results.iter().filter(|r| !r.pass).count();

        if self.json {
            print_json(&results);
        } else {
            print_human(&results, failed);
        }

        if failed > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

fn verdict(r: &ConstructResult) -> String {
    if r.pass {
        "PASS".to_string()
    } else if let Some(e) = &r.error {
        format!("FAIL (error: {e})")
    } else if r.exceeded_cap {
        format!("FAIL (exceeded {NODE_CAP} doc nodes — exponential)")
    } else if let Some(exp) = r.exponent {
        format!("FAIL (growth exponent {exp:.1} > {MAX_EXPONENT:.1} — rebuilt per candidate)")
    } else {
        "FAIL (inconclusive)".to_string()
    }
}

fn print_human(results: &[ConstructResult], failed: usize) {
    println!("build-fanout audit — doc-node count growth vs nesting depth\n");
    for r in results {
        let trail = r
            .points
            .iter()
            .map(|p| format!("{}:{}", p.depth, p.nodes))
            .collect::<Vec<_>>()
            .join(" → ");
        let exp = r
            .exponent
            .map_or_else(|| "—".to_string(), |e| format!("{e:.2}"));
        println!(
            "  {:<17} ({:<6}) depth:nodes {:<28} exponent {:<5} {}",
            r.name,
            parser_label(r.parser),
            trail,
            exp,
            verdict(r),
        );
    }
    println!();
    if failed > 0 {
        println!(
            "FAIL: {failed}/{} constructs show super-linear doc-node fanout (limit exponent {MAX_EXPONENT:.1}).",
            results.len()
        );
        println!("Cause: a builder rebuilds a child subtree once per layout candidate (e.g.");
        println!("each `conditional_group` state) instead of building it once and reusing the");
        println!("DocId. The doc IR decides flat-vs-broken at render, so one build suffices.");
    } else {
        println!(
            "OK: all {} constructs build O(1) docs per node (no per-candidate rebuild).",
            results.len()
        );
    }
}

fn print_json(results: &[ConstructResult]) {
    let arr: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "parser": parser_label(r.parser),
                "points": r.points.iter().map(|p| serde_json::json!({"depth": p.depth, "nodes": p.nodes})).collect::<Vec<_>>(),
                "exceeded_cap": r.exceeded_cap,
                "error": r.error,
                "exponent": r.exponent,
                "pass": r.pass,
            })
        })
        .collect();
    let out = serde_json::json!({
        "max_exponent": MAX_EXPONENT,
        "node_cap": NODE_CAP,
        "constructs": arr,
        "pass": results.iter().all(|r| r.pass),
    });
    match serde_json::to_string_pretty(&out) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("fanout audit: JSON serialize error: {e}"),
    }
}
