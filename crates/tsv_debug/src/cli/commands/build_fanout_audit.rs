use std::panic::{AssertUnwindSafe, catch_unwind};

use argh::FromArgs;

use crate::audit::panic_hook::{SuppressedPanicHook, panic_message};
use crate::audit::ratchet::{Ratchet, print_ratchet_skipped, refuse_narrowed_update};
use crate::cli::CliError;
use crate::cli::commands::profile::lang_token;
use tsv_cli::cli::input::ParserType;
use tsv_lang::doc::arena::DocArena;
use tsv_lang::estimated_ast_arena_capacity;
use tsv_lang::fanout_probe::{self, BUDGET_EXCEEDED, Budget};

mod legs;

use legs::{FanoutKey, Finding, InputResult};

/// Audit for exponential doc-node fanout — the per-layout-candidate rebuild
/// blowup — and for the work a format does beside its builds.
///
/// Builds synthetic nested inputs at increasing depth, formats each into a
/// fresh `DocArena`, and checks that the doc-node count grows no faster than
/// cubically with nesting depth (`MAX_EXPONENT`; a linear build passes with room
/// to spare). A builder that assembles `conditional_group` candidates by
/// *re-invoking the recursive builder* on the same nodes (instead of building
/// once and reusing the `DocId`) makes the count grow exponentially in depth;
/// this catches that and guards against reintroduction. The doc-node count is
/// read directly via `format_ast_in` into a caller-owned arena +
/// `borrow_nodes().len()`.
///
/// The counted legs ([`legs`]) read `tsv_lang::fanout_probe`'s counters (the
/// `audits` feature) over the depth-limit gate's catalog, the known-bug shapes and
/// flat runs — builds per AST node, doc nodes per byte, work-counter growth — and
/// grade against the `fanout_audit_known.txt` ratchet. Pure Rust, no Deno.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "build_fanout_audit")]
pub struct BuildFanoutAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// regenerate the known-failure snapshot (refused on a `--filter` run)
    #[argh(switch)]
    update: bool,

    /// grade only the inputs whose name contains this (diagnostic; the ratchet is not
    /// graded, never with --update)
    #[argh(option)]
    filter: Option<String>,
}

const SNAPSHOT_HEADER: &str = "\
# Build-fanout work ratchet — every line is a KNOWN BUG, the file shrinking is the goal.
#
# One line per failing (leg, shape), TAB-delimited: the leg (`multiplicity` — a node built
# more than 32 times, or more than 4 builds per built node; `density` — more than
# 8 doc nodes per byte + 2,000; `work` — a work counter growing faster than n^2 over
# nesting depths 8/16/32, or n^1.3 over flat runs of 1k/2k/4k or a chain's links at
# 8/16/32/64/128), the shape's name, and the bug it reproduces. A shape only pins with a bug
# number beside it (`KNOWN_CAUSES` in build_fanout_audit/legs.rs), so a NEW blowup fails
# until it is named; a fix removes its lines. A family of generated shapes one bug explains
# (`KNOWN_FAMILIES`) pins as ONE line, its label in the shape column, and covers one
# counter's work exponent alone: any other finding on a member fails as NEW.
#
# Regenerate with `deno task fanout:audit:update`.
";

const REPIN_HINT: &str = "deno task fanout:audit:update";

fn snapshot() -> Ratchet {
    Ratchet::colocated("fanout_audit_known.txt", SNAPSHOT_HEADER, REPIN_HINT)
}

/// Stop a construct's format once it allocates more than this many doc nodes (the work
/// legs' node budget, armed here too), so the audit itself never OOMs on a blowup.
/// Exceeding it is treated as a failure (clearly super-linear).
const NODE_CAP: u64 = legs::NODE_BUDGET;

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
/// the sibling-`>` dangle path (`block_sibling_takes_gt`), which must build the block once,
/// with the `>` threaded in: probing it for a `will_break` test and rebuilding it folded with the
/// `>` doubles the work per level.
fn gen_svelte_block_sibling(depth: usize) -> String {
    let opens = "<span>t</span>{#if c}".repeat(depth);
    let closes = "{/if}".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// Nested glued inline-element runs (`<b>a</b><i><b>a</b><i>…x…</i></i>`) — the G2 glued-run
/// builder (`build_glued_element_run`), which decides every member's shed / receive role before
/// building any of them. Deciding a role by building the member's whole doc (children included)
/// and discarding it, then building it again, doubles the work at every nesting level.
fn gen_svelte_glued_run(depth: usize) -> String {
    let opens = "<b>a</b><i>".repeat(depth);
    let closes = "</i>".repeat(depth);
    format!("{opens}x{closes}\n")
}

/// [`gen_svelte_glued_run`] with every member's content authored on its own lines
/// (`<b>⏎{a}⏎</b><i>⏎…⏎</i>`), so each shedder is multiline by its authored line breaks — the
/// multiline arm of the shed / receive roles, which the compact nest (every member `Soft`)
/// never reaches.
fn gen_svelte_glued_run_multiline(depth: usize) -> String {
    let opens = "<b>\n{a}\n</b><i>\n".repeat(depth);
    let closes = "\n</i>".repeat(depth);
    format!("{opens}{{x}}{closes}\n")
}

/// An inline element that sheds its `>` onto a glued control-flow block, nested inside an inline
/// wrapper that holds a block of its own (`<b><i>…{#if c}x{/if}</i></b>{#if c}x{/if}`), so the
/// wrapper is structurally multiline and each level's shedder sits in the one below — the
/// element→block dangle (`block_sibling_takes_gt`), which must decide the dangle before it
/// builds the element. Building the element as the fragment loop's ordinary child, then again
/// with its `>` split off once the block turns up, doubles the work at every level.
fn gen_svelte_block_dangle_nest(depth: usize) -> String {
    let mut s = String::from("t");
    for _ in 0..depth {
        s = format!("<b><i>{s}{{#if c}}x{{/if}}</i></b>{{#if c}}x{{/if}}");
    }
    s.push('\n');
    s
}

/// [`gen_svelte_block_dangle_nest`] with every boundary authored on its own line. Each
/// shedder is then multiline by its authored line breaks, which sheds its `>` exactly as the
/// compact form's width-broken shedder does, so a re-format walks the same dangle path.
fn gen_svelte_block_dangle_nest_multiline(depth: usize) -> String {
    let mut s = String::from("t");
    for _ in 0..depth {
        s = format!("<b>\n<i>\n{s}{{#if c}}\nx\n{{/if}}\n</i>\n</b>{{#if c}}\nx\n{{/if}}");
    }
    s.push('\n');
    s
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

/// The commented twin of `gen_ts_nested_arrow`: one block comment in the innermost body
/// (`a(x => b(y => … z(/* c */ q)))`) puts every enclosing argument's span on page, so a
/// lone call-body arm that declines to share its body build for a commented argument
/// rebuilds every level twice.
fn gen_ts_nested_arrow_inner_comment(depth: usize) -> String {
    let mut expr = String::from("z(/* c */ q)");
    for i in 0..depth {
        expr = format!("call{i}(p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg arrow whose body is another such call — a leading arg at every
/// level routes through `try_expand_last_function_arg` (the `build_args_split_last` /
/// `build_break_body_ladder` expand-last path). The multi-arg twin of `gen_ts_nested_arrow`:
/// with a leading argument the whole-arrow arg doc and the break-body state each recurse
/// into the body unless the body build is shared.
fn gen_ts_nested_arrow_multiarg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(lead{i}, p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// The commented twin of `gen_ts_nested_arrow_multiarg`: one block comment in the innermost
/// body (`a(l, x => b(l, y => … z(/* c */ q)))`) puts every enclosing argument's span on page,
/// so an expand-last arm that declines to share its body build for a commented argument
/// rebuilds every level twice.
fn gen_ts_nested_arrow_multiarg_inner_comment(depth: usize) -> String {
    let mut expr = String::from("z(/* c */ q)");
    for i in 0..depth {
        expr = format!("call{i}(lead{i}, p{i} => {expr})");
    }
    format!("const v = {expr};\n")
}

/// The `new` spelling of `gen_ts_nested_arrow_multiarg_inner_comment`
/// (`new A(l, x => f(new B(l, y => … z(/* c */ q))))`): a `new` expression's last-argument
/// expansion shares the plain call's arm, and the arrow's call body carries the next level.
fn gen_ts_nested_new_arrow_multiarg_inner_comment(depth: usize) -> String {
    let mut expr = String::from("z(/* c */ q)");
    for i in 0..depth {
        expr = format!("new Call{i}(lead{i}, p{i} => f{i}({expr}))");
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

/// SINGLE-arg last-arg arrow with an OWN-LINE comment after `=>` and an OBJECT body — the
/// state that legitimately builds the argument TWICE (`build_own_line_post_arrow_arg_docs`:
/// the `expandLastArg` printing for the hug state, the `printedArguments` one for the
/// `allArgsBrokenOut()` fallback behind it). The object terminal keeps the body injection
/// armed, which is what holds the pair linear.
fn gen_ts_nested_arrow_own_line_comment_obj(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} =>\n// c{i}\n({{ k{i}: {expr} }}))");
    }
    format!("const v = {expr};\n")
}

/// The same shape with a **BLOCK** body — the axis that matters, because no body injection
/// reaches a block terminal, so both printings rebuild the argument's whole subtree. This is
/// the one construct where the second build is paid unguarded, and the exponent is what says
/// whether it compounds.
fn gen_ts_nested_arrow_own_line_comment_block(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} =>\n// c{i}\n{{ return {expr}; }})");
    }
    format!("const v = {expr};\n")
}

/// The same shape with a plain **CALL** body — the third prebuild arm, where the injected doc
/// is the body's own build rather than a paren-wrapped or block one.
fn gen_ts_nested_arrow_own_line_comment_call(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} =>\n// c{i}\ng{i}({expr}))");
    }
    format!("const v = {expr};\n")
}

/// The **other** arm of the same gap question: a block comment GLUED to `=>` that the author
/// broke after, before an object body that hard-breaks. It selects the identical state
/// (`last_arg_arrow_gap_break`), and answering it needs the body's `will_break` — so this is
/// the axis that says whether that probe built a doc of its own. It must not: the gate hands
/// its injection to both printings, exactly as the own-line arm above does.
fn gen_ts_nested_arrow_broke_after_comment_obj(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} => /* c{i} */\n({{\nk{i}: {expr}\n}}))");
    }
    format!("const v = {expr};\n")
}

/// SINGLE-arg **curried** arrow chain (`f((a) => (b) => {{ … }})`) whose block terminal holds
/// the next such call — `call_formatting.rs`'s `build_block_arrow_hug_states`. Prettier prints
/// its last argument twice (`printedArguments` vs an `expandLastArg` `lastArg`), so this is the
/// standing temptation to build the argument twice here; doing so recurses into the nested call
/// and the doc-node count goes 2^depth. The builder instead picks ONE rendering, which is what
/// this axis holds.
fn gen_ts_nested_curried_arrow(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}((a{i}) => (b{i}) => {{ return {expr}; }})");
    }
    format!("const v = {expr};\n")
}

/// The OBJECT-TERMINAL curried chain (`f((a) => (b) => ({ … }))`), the one curried shape whose
/// hug ladder has a middle state — so it is the one that legitimately builds the argument TWICE,
/// as prettier's `printedArguments` and its `expandLastArg` `lastArg`. The three siblings all end
/// in a BLOCK and so cannot reach this arm at all.
///
/// What holds the curve is the terminal body being pre-built and injected around both builds
/// (`prebuild_expand_last_obj_array_body`): the second printing then costs the chain's signatures
/// rather than the subtree, and the nested call inside the object is built once. Drop the
/// injection and this axis goes 2^depth.
fn gen_ts_nested_curried_arrow_obj(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}((a{i}) => (b{i}) => ({{ k{i}: {expr} }}))");
    }
    format!("const v = {expr};\n")
}

/// The multi-argument twin of `gen_ts_nested_curried_arrow_obj` — `expand_last.rs`'s arm rather
/// than the single-argument ladder, which re-injects the pre-built body around its own second
/// printing.
fn gen_ts_nested_curried_arrow_obj_multiarg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}('lead{i}', (a{i}) => (b{i}) => ({{ k{i}: {expr} }}))");
    }
    format!("const v = {expr};\n")
}

/// The member-chain twin (`chain_args.rs`, both its single- and multi-argument object/array
/// arms), which owns its own copy of the injection. The curried argument sits on a NON-LAST
/// chain call for the reason `gen_ts_nested_curried_arrow_chain` gives.
fn gen_ts_nested_curried_arrow_obj_chain(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("obj{i}.method((a{i}) => (b{i}) => ({{ k{i}: {expr} }})).next()");
    }
    format!("const v = {expr};\n")
}

/// The BREAK-FORCING twin of `gen_ts_nested_curried_arrow` — a destructured head makes
/// `arrow_chain_should_break` true, which sends the expand-last hug down its other arm
/// (`skip_arrow_chain`, prettier's `expandLastArg` print) instead of reusing the chain doc.
/// Both arms must stay single-build; only one of them is exercised by the other axis.
fn gen_ts_nested_curried_arrow_typed(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(({{ a{i} }}) => ({{ b{i} }}) => {{ return {expr}; }})");
    }
    format!("const v = {expr};\n")
}

/// The `new` twin of `gen_ts_nested_curried_arrow` (new_expression.rs's single-argument
/// block-arrow arm) — the same two renderings, the same temptation, its own copy of the code.
fn gen_ts_nested_curried_arrow_new(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("new Cls{i}((a{i}) => (b{i}) => {{ return {expr}; }})");
    }
    format!("const v = {expr};\n")
}

/// The member-chain twin (`chain_args.rs`'s `HugsNaturally` arm, whose curried case is a
/// measured `conditional_group` rather than an unconditional hug). The curried argument sits
/// on a NON-LAST chain call (`obj.method(…).next()`): a chain whose curried argument is on the
/// last call is printed by the plain-call path instead, so that spelling would make this axis
/// decorative — a distinction confirmed by probing which builder each shape reaches.
///
/// ⚠️ Unlike its three siblings, a double build in that arm does **not** move this curve, and
/// that is a fact about the chain path rather than a gap here: every build under a chain rides
/// the chain share store (`Printer::chain_share_key`), whose key carries the builder and the
/// `expandLastArg` state, so a repeat under the same state returns the same `DocId`. The axis
/// still holds the growth curve for everything the store does not cover — anything built outside
/// a share scope, and anything whose state varies per candidate.
fn gen_ts_nested_curried_arrow_chain(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("obj{i}.method((a{i}) => (b{i}) => {{ return {expr}; }}).next()");
    }
    format!("const v = {expr};\n")
}

/// LONG-CURRIED calls (`f(a, b)(c)`, an inner list longer than the outer one) nested through
/// the OUTER call's argument — the chain link whose list prints without its own group
/// (`ChainCall::ungroup_args`). The pass that marks it groups the chain ending at the inner
/// call a second time, so a mark that re-entered the argument's own chain per level would
/// show here.
fn gen_ts_long_curried_arg(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("f{i}(a{i}, b{i})({expr})");
    }
    format!("const v = {expr};\n")
}

/// The CALLEE twin of `gen_ts_long_curried_arg` — one curried run `f(a, b)(c)(a, b)(c)…`, where
/// every other link is long-curried and every call on a call closes a group over everything
/// before it (`print_curried_group`). Each group must wrap the prefix built so far, not
/// rebuild it.
fn gen_ts_long_curried_callee(depth: usize) -> String {
    let mut expr = String::from("f");
    for i in 0..depth {
        expr = format!("{expr}(a{i}, b{i})(c{i})");
    }
    format!("const v = {expr};\n")
}

/// A curried call on a MEMBER CHAIN (`obj.a().b().f(x, y)(z)`), nested through the outer
/// argument — the call printed `group([callee, args])` with the chain built whole as its
/// callee (`build_curried_member_chain_call_doc`). The callee chain's last list is long-curried
/// here, so the chain is built under the outer call's mark; a mark that rebuilt it per state
/// would show here.
fn gen_ts_curried_member_chain_tail(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("obj{i}.a().b().f(x{i}, y{i})({expr})");
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
/// sub-branch (the sibling of the call-body branch, sharing `build_break_body_ladder`).
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

/// [`gen_ts_nested_arrow_cond_single`] with a forced break in the ternary's other branch (a
/// multi-line template) — the lone ternary ladder's will-break branch, whose broken-out state
/// prints the whole argument with the already-built body injected
/// (`build_ternary_arrow_hug_ladder`).
fn gen_ts_nested_arrow_cond_single_forced(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("call{i}(p{i} => (c{i} ? {expr} : `x\ny`))");
    }
    format!("const v = {expr};\n")
}

/// A lone `new` arrow argument whose ternary body follows an own-line comment — the broken
/// `=>`→body gap route, asked above `new`'s single-argument body-kind match
/// (`build_arrow_gap_break_single_arg_doc`).
fn gen_ts_nested_arrow_cond_single_new_comment(depth: usize) -> String {
    let mut expr = String::from("done");
    for i in 0..depth {
        expr = format!("new C{i}((p{i}) =>\n/* c */\nc{i} ? {expr} : z{i})");
    }
    format!("const v = {expr};\n")
}

/// MULTI-arg last-arg FUNCTION expression (block body) that recurses in its `return`
/// (`f(lead, function (x) {{ return f(lead, function (y) {{ … }}); }})`) — the block-body
/// last-arg path (`ArgOpener::inline_or_expand_all`).
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
    match parser {
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source, &bump).map_err(|e| format!("{e:#}"))?;
            let _ = tsv_svelte::__private::format_ast_in(&ast, source, &doc_arena);
        }
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(source, &bump).map_err(|e| format!("{e:#}"))?;
            let _ = tsv_ts::__private::format_ast_in(&ast, source, &doc_arena);
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &bump).map_err(|e| format!("{e:#}"))?;
            let _ = tsv_css::__private::format_ast_in(&ast, source, &doc_arena);
        }
    }
    Ok(doc_arena.borrow_nodes().len())
}

/// The doc-node growth leg's budget: the node cap alone, every other counter unbounded.
const GROWTH_BUDGET: Budget = Budget {
    builds: u64::MAX,
    walks: u64::MAX,
    scans: u64::MAX,
    predicates: u64::MAX,
    nodes: NODE_CAP,
};

/// Measure one construct across its depth sweep (ascending, stopping at the format that
/// allocates past the node cap) and decide pass/fail from the growth exponent. A panic is
/// the construct's error.
fn measure(c: &Construct) -> ConstructResult {
    let mut points = Vec::new();
    let mut exceeded_cap = false;
    let mut error = None;
    for &d in c.depths {
        let src = (c.generate)(d);
        fanout_probe::arm(GROWTH_BUDGET);
        let result = catch_unwind(AssertUnwindSafe(|| doc_node_count(&src, c.parser)));
        fanout_probe::disarm();
        match result {
            Ok(Ok(n)) => points.push(Point { depth: d, nodes: n }),
            Ok(Err(e)) => {
                error = Some(e);
                break;
            }
            Err(payload) => {
                let message = panic_message(&*payload);
                if message == BUDGET_EXCEEDED {
                    exceeded_cap = true;
                } else {
                    error = Some(format!("panicked: {message}"));
                }
                break;
            }
        }
    }
    // doc_nodes ~ depth^E  ⇒  E = log_(d_hi/d_lo)(n_hi/n_lo). Counts are bounded
    // by NODE_CAP and depths are tiny, so the f64 conversions are exact.
    #[expect(clippy::cast_precision_loss)]
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

/// The object type a level of the flat-sole-parameter-literal constructs annotates its
/// callback's sole parameter with: written flat and too wide for any line at any depth, so
/// every level takes the width-broken literal's convergence state.
const FLAT_WIDE_PARAM_LITERAL: &str =
    "{ b: C; d: DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD }";

/// A block-bodied callback LAST argument whose sole parameter's object type is written flat
/// and too wide (`f(lead, (a: { … }) => { f(lead, (b: { … }) => { … }); })`) — the
/// width-broken literal's hug state (`calls::AssumedParamHug`), whose second printing of the
/// callback must share the body rather than rebuild it.
fn gen_ts_nested_flat_param_literal_multiarg(depth: usize) -> String {
    let mut body = String::from("done();");
    for i in 0..depth {
        body = format!("call{i}(lead{i}, (a{i}: {FLAT_WIDE_PARAM_LITERAL}) => {{ {body} }});");
    }
    format!("{body}\n")
}

/// The same callback as a LONE argument of a two-call member chain's last call — the chain's
/// own convergence state, which rebuilds the last group per candidate.
fn gen_ts_nested_flat_param_literal_chain(depth: usize) -> String {
    let mut body = String::from("done();");
    for i in 0..depth {
        body = format!("obj{i}.a().fn((a{i}: {FLAT_WIDE_PARAM_LITERAL}) => {{ {body} }});");
    }
    format!("{body}\n")
}

/// The same callback as a `function` expression after a lead argument — the body shared
/// through the `function` spelling of the body injection.
fn gen_ts_nested_flat_param_literal_function(depth: usize) -> String {
    let mut body = String::from("done();");
    for i in 0..depth {
        body =
            format!("call{i}(lead{i}, function (a{i}: {FLAT_WIDE_PARAM_LITERAL}) {{ {body} }});");
    }
    format!("{body}\n")
}

/// The same `function` callback with an object return type, which ends the broken-out
/// line at that type's `{` — the third printing of the callback the state's gate measures.
fn gen_ts_nested_flat_param_literal_function_return(depth: usize) -> String {
    let mut body = String::from("done();");
    for i in 0..depth {
        body = format!(
            "call{i}(lead{i}, function (a{i}: {FLAT_WIDE_PARAM_LITERAL}): {{ x: X }} {{ {body} }});"
        );
    }
    format!("{body}\n")
}

/// That callback after a lead argument of a two-call member chain's last call, in a value
/// position — the chain's own convergence state, which measures its own printing of that line.
fn gen_ts_nested_flat_param_literal_chain_return(depth: usize) -> String {
    let mut body = String::from("done();");
    for i in 0..depth {
        body = format!(
            "const v{i} = obj{i}.a().fn(lead{i}, function (a{i}: {FLAT_WIDE_PARAM_LITERAL}): {{ x: X }} {{ {body} }});"
        );
    }
    format!("{body}\n")
}

/// The same callback as the FIRST of two arguments (`reduce((a: { … }) => { … }, '')`) —
/// the expand-first hug's spelling of the state.
fn gen_ts_nested_flat_param_literal_first(depth: usize) -> String {
    let mut body = String::from("done();");
    for i in 0..depth {
        body = format!("call{i}((a{i}: {FLAT_WIDE_PARAM_LITERAL}) => {{ {body} }}, '');");
    }
    format!("{body}\n")
}

/// A member chain whose last call hugs an object type argument, nested in that call's own
/// argument (`obj0.a().f<{ k0: 1 }>(obj1.a().f<{ k1: 1 }>(…))`) — the chain's type-argument
/// hug state, which rebuilds the last group under the assumed break. The nested chain's
/// conditional states are copied into every outer level's rebuild, a quadratic term this
/// construct already pays without the state; the state multiplies that term by a constant
/// and must not raise its exponent.
fn gen_ts_nested_type_arg_hug_chain(depth: usize) -> String {
    let mut expr = String::from("done()");
    for i in 0..depth {
        expr = format!("obj{i}.a().f<{{ k{i}: 1 }}>({expr})");
    }
    format!("{expr};\n")
}

/// The same chain nested in an EARLIER call's argument (`obj0.a(obj1.a(…).f<…>(x1)).f<…>(x0)`)
/// — the state's prefix docs come from the one-line state's flat groups and must stay shared.
fn gen_ts_nested_type_arg_hug_chain_prefix(depth: usize) -> String {
    let mut expr = String::from("done()");
    for i in 0..depth {
        expr = format!("obj{i}.a({expr}).f<{{ k{i}: 1 }}>(x{i})");
    }
    format!("{expr};\n")
}

/// Every construct the audit measures, with the depths it grows each one to.
const CONSTRUCTS: &[Construct] = &[
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
        name: "svelte_glued_run",
        parser: ParserType::Svelte,
        generate: gen_svelte_glued_run,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "svelte_glued_run_multiline",
        parser: ParserType::Svelte,
        generate: gen_svelte_glued_run_multiline,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "svelte_block_dangle_nest",
        parser: ParserType::Svelte,
        generate: gen_svelte_block_dangle_nest,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "svelte_block_dangle_nest_multiline",
        parser: ParserType::Svelte,
        generate: gen_svelte_block_dangle_nest_multiline,
        depths: &[4, 8, 12],
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
        name: "ts_nested_arrow_inner_comment",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_inner_comment,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_multiarg",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_multiarg,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_multiarg_inner_comment",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_multiarg_inner_comment,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_new_arrow_multiarg_inner_comment",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_new_arrow_multiarg_inner_comment,
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
        name: "ts_nested_curried_arrow",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_curried_arrow_typed",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow_typed,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_curried_arrow_new",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow_new,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_curried_arrow_chain",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow_chain,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_long_curried_arg",
        parser: ParserType::TypeScript,
        generate: gen_ts_long_curried_arg,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_long_curried_callee",
        parser: ParserType::TypeScript,
        generate: gen_ts_long_curried_callee,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_curried_member_chain_tail",
        parser: ParserType::TypeScript,
        generate: gen_ts_curried_member_chain_tail,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_curried_arrow_obj",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow_obj,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_curried_arrow_obj_multiarg",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow_obj_multiarg,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_curried_arrow_obj_chain",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_curried_arrow_obj_chain,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_obj_multiarg",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_obj_multiarg,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_own_line_comment_obj",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_own_line_comment_obj,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_own_line_comment_block",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_own_line_comment_block,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_own_line_comment_call",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_own_line_comment_call,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_broke_after_comment_obj",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_broke_after_comment_obj,
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
        name: "ts_nested_arrow_cond_single_forced",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_cond_single_forced,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_arrow_cond_single_new_comment",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_arrow_cond_single_new_comment,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_fn_expr_multiarg",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_fn_expr_multiarg,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_flat_param_literal_multiarg",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_flat_param_literal_multiarg,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_flat_param_literal_chain",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_flat_param_literal_chain,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_flat_param_literal_function",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_flat_param_literal_function,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_flat_param_literal_first",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_flat_param_literal_first,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_flat_param_literal_function_return",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_flat_param_literal_function_return,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_flat_param_literal_chain_return",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_flat_param_literal_chain_return,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_type_arg_hug_chain",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_type_arg_hug_chain,
        depths: &[4, 8, 12],
    },
    Construct {
        name: "ts_nested_type_arg_hug_chain_prefix",
        parser: ParserType::TypeScript,
        generate: gen_ts_nested_type_arg_hug_chain_prefix,
        depths: &[4, 8, 12],
    },
];

impl BuildFanoutAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if !fanout_probe::ENABLED {
            eprintln!(
                "Error: build_fanout_audit needs the work counters: build with `--features \
                 audits` (or `fanout_check`) — `deno task fanout:audit` does."
            );
            return Err(CliError::Failed);
        }
        let narrowed: &[&'static str] = if self.filter.is_some() {
            &["--filter"]
        } else {
            &[]
        };
        refuse_narrowed_update(
            self.update,
            narrowed,
            "the depth gate's full sweep, the bug shapes and the flat runs",
            "SUBSET",
        )?;
        // every leg records the panics it catches: the growth leg as its construct's error,
        // the work legs as findings
        let hook = SuppressedPanicHook::install();
        let results: Vec<ConstructResult> = CONSTRUCTS.iter().map(measure).collect();
        let failed = results.iter().filter(|r| !r.pass).count();

        // The work legs: each construct at its deepest depth, then every input.
        let mut findings: Vec<Finding> = Vec::new();
        for c in CONSTRUCTS {
            if let Some(&depth) = c.depths.last() {
                let outcome = legs::measure(&(c.generate)(depth), c.parser);
                legs::grade_point(c.name, depth, &outcome, &mut findings);
            }
        }
        // The full sweep, not `check`'s sample of it: the depth gate samples because it
        // searches each shape's limit, where this formats each shape a few times at small
        // sizes (a nest to 32, a chain to 128 links) — the whole sweep is seconds.
        let inputs: Vec<legs::Input> = legs::depth_inputs(crate::depth_shapes::Scope::Full)
            .chain(legs::bug_inputs())
            .chain(legs::flat_inputs())
            .filter(|input| {
                self.filter
                    .as_deref()
                    .is_none_or(|f| input.shape.name.contains(f))
            })
            .collect();
        let mut input_results: Vec<InputResult> = Vec::with_capacity(inputs.len());
        for input in &inputs {
            let result = legs::grade_input(input, &mut findings);
            if input.required
                && let Some(reason) = &result.skipped
            {
                drop(hook);
                eprintln!(
                    "Error: the hand-written shape `{}` no longer parses: {reason}",
                    input.shape.name
                );
                return Err(CliError::Failed);
            }
            input_results.push(result);
        }
        drop(hook);
        findings.sort_by(|a, b| a.key.cmp(&b.key));
        let found = legs::found_keys(&findings);

        if self.json {
            print_json(&results, &input_results, &findings);
        } else {
            print_human(&results, failed);
            print_legs_human(&input_results, &findings);
        }

        if self.filter.is_some() {
            print_ratchet_skipped(narrowed);
            return if failed > 0 {
                Err(CliError::Failed)
            } else {
                Ok(())
            };
        }
        let ratchet = snapshot();
        if self.update {
            if failed > 0 {
                eprintln!("Error: the doc-node growth leg failed; fix it before re-pinning.");
                return Err(CliError::Failed);
            }
            let unpinnable: Vec<&FanoutKey> = found
                .iter()
                .filter(|k| !crate::audit::ratchet::SnapshotKey::is_pinnable(*k))
                .collect();
            if !unpinnable.is_empty() {
                for key in unpinnable {
                    eprintln!(
                        "✗ cannot pin (no known bug, or a panic): {}",
                        legs::render_key(key)
                    );
                }
                return Err(CliError::Failed);
            }
            ratchet.write_pinned(&found, "shape", self.json)?;
            return Ok(());
        }
        for key in found
            .iter()
            .filter(|k| !crate::audit::ratchet::SnapshotKey::is_pinnable(*k))
        {
            eprintln!(
                "✗ unpinnable (no known bug names it, or a panic): {}",
                legs::render_key(key)
            );
        }
        let ratchet_result = ratchet.grade_and_report(
            &found,
            "failing shape",
            &format!("{} inputs", input_results.len()),
            self.json,
            legs::render_key,
        );
        if failed > 0 {
            return Err(CliError::Failed);
        }
        ratchet_result
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
            lang_token(r.parser),
            trail,
            exp,
            verdict(r),
        );
    }
    println!();
    if failed > 0 {
        println!(
            "FAIL: {failed}/{} constructs show doc-node fanout past the exponent limit ({MAX_EXPONENT:.1}).",
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

/// The work legs' report: the failing findings, then a census of what was graded.
fn print_legs_human(inputs: &[InputResult], findings: &[Finding]) {
    let graded = inputs.iter().filter(|r| r.skipped.is_none()).count();
    let skipped = inputs.len() - graded;
    println!(
        "\nbuild-fanout work legs — {graded} inputs graded ({skipped} the parser rejects, \
         skipped): build multiplicity and doc nodes per byte at n={} (flat: n={}), work \
         counter growth over n={:?} (flat: {:?}; chain links: {:?}, graded on the flat \
         exponent)\n",
        legs::GRADED_DEPTH,
        legs::FLAT_SWEEP[0],
        legs::NESTING_SWEEP,
        legs::FLAT_SWEEP,
        legs::CHAIN_SWEEP,
    );
    let worst = |f: fn(&legs::Point) -> f64| {
        inputs
            .iter()
            .filter_map(|r| r.graded.as_ref().map(|p| (f(p), r.name.as_str())))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    };
    #[expect(clippy::cast_precision_loss)]
    let ratio = |p: &legs::Point| p.total_builds as f64 / p.built_nodes.max(1) as f64;
    #[expect(clippy::cast_precision_loss)]
    let density = |p: &legs::Point| p.doc_nodes as f64 / p.bytes.max(1) as f64;
    if let Some((v, name)) = worst(|p| f64::from(p.max_builds)) {
        println!("  most builds of one node:     {v:>9.0}  {name}");
    }
    if let Some((v, name)) = worst(ratio) {
        println!("  most builds per built node:  {v:>9.2}  {name}");
    }
    if let Some((v, name)) = worst(density) {
        println!("  most doc nodes per byte:     {v:>9.2}  {name}");
    }
    for growth in [
        legs::Growth::Nesting,
        legs::Growth::Chain,
        legs::Growth::Flat,
    ] {
        if let Some((e, counter, name)) = inputs
            .iter()
            .filter(|r| r.growth == growth)
            .filter_map(|r| r.exponent.map(|(c, e)| (e, c, r.name.as_str())))
            .max_by(|a, b| a.0.total_cmp(&b.0))
        {
            println!(
                "  steepest {:<7} work growth: n^{e:<6.2} {counter} of {name}",
                legs::growth_token(growth)
            );
        }
    }
    if !findings.is_empty() {
        println!("\n  failing ({}):", findings.len());
        for f in findings {
            println!("    {} — {}", legs::render_key(&f.key), f.detail);
        }
    }
}

fn print_json(results: &[ConstructResult], inputs: &[InputResult], findings: &[Finding]) {
    let arr: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "parser": lang_token(r.parser),
                "points": r.points.iter().map(|p| serde_json::json!({"depth": p.depth, "nodes": p.nodes})).collect::<Vec<_>>(),
                "exceeded_cap": r.exceeded_cap,
                "error": r.error,
                "exponent": r.exponent,
                "pass": r.pass,
            })
        })
        .collect();
    let inputs: Vec<serde_json::Value> = inputs
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "parser": legs::parser_token(r.parser),
                "growth": legs::growth_token(r.growth),
                "skipped": r.skipped,
                "graded": r.graded.as_ref().map(|p| serde_json::json!({
                    "n": p.size,
                    "bytes": p.bytes,
                    "doc_nodes": p.doc_nodes,
                    "built_nodes": p.built_nodes,
                    "builds": p.total_builds,
                    "max_builds": p.max_builds,
                })),
                "sweep": r.sweep.iter().map(|(n, counters)| {
                    let mut point = serde_json::Map::new();
                    point.insert("n".into(), (*n).into());
                    for &(name, count) in counters {
                        point.insert(name.into(), count.into());
                    }
                    serde_json::Value::Object(point)
                }).collect::<Vec<_>>(),
                "exponent": r.exponent.map(|(counter, e)| serde_json::json!({"counter": counter, "exponent": e})),
            })
        })
        .collect();
    let findings: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "key": crate::audit::ratchet::SnapshotKey::to_line(&f.key),
                "detail": f.detail,
            })
        })
        .collect();
    let out = serde_json::json!({
        "max_exponent": MAX_EXPONENT,
        "node_cap": NODE_CAP,
        "constructs": arr,
        "inputs": inputs,
        "findings": findings,
        "pass": results.iter().all(|r| r.pass),
    });
    super::print_json_pretty(&out);
}
