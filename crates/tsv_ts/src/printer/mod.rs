// TypeScript printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct, constructors, and source/comment utilities
// - **analysis.rs**: Pure AST analysis functions (no Printer state needed)
// - **comments.rs**: Comment handling (printing, doc building, filtering)
// - **program.rs**: Program-level printing orchestration (statements, blank lines, comments)
// - **decorators.rs**: Decorator printing (class-level and class-member)
// - **statements/**: Statement printing (declarations, control flow, modules, etc.)
// - **expressions/**: Expression printing (dispatch, literals, functions, patterns, templates,
//   objects, arrays, operators, assignment, conditionals)
// - **types/**: Type annotation printing (TypeScript-specific type syntax)
// - **calls/**: Call and `new` expression formatting (argument wrapping, expand patterns)
// - **chain/**: Member/call chain linearization, grouping, and rendering
// - **class_common.rs**: Shared class-header layout for declaration + expression printers
// - **pre_body.rs**: The head→body `{` seam every braced-body declaration crosses (class,
//   interface, enum, namespace, and every value-level function definition)
// - **needs_parens.rs**: Centralized parenthesization logic (`needs_parens(expr, ctx)`)
// - **layout.rs**: Shared hang-indent "break after operator, then indent continuation" doc shapes
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change TypeScript semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod analysis;
#[cfg(feature = "buffer_stats")]
pub mod buffer_stats;
mod calls;
mod chain;
mod class_common;
mod comments;
mod decorators;
mod expressions;
mod ignore;
mod layout;
mod needs_parens;
mod pre_body;
mod program;
mod statements;
mod types;

// Layout predicates re-exported from the crate root for embedders (tsv_svelte's
// {@const} assignment layout reuses Prettier's break-after-operator rules).
pub use analysis::conditional_should_break_after_op;
pub(crate) use analysis::{
    PatternContext, build_entity_name_doc, container_may_have_multiline_content,
    has_multiline_content, is_brace_block_multiline, is_effectively_empty_body,
    is_module_path_fluid_call, is_multiline_string_literal, is_multiline_template_expression,
    is_pure_property_chain, is_string_literal, next_printed_stmt, next_printed_stmt_start,
    object_pattern_should_expand, statement_gap_floor, template_literal_has_newlines,
};
pub(crate) use comments::{
    ClassMemberModifiers, CommentFilter, CommentSpacing, CommentVec, ContinuationValue,
    HeritageKeyword, LeadingGlue, MemberBlankScan, MemberBody, MemberFloor, MemberFreeze,
    MemberGap, MemberSeam, OwnedCommentEffect, RunLeadingBlank, ShellLeadingRun, StandaloneGlue,
};
pub use expressions::assignment::should_inline_logical_expression;
pub(crate) use expressions::assignment::{
    arrow_chain_should_break, class_expr_has_decorators, is_call_on_member_chain,
    is_curried_arrow_chain, is_curried_arrow_chain_that_breaks, is_literal_member_chain,
    is_poorly_breakable_chain, is_regex_root_chain, is_self_expanding_value,
    is_simple_self_expanding, is_simple_value, is_single_call_on_member_chain,
    is_type_assertion_call, jsdoc_cast_comment_is_own_line,
};
pub(crate) use needs_parens::{ParenContext, is_in_binary, needs_parens};
pub(crate) use types::unwrap_parenthesized;

use crate::PrinterInputs;
use crate::ast::internal;
use std::cell::Cell;
use tsv_lang::{
    EmbedContext, OutputBuffer, Span, TAB_WIDTH, comments_in_source_after,
    comments_to_emit_in_range,
    doc::{
        self,
        arena::{DocArena, DocId},
    },
    has_comments_to_emit_in_range, has_line_comments_in_range, printing,
    source_scan::{
        TriviaProfile, has_newline_before_position, is_regex_start_after, operand_end_after,
        skip_regex_literal, skip_trivia, trivia_ends_operand,
    },
};

/// Which builder a chain-share entry belongs to — the builder half of the share-map tag,
/// sitting ABOVE the two `expandLastArg` state bits it is OR-ed with.
///
/// One node is reached by more than one builder, and their docs are NOT interchangeable: the
/// expand-last body prebuild wraps an object body in the grammar's parens, which the argument
/// builder never does. Keying them apart is what lets both cache instead of one of them
/// poisoning the other.
///
/// ⚠️ A new variant must claim a bit **outside** [`ShareTag::SKIP_ARROW_CHAIN_BIT`] /
/// [`ShareTag::FLAT_PARAMS_BIT`] — a collision there silently merges a builder with a state,
/// which is a cache hit that is not byte-identical to a rebuild.
#[derive(Clone, Copy)]
pub(crate) enum ShareTag {
    /// [`Printer::build_arg_expression_doc`] — a call argument.
    ArgExpression = 0b0000_0100,
    /// `calls::prebuild_expand_last_obj_array_body` — an arrow's object/array terminal body,
    /// parens included, as the arrow's own body build would produce it.
    ExpandLastBody = 0b0000_1000,
}

impl ShareTag {
    /// `skip_arrow_chain` was set for this build (prettier's `expandLastArg`, chain half).
    pub(crate) const SKIP_ARROW_CHAIN_BIT: u8 = 0b0000_0001;
    /// `expand_last_arg_flat_params` was set for this build (its `removeLines` half).
    pub(crate) const FLAT_PARAMS_BIT: u8 = 0b0000_0010;
    /// The two bits above, which no builder discriminant may claim.
    const STATE_BITS: u8 = Self::SKIP_ARROW_CHAIN_BIT | Self::FLAT_PARAMS_BIT;
}

// The invariant the whole share map rests on, stated where a new variant is written rather
// than only in prose: a builder that overlapped a state bit would answer a lookup made under
// different `expandLastArg` state, which is a hit that is NOT byte-identical to a rebuild.
const _: () = assert!(ShareTag::ArgExpression as u8 & ShareTag::STATE_BITS == 0);
const _: () = assert!(ShareTag::ExpandLastBody as u8 & ShareTag::STATE_BITS == 0);
const _: () = assert!(ShareTag::ArgExpression as u8 != ShareTag::ExpandLastBody as u8);

/// The parent context that routes a curried arrow chain (`(a) => (b) => …`)
/// through a flattened chain layout, mirroring prettier's
/// `printArrowFunctionSignatures` parent-context branches. Set by the enclosing
/// printer (assignment chokepoint, call-argument printer, binary-operand
/// printer) just before the chain's RHS / argument / operand is built; the
/// outermost chain arrow reads and clears it at entry (`replace(None)`) so
/// nested arrows in the chain don't inherit it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ArrowChainContext {
    /// No chain context — arrows use the default break-after-operator path.
    #[default]
    None,
    /// Assignment RHS (`const f = (a) => (b) => …`). The heads join into one
    /// breakable group indented one level after `=` (a leading softline is the
    /// break-after-`=`); all heads share the same indent when they break.
    AssignmentRhs,
    /// Call argument or binaryish operand (`fn((a) => (b) => …)`,
    /// `x ?? ((a) => (b) => …)`) — prettier handles both in one
    /// `printArrowFunctionSignatures` branch. Progressive indent: the first head
    /// stays on the line, the rest indent one level
    /// (`group([sig0, " =>", indent([line, join([" =>", line], rest)])])`).
    CallArgOrBinaryish,
}

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (base indent offset, first-line offset, layout mode, etc.).
    pub(crate) embed: EmbedContext,
    /// Arena allocator for doc nodes (borrowed from caller or locally owned)
    pub(crate) arena: &'a DocArena,
    /// Original source code (for extracting raw values, preserving escape sequences, etc.)
    pub(crate) source: &'a str,
    /// Comments from the program (for printing leading/trailing comments)
    pub(crate) comments: &'a [internal::Comment],
    /// Whether any comment in this document is owned by a node (`owned_by_node`).
    /// Document-level presence flag (from `PrinterInputs`), computed once per document
    /// — never here, per that field's doc (the `.svelte` per-`{expr}` trap). Gates the
    /// owned-leading-comment path so a document with no owned comment (~all of them)
    /// skips its per-expression byte gate entirely.
    pub(crate) has_owned_comments: bool,
    /// Whether any comment in this document is a `format-ignore` directive.
    /// Document-level presence flag (from `PrinterInputs`), computed once per document —
    /// never here (the `.svelte` per-`{expr}` trap). Gates every entry of the
    /// format-ignore seam (`printer/ignore.rs`) so a document with no format-ignore
    /// directive (~all of them) skips the per-node range scan + directive-string match
    /// entirely — each entry reads this flag before any span arithmetic behind it.
    pub(crate) has_format_ignore: bool,
    /// Precomputed line break positions for O(log n) line boundary lookups —
    /// the *layout* table.
    ///
    /// Backs every newline-derived **layout** read: blank-line preservation
    /// (`has_blank_line_between`) and expansion intent (`has_newline_between`,
    /// plus the free-function `*_fast` call sites that take this slice directly).
    /// The canonical reprint path ([`crate::format_canonical`]) empties this
    /// table via [`Self::set_canonical`] so those reads collapse (nothing is
    /// "on a new line", no blank lines), erasing authoring intent.
    ///
    /// **Never read this for comment classification** — use `comment_line_breaks`.
    /// Under `set_canonical` this table is empty, so a comment-adjacency read
    /// against it reports "same line" for every comment: a `//` comment stops
    /// being followed by a break and the next token is glued onto its line,
    /// swallowing content. The name is qualified precisely so the choice has to
    /// be conscious at every call site.
    pub(crate) layout_line_breaks: &'a [u32],
    /// Line breaks used exclusively for *comment* position classification
    /// (`is_same_line`, `classify_comment_fast`, `PartitionedComments`), kept
    /// real even in the canonical path so a comment's trailing/leading/own-line
    /// role stays correct and consecutive line comments never merge onto one
    /// output line. In the normal path this is the same table as
    /// `layout_line_breaks`; they diverge only under [`Self::set_canonical`].
    pub(crate) comment_line_breaks: &'a [u32],
    /// Whether this printer is producing the intent-erased *canonical* reprint
    /// (see [`crate::format_canonical`]).
    ///
    /// Gates the direct source-newline scans — the ones that read `self.source`
    /// instead of a line-break table (type-literal brace + first member, own-line
    /// decorators). Those need an explicit flag because, unlike a table read, they
    /// are **not** self-stabilizing across canonical passes: emptying the table
    /// makes every table read return the same answer for any input, but a raw
    /// source scan sees a newline in pass 1 (the original source) and none in
    /// pass 2 (the collapsed canonical output), so an ungated scan silently breaks
    /// idempotence. Any *new* direct source-newline scan in the printer must be
    /// gated here — or deliberately paired and left un-erased, as the mapped-type
    /// residual is (see `types::composite::build_mapped_type_doc`).
    pub(crate) canonical: bool,
    /// Extra indent depth for declaration contexts (0 normally, 1+ in multi-declarator)
    /// When > 0, multiline objects/arrays get extra indentation
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) declaration_indent_depth: Cell<usize>,
    /// Whether we're currently inside an expression statement (for chain merging decisions)
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) is_expression_statement: Cell<bool>,
    /// Whether we're in a top-level assignment context (ExpressionStatement or VariableDeclaration)
    /// Used for assignment chain detection - assignments at top level use regular grouped layout,
    /// only nested assignments (where parent is another assignment) use chain formatting
    /// Uses Cell for interior mutability so doc builders (&self) can set this
    pub(crate) in_top_level_assignment: Cell<bool>,
    /// Whether we're inside a curried arrow chain that `arrow_chain_should_break` forced
    /// open, on the DEFAULT arrow path (the flattened `build_arrow_chain_doc` never sets it —
    /// it stacks the heads itself). When true, every nested arrow breaks after its `=>`
    /// whether or not it is the head that carried the trigger: `const f = (x: T): H => (y) =>
    /// expr` stacks BOTH heads, not just the annotated one, which is the whole reason the
    /// answer has to travel down rather than be re-asked per arrow.
    pub(crate) in_stacked_arrow_chain: Cell<bool>,
    /// tsv's spelling of prettier's `expandLastArg` — the argument is being printed for an
    /// expand-last **hug** state, so it must render as prettier's `lastArg` rather than as
    /// its `printedArguments` (the counterpart is
    /// `calls::build_printed_argument_doc`, which every broken-out argument goes through).
    ///
    /// ⚠️ **Its live job is the nested-arrow break, not the chain-layout bail.** Two readers
    /// name it: `should_use_arrow_chain_layout`, where it is redundant only because the two
    /// sites that set it (`build_block_arrow_hug_states` and its `new` twin) build the
    /// argument WITHOUT going through `calls::build_printed_argument_doc`, so no
    /// `ArrowChainContext` is in scope and that predicate declines on the context alone — and
    /// `build_arrow_body`'s `chain_should_break`, where it suppresses the break so the hugged
    /// body stays on the `=>` line. That second reader is the whole reason the flag still
    /// exists; deleting it as "the chain bail, already covered" would silently unhug every
    /// typed curried callback (`calls/curried_arrow_chain` is the fixture that says so).
    /// ⚠️ The first reader's redundancy is a fact about those two CALL SITES, not about the
    /// predicate: `should_use_arrow_chain_layout` does not refuse a `shouldBreakChain`
    /// chain outright (it is a break, in every context but the assignment RHS), so the
    /// redundancy is not guaranteed by the predicate. A third setter that ran under a chain context
    /// would make this term load-bearing.
    ///
    /// ⚠️ **Ambient, where prettier's is per-`print()`.** It therefore has to be CLEARED on
    /// the way into anything nested, which `build_printed_argument_doc` does — without that
    /// it reached an unrelated chain inside the hugged body and suppressed its layout too.
    pub(crate) skip_arrow_chain: Cell<bool>,
    /// Whether to render arrow parameters flat (no break points) — mirrors
    /// prettier's `expandLastArg`/`expandFirstArg` path, which prints the
    /// signature with `removeLines` so an expanded last-arg arrow keeps its
    /// params on one line and only the body breaks. Without this, a force-broken
    /// arrow could shatter a destructuring param (`([a, b])`) instead of falling
    /// through to the all-args-broken-out layout. Set around the arrow-doc build
    /// in the expand-last-arg call-argument states.
    pub(crate) expand_last_arg_flat_params: Cell<bool>,
    /// Whether the next parameter list built belongs to a **test call's callback**, and so
    /// renders flat at any width — prettier's `isParametersInTestCall`
    /// (`print/function-parameters.js`, and `print/type-parameters.js`'s
    /// `isParameterInTestCall` for the type parameters), which both ask `isTestCall` of the
    /// function's *parent*. tsv's printer has no parent link, so the call sets this on the way
    /// down instead; the test-call flat layout in `calls/call_formatting.rs` is the sole
    /// `.set` site, and it re-sets per argument so the flag cannot outlive one it never spent.
    ///
    /// ⚠️ **Three readers, and only one of them spends it.** The type-parameter builders PEEK
    /// (`.get()`) — `build_type_params_doc_for_arrow` for an arrow,
    /// `build_type_parameter_declaration_doc_wrapping` for a function expression — because a
    /// signature builds `<…>` before `(…)`, so consuming there would starve the
    /// value parameters. [`Self::build_params_doc_with_comments`] CONSUMES (`.replace(false)`)
    /// at its top, before any child doc exists, which is what bounds the flag to the
    /// callback's own list: a function in the body, or in a parameter default, is built after
    /// the spend and keeps ordinary width-driven params. Left set for the whole argument
    /// build, it would flatten those too.
    ///
    /// It is the *list* that goes flat, never a parameter's own doc — a destructured pattern
    /// still expands on its own, which is why this is not a `remove_lines` over the signature.
    pub(crate) test_call_flat_params: Cell<bool>,
    /// Span of the ObjectExpression at the leftmost position of an arrow body that must
    /// be wrapped in parens to avoid block ambiguity — `() => ({}) as Logger`,
    /// `() => ({}).prop`, `() => ({}) && a`, `() => ({}).b++`. Matches prettier's
    /// `startsWithNoLookaheadToken` traversal. Keyed by span (not consumed) so a chain
    /// rebuilding its base across conditional-group variants wraps consistently, and a
    /// same-shaped object nested deeper (a call argument) never matches.
    pub(crate) arrow_body_object_parens_target: Cell<Option<Span>>,
    /// Span of the object/function/class node that starts an expression statement
    /// and must be wrapped in parens, even when nested as the leftmost token of a
    /// member/binary/etc. chain: `(class {}).foo`, `({}).foo`, `(class {}) + 1`,
    /// `({a: 1}).b().c()`. Matches prettier's `startsWithNoLookaheadToken` traversal.
    /// Keyed by span (not consumed, like `arrow_body_object_parens_target`) so a chain
    /// rebuilding its base across conditional-group variants wraps consistently; cleared
    /// once per statement in `build_expression_statement`.
    pub(crate) expr_stmt_paren_target: Cell<Option<Span>>,
    /// Span of a ternary whose enclosing parens must **expand** onto their own lines
    /// (`(⏎\tcond ? a : b⏎) as T`) instead of hanging — prettier's
    /// `shouldExtraIndentForConditionalExpression`. Recorded by
    /// `Printer::mark_ternary_extra_indent` at the value positions prettier's
    /// `ancestorNameMap` lists, and read where the parens are supplied (a binary cast's
    /// operand, a non-null assertion's operand). Keyed by span and not consumed, like
    /// the two targets above.
    pub(crate) ternary_hang_target: Cell<Option<Span>>,
    /// Span of a JSDoc cast sitting directly in a **value gap**, whose comment→`(`
    /// separator therefore reflows to a space instead of taking the soft `line`
    /// ([`Printer::build_jsdoc_cast_doc`]).
    ///
    /// The gap the cast's comment sits in is a value gap or it isn't, and only the
    /// enclosing printer knows which — the byte before the comment cannot tell an object
    /// value's `key:` from a `label:`. So the value gaps record their cast here
    /// ([`Printer::mark_jsdoc_cast_value_gap`]) and the cast reads it, exactly as the three
    /// targets above route a parent's fact into a child. Keyed by span and not consumed,
    /// like them: a cast rebuilt across conditional-group variants must answer the same way
    /// every time, and a cast nested deeper (a call argument inside the value) has a
    /// different span and so is left alone — which is the point, since only the direct
    /// value's break is the one `docs/conformance_prettier.md` §Authored breaks in value
    /// position reflows.
    pub(crate) jsdoc_cast_value_gap_target: Cell<Option<Span>>,
    /// Span of the JSDoc cast **leading** an embedded value whose gap CANNOT hang — the
    /// braced-head category (`EmbedContext::jsdoc_cast_cannot_hang`): the comment→`(`
    /// separator reflows to a space in **every** authoring, the own-line hardline arm
    /// included, because the host has no operator line to end and the hardline would
    /// strand the `(` at the head's own column ([`Printer::build_jsdoc_cast_doc`]).
    ///
    /// Set once per expression entry (`tsv_ts::build_expression_doc` →
    /// [`Printer::mark_jsdoc_cast_cannot_hang_gap`]) on the value's **left-spine** cast —
    /// the one whose comment leads the value, the same `leading_jsdoc_cast` walk the TS
    /// hang predicates use — and never touched during the walk, unlike
    /// [`Self::jsdoc_cast_value_gap_target`], whose per-gap marks overwrite each other.
    /// Span-keyed like it: a cast nested off the spine keeps its width-decided layout.
    pub(crate) jsdoc_cast_cannot_hang_target: Cell<Option<Span>>,
    /// Span of the member chain whose EVERY `.prop` lookup carries no break point —
    /// prettier's `printMemberExpression` `shouldInline` (member.js), in the two clauses
    /// a chain's PARENT supplies: the assignment **target**
    /// (`firstNonMemberParent.type === "AssignmentExpression" &&
    /// firstNonMemberParent.left.type !== "Identifier"`) and the `new` **callee**
    /// (`shouldInlineNewExpressionCallee`).
    ///
    /// Either way the chain is one unbreakable unit, so the width falls to what the chain
    /// is built FROM — an over-width assignment breaks after the operator instead of
    /// splitting the thing being assigned to, and an over-width `new` sheds into its
    /// argument list. When nothing else can break the line overflows: an unbreakable value
    /// stays welded to the operator (`chooseLayout`'s `!canBreakLeftDoc` gate, which the
    /// per-lookup groups were falsifying), and an argument-less `new` runs long.
    ///
    /// Marked by [`Printer::mark_assignment_target_member_lookups`] /
    /// [`Printer::mark_new_callee_member_lookups`] and read once at the chain root
    /// ([`chain::resolve_inline_lookups`], which also states the optional-chain decline).
    /// Keyed by span and not consumed, like the four targets above: a chain rebuilt across
    /// `conditional_group` variants must answer the same way every time, and a same-shaped
    /// chain nested deeper — in a computed index, or in the assignment's VALUE — has a
    /// different span and keeps its width-driven break points. That last one is deliberate:
    /// prettier's `findAncestor` walk is position-blind and inlines the value's chain too
    /// (`a.b = <long chain>` overflows there), which is an artifact of the walk rather than
    /// the rule, and against tsv's print-width stance — see
    /// `docs/conformance_prettier_ts.md` §Assignment target member chains. The `new`
    /// callee's walk carries no such artifact: it identity-checks every link
    /// (`ancestor.object === child`, `ancestor.callee === child`), so tsv adopts it whole.
    ///
    /// A **computed** lookup is unaffected: its brackets are built by `computed_lookup_doc`,
    /// which prettier also leaves breakable (`shouldInline` includes `node.computed` for the
    /// break point *before* the `[`, never for the brackets themselves). That is what keeps
    /// `canBreakLeftDoc` true for `params['key'] = …`.
    pub(crate) inline_every_member_lookup: Cell<Option<Span>>,
    /// Span of the member chain sitting DIRECTLY under an assignment or a variable
    /// declarator — prettier's `shouldInline` call-object clause,
    /// `(firstNonChainElementWrapperParent.type === "AssignmentExpression" ||
    /// "VariableDeclarator") && isCallExpressionWithArguments(node.object)`.
    ///
    /// Unlike the mark above this one names only a POSITION: the shape half — the tail
    /// being a lone `.prop` off a call that has arguments — is the chain's own to check, at
    /// the peel. The effect is correspondingly narrow: that one lookup loses its break
    /// point, so `const x = fn(a).prop` sheds width into `fn`'s arguments rather than
    /// dropping `.prop` to a line of its own.
    ///
    /// Marked by [`Printer::mark_member_call_tail_operand`], read beside the mark above at
    /// the chain root. Prettier's second disjunct there, `objectDoc.label?.memberChain`,
    /// needs no mark: the label exists only when `printMemberChain` gets past its
    /// `groups.length <= cutoff` early return (which returns `group(oneLine)` UNLABELLED),
    /// and a chain long enough for that is never the bare-base call the peel gives a break
    /// point to — so tsv already glues that tail, by a different route.
    pub(crate) inline_member_call_tail: Cell<Option<Span>>,
    /// Start of an owned leading comment an **enclosing** node already claims, so the node
    /// beginning there must not claim it a second time
    /// ([`Printer::prepend_owned_leading_comment`]).
    ///
    /// One shape needs it: a **paren-less** arrow, whose span starts at its own sole
    /// parameter (`x => x`, `params_start: None`). Both nodes begin at the comment's token,
    /// so the position-keyed lookup answers for both and the comment printed TWICE
    /// (`/* c */ (/* c */ x) => x`). The claim stays on the ARROW, and the parameter declines
    /// through this mark. Handing the claim DOWN — the `left_spine_child` repair every other
    /// same-start pair takes, `[/* c */ a = 1]`'s `AssignmentPattern` among them — is not
    /// available here: those parents print nothing before their left child, so the innermost
    /// node still prints first, while this one emits a *synthesized* `(` that would land the
    /// comment inside a paren the author never wrote.
    ///
    /// Set by [`Printer::build_arrow_params_doc_ungrouped`], the one spelling of an arrow's
    /// parameter list, and save/restored around that build like
    /// [`Printer::with_jsdoc_cast_cannot_hang_gap`]: nothing may leak into the body, where a
    /// nested arrow's own comment sits. Every path that reaches it already claims at
    /// `arrow.span.start` — `build_expression_doc`'s seam for a plain arrow and a chain
    /// head, `prepend_owned_leading_comment_at` for the `build_arrow_sig_doc` reassembly and
    /// a chain's inner arrows — which is what makes the suppression a de-duplication rather
    /// than a DROP (`docs/comments.md` hazard 1).
    pub(crate) claimed_owned_comment_start: Cell<Option<u32>>,
    /// The span of a redundant paren shell whose LEADING run an enclosing keyword→value
    /// gap already claims, so the shell's own emitter
    /// ([`Printer::build_parenthesized_type_unwrap_doc`]) must not print it a second time.
    ///
    /// Set only where the shell sits at the value's leading EDGE rather than being the
    /// value — `: (⏎// c⏎A)[]` and its indexed-access / conditional-check siblings — since
    /// there the seam has no node to substitute and the shell is still built
    /// ([`types::StrippedParenHang::claimed_shell`]). Where the shell IS the value it is
    /// substituted away and never reached, so no mark is needed.
    ///
    /// A **span**, not a position, because `unwrap_parenthesized` peels every layer: the
    /// claim covers `((⏎// c⏎A))`'s inner shell too, which holds the comment while the
    /// outer holds nothing. Save/restored around the build like
    /// [`Printer::claimed_owned_comment_start`] — a mark that outlived its emitter would
    /// suppress a run nothing else prints, which is a DROP
    /// ([`comments.md`](../../../../docs/comments.md) hazard 1).
    pub(crate) claimed_shell_leading_run: Cell<Option<Span>>,
    /// The parent context for a curried arrow-chain value, set by the enclosing
    /// printer (assignment chokepoint, call-argument printer, binary-operand
    /// printer) just before the chain is built. The arrow printer reads and
    /// clears it at entry so the outermost chain arrow picks the right flattened
    /// layout (assignment-RHS vs progressive call-arg/binaryish) while nested
    /// arrows don't inherit it. Mirrors prettier routing the parent context
    /// (`args.assignmentLayout`, `isCallLikeExpression(parent)`,
    /// `isBinaryish(parent)`) into `printArrowFunctionSignatures`.
    pub(crate) arrow_chain_context: Cell<ArrowChainContext>,
    /// Whether we're building the **init** clause of a C-style `for` header.
    /// In that clause an `in` binary expression must be parenthesized to keep it
    /// distinct from the `for (x in y)` separator — prettier parenthesizes every
    /// `in` anywhere lexically under the init (not just where strictly required),
    /// so this flag is set while building the init subtree and propagates through
    /// it (including nested function/class bodies). Read by `needs_parens` and the
    /// surgical `in`-wrap at positions that build an expression without a
    /// `needs_parens` check (assignment RHS, ternary branches/test).
    /// Uses Cell for interior mutability so doc builders (&self) can set this.
    pub(crate) in_for_init: Cell<bool>,
    /// Whether the scoped doc-share map for member-chain building is active: an AST
    /// node's pointer **plus a build tag** ([`ShareTag`]) → the `DocId` already built
    /// for it. A member chain renders the same group **flat** (`print_group`) and
    /// **expanded** (`print_group_expanded`) across `conditional_group` candidates;
    /// without sharing, each recursive build runs once per candidate and a nested chain
    /// in a call arg compounds to O(4^depth) — the member-chain rebuild blowup.
    ///
    /// **The tag is what makes a hit byte-identical.** A given node is reached under
    /// identical Printer state in both candidates *except* for `skip_arrow_chain` /
    /// `expand_last_arg_flat_params` (prettier's `expandLastArg`, which
    /// `calls/chain_args.rs` sets to build the hugged printing of an argument beside its
    /// `printedArguments` one), and except for which *builder* is asking — the argument
    /// builder or the expand-last body prebuild, which wraps an object body in the
    /// grammar's parens. Both distinctions ride in the tag, so each variant caches
    /// separately instead of the cache being refused for the whole node. Every other
    /// flag is statement-constant during a chain or set identically by the shared AST
    /// traversal.
    ///
    /// Active only between `enter_chain_arg_share`/`exit_chain_arg_share` (the outermost
    /// `build_chain_doc`); the pointer is stable, the AST arena being immutable during
    /// formatting. The map's *storage* is the doc arena's parked
    /// [`DocArena::share_map_scratch`] (cleared at both enter and exit, so between chains
    /// — and across printers sharing one arena — it is logically empty and only its table
    /// capacity persists, killing the per-printer `HashMap` resize chain).
    pub(crate) chain_arg_share_active: Cell<bool>,
    /// Expand-last-arg body reuse: `(body-expr span start, pre-built body DocId)`.
    /// The call/new expand-last paths build an arrow's call body **once** up front
    /// and set this before building the whole-arrow argument doc via
    /// `build_args_split_last`; [`Self::build_arrow_body_doc`] then returns the
    /// pre-built DocId for that exact node instead of rebuilding it. Building the
    /// body twice — inside the whole arrow *and* separately for the break-body
    /// state — recurses into itself for a call-bodied arrow whose body is another
    /// such call (`f(lead, x => f(lead, y => …))`), making the doc-node count
    /// O(2^depth). Reusing the one build keeps it linear and is byte-identical (the
    /// injected DocId is exactly what the arrow's own body build would produce for a
    /// call body — `build_arrow_body_doc` returns `build_expression_doc` there).
    /// Keyed by span (unique per source position); nested expand-last calls
    /// save/restore, so only the node currently being reused ever matches.
    pub(crate) arrow_body_inject: Cell<Option<(u32, DocId)>>,
    /// Whether the member chain currently being built has any comment anywhere in its
    /// span. Set once per `build_chain_doc` (save/restore, so a nested chain in a call
    /// arg / base restores the parent's value on exit — re-entrancy-safe like
    /// [`Self::chain_arg_share_active`]). The chain print path reads it to skip per-member
    /// comment classification when the whole chain is comment-free (the common case).
    /// Safe by construction: the flag only *enables* a skip whose soundness is
    /// span-containment (a member's comment gap ⊆ the chain span), so a stale value can
    /// only cause more work, never a dropped comment. Defaults `true` (do the full
    /// classify) so any member print reached without a preceding set is fail-safe.
    pub(crate) chain_has_comments: Cell<bool>,
}

impl<'a> Printer<'a> {
    /// Create a new printer borrowing the given arena, [`PrinterInputs`], and
    /// embedding context.
    ///
    /// `buffer_capacity` pre-sizes the output buffer: the source length for the
    /// rendering path, or `0` for doc-only embedding builds that never write
    /// output (see `make_printer` / `make_doc_printer` in `lib.rs`).
    pub(crate) fn with_context(
        arena: &'a DocArena,
        inputs: &PrinterInputs<'a>,
        embed: EmbedContext,
        buffer_capacity: usize,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(buffer_capacity),
            indent_level: 0,
            embed,
            arena,
            source: inputs.source,
            comments: inputs.comments,
            has_owned_comments: inputs.has_owned_comments,
            has_format_ignore: inputs.has_format_ignore,
            layout_line_breaks: inputs.line_breaks,
            // Normal path: comment classification shares the one real table. The
            // canonical path re-points `layout_line_breaks` at an empty table but
            // leaves this one real (see `set_canonical`).
            comment_line_breaks: inputs.line_breaks,
            canonical: false,
            declaration_indent_depth: Cell::new(0),
            is_expression_statement: Cell::new(false),
            in_top_level_assignment: Cell::new(false),
            in_stacked_arrow_chain: Cell::new(false),
            skip_arrow_chain: Cell::new(false),
            expand_last_arg_flat_params: Cell::new(false),
            test_call_flat_params: Cell::new(false),
            arrow_body_object_parens_target: Cell::new(None),
            expr_stmt_paren_target: Cell::new(None),
            ternary_hang_target: Cell::new(None),
            jsdoc_cast_value_gap_target: Cell::new(None),
            jsdoc_cast_cannot_hang_target: Cell::new(None),
            inline_every_member_lookup: Cell::new(None),
            inline_member_call_tail: Cell::new(None),
            claimed_owned_comment_start: Cell::new(None),
            claimed_shell_leading_run: Cell::new(None),
            arrow_chain_context: Cell::new(ArrowChainContext::None),
            in_for_init: Cell::new(false),
            chain_arg_share_active: Cell::new(false),
            arrow_body_inject: Cell::new(None),
            chain_has_comments: Cell::new(true),
        }
    }

    /// Arm expand-last-arg body reuse for the node at `span`: the next
    /// [`Self::build_arrow_body_doc`] for that exact node returns `doc` instead of
    /// rebuilding it. Returns the previous injection (nested expand-last calls each
    /// arm their own, restoring the outer one after). See the `arrow_body_inject` field.
    pub(crate) fn inject_arrow_body(&self, span: u32, doc: DocId) -> Option<(u32, DocId)> {
        self.arrow_body_inject.replace(Some((span, doc)))
    }

    /// Restore the previous expand-last-arg body injection (from `inject_arrow_body`).
    pub(crate) fn restore_arrow_body_inject(&self, prev: Option<(u32, DocId)>) {
        self.arrow_body_inject.set(prev);
    }

    /// Run `build` with `inject` armed ([`Self::inject_arrow_body`]), restoring the previous
    /// injection after. `None` arms nothing and just runs `build`.
    ///
    /// The scoped form, because the injection is a single slot on the printer and every arm
    /// that arms one must hand it back — a call/`new` expand-last path re-arms the SAME
    /// injection around each printing of its argument, so the pairs interleave and a
    /// hand-rolled `if let Some(prev)` per site is where one goes missing.
    pub(crate) fn with_arrow_body_inject<R>(
        &self,
        inject: Option<(u32, DocId)>,
        build: impl FnOnce() -> R,
    ) -> R {
        let Some((span, doc)) = inject else {
            return build();
        };
        let prev = self.inject_arrow_body(span, doc);
        let out = build();
        self.restore_arrow_body_inject(prev);
        out
    }

    /// Wrap `doc` in parens when `expr` is an `in` binary built directly inside a
    /// `for` header init. Used at positions that build an expression *without* a
    /// [`fn@needs_parens`] check — assignment RHS, ternary branches/test, and the
    /// init clause's own expression/declarator — so the for-init `in` rule still
    /// applies there. Positions that already route through [`fn@needs_parens`] (call
    /// args, object values, binary operands, …) get the same wrap via that path.
    #[inline]
    pub(crate) fn wrap_for_init_in(&self, expr: &internal::Expression<'_>, doc: DocId) -> DocId {
        if self.in_for_init.get() && is_in_binary(expr) {
            self.arena.parens(doc)
        } else {
            doc
        }
    }

    /// Print-context-aware wrapper over the free [`fn@needs_parens`]: supplies the
    /// ambient for-header-init flag so the for-init `in` rule applies at every
    /// call site without threading the flag by hand. Prefer this inside `Printer`
    /// methods; the free function (which still requires the flag explicitly) is
    /// only for the few free helpers that have no `self`.
    #[inline]
    pub(crate) fn needs_parens(&self, expr: &internal::Expression<'_>, ctx: ParenContext) -> bool {
        needs_parens(expr, ctx, self.in_for_init.get())
    }

    /// Get a reference to the doc arena.
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        self.arena
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write a DocId to the buffer, accounting for current column and indent level
    ///
    /// This handles the common pattern of:
    /// 1. Calculate current column with context offset
    /// 2. Print doc with indent-aware width calculations
    /// 3. Write the result to the buffer
    ///
    /// For width calculations, we account for outer context in two ways:
    /// - If `first_line_offset > 0`: expression is embedded inline (e.g., Svelte block), use it directly
    /// - If `first_line_offset == 0`: standalone block (e.g., `<script>`), use `base_indent_offset * tab_width`
    pub(crate) fn write_arena_doc(&mut self, d: DocId) {
        let context_offset = if self.embed.first_line_offset > 0 {
            if self.current_column() == 0 {
                self.embed.first_line_offset
            } else {
                0
            }
        } else {
            self.embed.base_indent_offset * TAB_WIDTH
        };
        let current_col = self.current_column() + context_offset;
        // Render into the arena-parked scratch: one warm buffer across the
        // file's renders (the whole program standalone; one per template
        // expression when Svelte-embedded) instead of an alloc/free per call.
        let mut output = self.arena.take_render_scratch();
        // Pass the document source so `DocText::SourceSpan` nodes (verbatim
        // comment/literal slices) resolve at render without a `DocArena` lifetime.
        doc::arena_print_doc_with_indent_resolved_into(
            self.arena,
            d,
            &self.embed,
            current_col,
            self.indent_level,
            self.source,
            &mut output,
        );
        self.write(&output);
        self.arena.park_render_scratch(output);
    }

    /// Render an arena DocId to a flat string with effectively infinite width.
    pub(crate) fn render_arena_doc_flat(&self, d: DocId) -> String {
        doc::arena_measure_doc_flat_resolved(self.arena, d, &self.embed, self.source)
    }

    /// Get the formatted output
    pub(crate) fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Get the current column position (for doc-builder width calculations)
    pub(crate) fn current_column(&self) -> usize {
        self.buffer.current_column(TAB_WIDTH)
    }

    /// Compute the visual indent width at a source position.
    ///
    /// Finds the start of the line containing `pos` and measures the leading
    /// whitespace visual width (tabs count as `tab_width` chars).
    pub(crate) fn source_indent_visual(&self, pos: u32) -> usize {
        let pos = pos as usize;
        let line_start = self.source[..pos].rfind('\n').map_or(0, |i| i + 1);
        printing::visual_width(&self.source[line_start..pos], TAB_WIDTH)
    }

    /// Check if two positions are on the same line (O(log n) binary search).
    ///
    /// Reads `comment_line_breaks` (not `line_breaks`) so comment position
    /// classification stays correct even when the canonical path has emptied the
    /// layout table. In the normal path the two tables are identical.
    #[inline]
    pub(crate) fn is_same_line(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::is_same_line_fast(self.comment_line_breaks, prev_end, curr_start)
    }

    /// Switch this printer into the intent-erased *canonical* reprint mode.
    ///
    /// Empties the layout line-break table (so `has_blank_line_between` /
    /// `has_newline_between` and every direct `*_fast` reader collapse — no blank
    /// lines, nothing forced multiline by a source newline) and sets the
    /// `canonical` flag that gates the direct source-newline scans. Comment
    /// classification keeps the real `comment_line_breaks` table, so comments are
    /// preserved losslessly (merely re-placed deterministically). Cold path:
    /// called once, right after construction, before any doc is built.
    ///
    /// Erasing intent takes two mechanisms because there are two kinds of read, and
    /// only one of them is self-stabilizing:
    ///
    /// - **Table reads** (this empty table) erase automatically, and stay idempotent
    ///   for free: an empty table answers "no newline anywhere" regardless of what
    ///   the source said, so pass 1 and pass 2 agree by construction. A future
    ///   layout read routed through `layout_line_breaks` needs no further thought.
    /// - **Direct source scans** (`self.source[..].contains('\n')`) bypass the table
    ///   entirely, so they see the original source in pass 1 and the collapsed
    ///   output in pass 2 and *disagree*. Each one must be gated on `canonical`.
    ///
    /// That asymmetry is the rule for new code: route a layout read through the
    /// table and it is handled; scan the source directly and you must gate it.
    pub(crate) fn set_canonical(&mut self) {
        self.canonical = true;
        // `&[]` is `&'static`, which coerces to the field's `'a`.
        self.layout_line_breaks = &[];
    }

    /// Check if there's a blank line (2+ newlines) between two positions (O(log n) binary search)
    ///
    /// A *layout* read: erased in the canonical reprint (see [`Self::set_canonical`]).
    #[inline]
    pub(crate) fn has_blank_line_between(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::has_blank_line_between_fast(self.layout_line_breaks, prev_end, curr_start)
    }

    /// Check if there's any newline between two positions (O(log n) binary search)
    ///
    /// A *layout* read: erased in the canonical reprint (see [`Self::set_canonical`]).
    /// For comment adjacency use [`Self::comment_has_newline_between`] instead —
    /// this one reports "no newline" for everything under `set_canonical`.
    #[inline]
    pub(crate) fn has_newline_between(&self, start: u32, end: u32) -> bool {
        printing::has_newline_between_fast(self.layout_line_breaks, start, end)
    }

    /// [`Self::has_newline_between`] against the *comment* line-break table.
    ///
    /// For comment-adjacency classification — is a comment on its neighbor's
    /// source line? — which must stay real in canonical mode ([`Self::set_canonical`])
    /// just like [`Self::is_same_line`]: these reads decide whether a `//` line
    /// comment's emission is followed by a break, and erasing them lets content
    /// trail onto a line-comment's output line (swallow / merge — content loss).
    /// In the normal path the two tables are identical, so this is byte-identical
    /// to `has_newline_between` there.
    #[inline]
    pub(crate) fn comment_has_newline_between(&self, start: u32, end: u32) -> bool {
        printing::has_newline_between_fast(self.comment_line_breaks, start, end)
    }

    /// Wrap content and closing line with declaration indent depth handling
    ///
    /// In multi-declarator contexts (declaration_indent_depth > 0), content gets
    /// double-indented and the closing line gets single extra indent. This creates
    /// the proper visual alignment for:
    /// ```javascript
    /// const a = {
    ///         prop: value,
    ///     },
    ///     b = 2;
    /// ```
    pub(crate) fn wrap_with_decl_indent(
        &self,
        inner: DocId,
        closing_line: DocId,
    ) -> (DocId, DocId) {
        let d = self.d();
        if self.declaration_indent_depth.get() > 0 {
            (d.indent(d.indent(inner)), d.indent(closing_line))
        } else {
            (d.indent(inner), closing_line)
        }
    }

    /// The share-map key for `node` under `builder`, or `None` when no member chain is
    /// building and there is nothing to share with.
    ///
    /// The key carries the builder AND the two `expandLastArg` flags, which are the only
    /// Printer state a chain's flat and expanded candidates reach a given node under
    /// differently — so a hit is byte-identical to a rebuild by construction rather than
    /// by the caller having checked. See the `chain_arg_share_active` field doc.
    pub(crate) fn chain_share_key<T>(&self, node: &T, builder: ShareTag) -> Option<(usize, u8)> {
        if !self.chain_arg_share_active.get() {
            return None;
        }
        let mut tag = builder as u8;
        if self.skip_arrow_chain.get() {
            tag |= ShareTag::SKIP_ARROW_CHAIN_BIT;
        }
        if self.expand_last_arg_flat_params.get() {
            tag |= ShareTag::FLAT_PARAMS_BIT;
        }
        Some((std::ptr::from_ref(node) as usize, tag))
    }

    /// Look up `key` in the chain share map, or build and record it.
    pub(crate) fn chain_shared_doc(
        &self,
        key: Option<(usize, u8)>,
        build: impl FnOnce() -> DocId,
    ) -> DocId {
        let Some(key) = key else {
            return build();
        };
        let share_map = self.arena.share_map_scratch();
        if let Some(&doc) = share_map.borrow().get(&key) {
            return doc;
        }
        let doc = build();
        share_map.borrow_mut().insert(key, doc);
        doc
    }

    /// Activate the chain-arg share map for the outermost `build_chain_doc` only. Returns the
    /// prior active state; nested chains observe `true` and become no-ops (the map
    /// persists across the whole top-level chain so every nesting level shares).
    pub(crate) fn enter_chain_arg_share(&self) -> bool {
        let was_active = self.chain_arg_share_active.get();
        if !was_active {
            self.chain_arg_share_active.set(true);
            self.arena.share_map_scratch().borrow_mut().clear();
        }
        was_active
    }

    /// Deactivate + clear the chain-arg share map when leaving the outermost `build_chain_doc`
    /// (`was_active` false). Nested exits are no-ops.
    pub(crate) fn exit_chain_arg_share(&self, was_active: bool) {
        if !was_active {
            self.chain_arg_share_active.set(false);
            self.arena.share_map_scratch().borrow_mut().clear();
        }
    }

    /// Check if identifier has a complex type annotation (nested generics)
    ///
    /// Corresponds to prettier's `hasComplexTypeAnnotation`:
    /// - Type reference with >1 type parameters
    /// - At least one type param has nested generics OR is a conditional type
    ///
    /// Example: `Map<string, Array<number>>` - Map has 2 params, second has nested generic
    pub(crate) fn id_has_complex_type_annotation(&self, expr: &internal::Expression<'_>) -> bool {
        crate::pattern_type_annotation(expr)
            .is_some_and(|ann| self.type_has_complex_annotation(ann.type_annotation))
    }

    /// Check if a type has complex nested type parameters
    fn type_has_complex_annotation(&self, ts_type: &internal::TSType<'_>) -> bool {
        match ts_type {
            internal::TSType::TypeReference(type_ref) => {
                // Must have >1 type argument
                let type_args = match &type_ref.type_arguments {
                    Some(args) => &args.params,
                    None => return false,
                };

                if type_args.len() <= 1 {
                    return false;
                }

                // At least one arg must have nested generics or be a conditional type
                type_args
                    .iter()
                    .any(|param| self.type_has_nested_generics(param))
            }
            _ => false,
        }
    }

    /// Check if a type has nested type parameters or is a conditional type
    fn type_has_nested_generics(&self, ts_type: &internal::TSType<'_>) -> bool {
        match ts_type {
            internal::TSType::TypeReference(type_ref) => {
                // Has type arguments means nested generics
                type_ref.type_arguments.is_some()
            }
            internal::TSType::Conditional(_) => true,
            _ => false,
        }
    }

    /// Check if a type alias has complex type parameters
    ///
    /// Corresponds to prettier's `isComplexTypeAliasParams`:
    /// - >1 type parameter
    /// - At least one has a constraint or default value
    ///
    /// Example: `type Foo<T extends string, U = number> = ...`
    pub(crate) fn type_alias_has_complex_params(
        &self,
        type_params: Option<&internal::TSTypeParameterDeclaration<'_>>,
    ) -> bool {
        let params = match type_params {
            Some(p) => &p.params,
            None => return false,
        };

        if params.len() <= 1 {
            return false;
        }

        // At least one param has a constraint or default
        params
            .iter()
            .any(|param| param.constraint.is_some() || param.default.is_some())
    }

    /// Check if identifier has complex destructuring pattern
    ///
    /// Corresponds to prettier's `isComplexDestructuring`:
    /// - ObjectPattern with >2 properties
    /// - At least one property has a default value OR is not shorthand
    ///
    /// Example: `const { a, b = 1, c } = obj` - 3 properties, one has default
    pub(crate) fn id_has_complex_destructuring(&self, expr: &internal::Expression<'_>) -> bool {
        let internal::Expression::ObjectPattern(obj) = expr else {
            return false;
        };

        if obj.properties.len() <= 2 {
            return false;
        }

        // At least one property has a default value or is not shorthand
        obj.properties.iter().any(|prop| {
            match prop {
                internal::ObjectPatternProperty::Property(p) => {
                    // Has default if value is AssignmentPattern
                    let has_default = matches!(p.value, internal::Expression::AssignmentPattern(_));
                    // Not shorthand if key != value
                    let not_shorthand = !p.shorthand;
                    has_default || not_shorthand
                }
                internal::ObjectPatternProperty::RestElement(_) => false,
            }
        })
    }

    /// Find the position of `=` character in the source between two positions
    /// Skips over comments to avoid matching `=` inside them.
    /// Also skips `==` and `===` comparison operators (we want assignment `=`).
    pub(crate) fn find_equals_position(&self, start: u32, end: u32) -> u32 {
        let bytes = self.source.as_bytes();
        let start_pos = start as usize;
        let end_pos = end as usize;
        let mut i = start_pos;

        while i < end_pos {
            if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, end_pos) {
                i = new_i;
                continue;
            }
            // Check for assignment `=` (not `==` or `===`)
            if bytes[i] == b'=' && (i + 1 >= end_pos || bytes[i + 1] != b'=') {
                return i as u32;
            }
            i += 1;
        }
        // Fallback: return midpoint if `=` not found
        usize::midpoint(start_pos, end_pos) as u32
    }

    /// **to emit**: whether *this* caller has a comment to print in `[start, end)`.
    ///
    /// Skips a comment a node owns and prints itself
    /// ([`tsv_lang::Comment::owned_by_node`]). See `tsv_lang::comment` for the three
    /// axes — a *layout* gate wants [`Self::has_comments_on_page_between`] and a source
    /// cursor wants [`Self::comments_in_source_between`].
    pub(crate) fn has_comments_to_emit_between(&self, start: u32, end: u32) -> bool {
        has_comments_to_emit_in_range(self.comments, start, end)
    }

    /// **on page**: whether any comment occupies the page in `[start, end)` — an owned
    /// comment **counted**.
    ///
    /// The existence check for a *layout* gate (break / expand / hug / paren /
    /// fast-path). An owned comment is printed by its own node rather than by this gap,
    /// but it is still in the output and still occupies width, so a layout decision must
    /// see it. Using [`Self::has_comments_to_emit_between`] here makes the comment
    /// silently vanish from a decision it is visibly part of.
    pub(crate) fn has_comments_on_page_between(&self, start: u32, end: u32) -> bool {
        tsv_lang::has_comments_on_page_in_range(self.comments, start, end)
    }

    /// **on page**: every comment occupying the page in `[start, end)` — an owned comment
    /// **counted**.
    ///
    /// The iterator form of [`Self::has_comments_on_page_between`], for a layout gate whose
    /// rule is per-comment (`.any(|c| …)`). Same membership as
    /// [`Self::comments_in_source_between`] — on-page and in-source both count an owned
    /// comment, only *to emit* skips it — but the name says which question is being asked.
    pub(crate) fn comments_on_page_between(
        &self,
        start: u32,
        end: u32,
    ) -> impl Iterator<Item = &'a internal::Comment> {
        tsv_lang::comments_on_page_in_range(self.comments, start, end)
    }

    /// **in source**: every comment physically inside `[start, end)` — an owned comment
    /// **counted**.
    ///
    /// For a cursor stepping over comment *bytes*: a blank-line scan, an offset, a
    /// `prev_end`. The bytes are in the file regardless of who prints them, so a scan
    /// that skipped an owned comment would read its own newlines as an author's blank
    /// line — and emit a blank line that was never written.
    pub(crate) fn comments_in_source_between(
        &self,
        start: u32,
        end: u32,
    ) -> impl Iterator<Item = &'a internal::Comment> {
        tsv_lang::comments_in_source_range(self.comments, start, end)
    }

    /// Find the first occurrence of a byte in source between `start` and `end`
    /// that is NOT inside a comment. Returns absolute position.
    pub(crate) fn find_char_outside_comments(&self, start: u32, end: u32, ch: u8) -> Option<u32> {
        tsv_lang::source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            ch,
        )
        .map(|pos| pos as u32)
    }

    /// Position of the comma separating two consecutive list items in
    /// `[prev_end, next_start)`, ignoring commas inside comments. The shared anchor
    /// for splitting a gap's comments into before-comma (trailing the previous item)
    /// and after-comma (leading the next / stranded). Falls back to `next_start` when
    /// none is found — a defensive case (list items always have a real separator);
    /// the fallback keeps the split lossless: the whole gap then reads as before-comma
    /// (trailing the previous item), so no comment is dropped.
    pub(crate) fn comma_between(&self, prev_end: u32, next_start: u32) -> u32 {
        self.find_char_outside_comments(prev_end, next_start, b',')
            .unwrap_or(next_start)
    }

    /// Check if there are line comments (// style) between two positions
    ///
    /// Uses binary search: O(log n + k) where k is comments in range
    pub(crate) fn has_line_comments_between(&self, start: u32, end: u32) -> bool {
        has_line_comments_in_range(self.comments, start, end)
    }

    /// Check if there are multiline block comments between two positions
    ///
    /// Multiline block comments (containing newlines) force break-after-operator
    /// layout in assignments and property values.
    /// Prettier ref: `hasLeadingOwnLineComment` in assignment.js `chooseLayout`
    pub(crate) fn has_multiline_block_comments_on_page_between(
        &self,
        start: u32,
        end: u32,
    ) -> bool {
        tsv_lang::has_multiline_block_comments_on_page_in_range(self.comments, start, end)
    }

    /// Whether comments in the range force the following value onto its own line.
    /// Two comment shapes hang the value: a **line** comment (runs to
    /// end-of-line — inlining would swallow the value), and a **multiline** block
    /// comment the author wrote on its own line (`kw⏎/* … */⏎v`, i.e. a newline
    /// after it). Everything else collapses to the inline form (`kw /* c */ v`):
    /// a single-line block in *any* position (glued, trailing the keyword, or
    /// own-line), and a **glued** multiline block — one whose operand shares the
    /// comment's closing line (`kw /* …⏎… */ v`), the way prettier keeps it.
    ///
    /// This is the gate for the keyword→value gaps (as/satisfies,
    /// heritage/conditional `extends`, keyof/typeof/readonly, infer,
    /// type-param constraint/default, predicate `is`, indexed access,
    /// `export default`) and the type-alias `=` layout. Keying the multiline case on the newline *after*
    /// the comment (not before) keeps it idempotent: a block glued to the value
    /// stays inline even at line start in already-broken output, and only an
    /// authored break hangs it. Contrast
    /// [`Self::comment_hangs_binary_operand`], which *also* hangs a
    /// single-line own-line block (it has no `c.multiline` guard) but demands the
    /// comment genuinely own its line, which this one never asks; use that variant
    /// only at the one carve-out site where prettier *keeps* that break
    /// (binary/logical operands).
    /// What the chain linearizer reads off the input — the source and the comment table
    /// as one value, so the two cannot be handed over separately at a call site.
    pub(crate) fn linearize_input(&self) -> chain::LinearizeInput<'_> {
        chain::LinearizeInput {
            source: self.source,
            comments: self.comments,
        }
    }

    pub(crate) fn comments_force_own_line_between(&self, start: u32, end: u32) -> bool {
        self.any_comment_on_page_with_next(start, end, |c, next| self.comment_hangs_next(c, next))
    }

    /// Whether this one comment hangs what follows it onto its own line, where `next` is
    /// the start of the following comment, or the gap's end for the last.
    ///
    /// The single statement of the rule: a **line** comment (runs to end-of-line, so
    /// inlining would swallow what follows) or a **multiline** block the author wrote on
    /// its own line (a newline after it — inlining would reflow the author's break).
    /// Everything else collapses inline. The hang counterpart of
    /// [`Self::comment_hugs_next`], and keyed like it on what follows the comment.
    ///
    /// One question, one predicate: both the gate
    /// ([`Self::comments_force_own_line_between`]) and the emitter that gate selects
    /// ([`Self::build_trailing_comments_hang_next`]) ask it, so they cannot answer
    /// differently. ⚠️ Do not re-derive it at a call site. Keying an emitter on
    /// `is_block` alone reads as plausible code but silently collapses the own-line
    /// multiline case the gate had just flagged as hanging — and then a layout keyed on
    /// the authored newline is decided by a newline that very collapse destroys, so the
    /// format stops being idempotent on its own output.
    ///
    /// Contrast [`Self::comment_cannot_glue_to_operator`], the operator-glue rule, which keys on
    /// the newline *before* a comment and hangs an own-line **single-line** block too.
    ///
    /// ⚠️ A third shape hangs: an **honored format-ignore directive**
    /// ([`Self::is_honored_directive`]), whatever its spelling. A directive that shares its
    /// line with what follows is inert under the placement floor, so collapsing one inline
    /// would silently cost the freeze it earns on the very next pass — the wrong output being
    /// its own fixed point, invisible to every gate. It belongs in this predicate rather than
    /// at the emitters because the gate and the emitter must keep answering as one, and
    /// because the rule is about the DIRECTIVE, not about whether this particular gap freezes:
    /// a gap that doesn't freeze today can only start honoring one if the placement survives
    /// to be read. The leading-separator half is
    /// [`Self::leading_comment_is_honored_directive`], and the same rule at the declaration
    /// headers is `Printer::build_header_comment_run`.
    pub(crate) fn comment_hangs_next(&self, c: &internal::Comment, next: u32) -> bool {
        !c.is_block
            || (c.multiline && self.has_newline_between(c.span.end, next))
            || self.is_honored_directive(c)
    }

    /// Whether a comment in `(start, end)` forces the *following* value onto its own
    /// line: a line comment (runs to EOL), or a block comment the author gave a line of
    /// its **own** — a newline after it (toward the next comment, or `end` for the last)
    /// **and** nothing before it on its line. Keying the newline half on what comes
    /// *after* the comment (not before) keeps the layout idempotent: a block glued to the
    /// value (`/* c */ v`, even at line start in already-broken output) stays inline, and
    /// only an authored break (`/* c */⏎v`) forces the value down.
    ///
    /// ⚠️ **Both halves, per comment — this is prettier's `printLeadingComment` hardline
    /// arm, not its `hasLeadingOwnLineComment`.** The newline-after half alone reads the
    /// LAST comment of a run the author glued onto one line as owning it (`x +⏎/* c1 */
    /// /* c2 */⏎y`: `c2` has a newline after it, `c1` one before, and neither owns a
    /// line), which forced the chain — and its enclosing `=` — open on an expression
    /// prettier keeps flat (`docs/comments.md` §Own-line-ness is a SOURCE question).
    /// `hasLeadingOwnLineComment` really is the newline-after half alone, but prettier
    /// asks it at the **assignment** seam, of the node the comments lead; at an
    /// operator→operand gap prettier asks nothing and prints the run. Pinned by
    /// `expressions/binary/operator_glued_comment_run` and, for the position tsv holds
    /// against prettier's relocation, `operator_trailing_block_comment_prettier_divergence`.
    ///
    /// ⚠️ **Site-specific — the name says which.** This serves binary/logical
    /// operands only (`operators.rs`), where prettier *keeps* an own-line operand break
    /// so hanging is the smaller divergence than collapsing. It is NOT a
    /// general keyword→value gate: `export default` reached for it (under its old,
    /// general-sounding name) and became the lone value gap preserving an unforced
    /// break, disagreeing with its own twin `export =`. A keyword→value gap wants
    /// [`Self::comments_force_own_line_between`]; an operator→value gap wants
    /// [`Self::comment_cannot_glue_to_operator`]. Contrast
    /// [`Self::comments_force_own_line_between`], which collapses an authored
    /// own-line single-line block inline; that is the gate for every other
    /// keyword→value gap.
    ///
    /// ⚠️ **A gap whose shell is a real GROUP wants no gate at all** — the third case,
    /// and the one the routing rule above will mislead you into missing. The unary
    /// comment-holder parens took `comment_cannot_glue_to_operator` and pre-empted their
    /// own width decision with it (`docs/comments.md` §Own-line-ness is a SOURCE
    /// question); they now emit prettier's `group(["(", indent([softline, …]), softline,
    /// ")"])` and let the leading run's own soft `line` decide. Reach for a gate only
    /// where the layout genuinely cannot express both forms.
    pub(crate) fn comment_hangs_binary_operand(&self, start: u32, end: u32) -> bool {
        // The glue half asks the comment's own neighbours ([`Self::comment_hugs_next`]),
        // never the distance to `next` — for the last comment that is the operand's span
        // start, which sits inside any grouping paren the author wrote, so a break they
        // put after the `(` read as a break after the comment
        // ([`Self::comment_hangs_value_after_operator`] carries the family-wide note). It
        // subsumes the `!c.is_block` arm too: a line comment never hugs what follows.
        self.any_comment_on_page(start, end, |c| {
            !self.comment_hugs_next(c) && self.is_own_line_comment(c)
        })
    }

    /// Whether the gap's comment run is **glued through to the value**: no comment in
    /// `(start, end)` has a newline after it (toward the next comment, or `end` for
    /// the last). Line breaks *inside* a preserved multiline block don't count — the
    /// run still delivers the value on its closing line, so a `will_break` on the
    /// run's doc is the comment's interior, not an own-line separator. The to-emit
    /// counterpart of `OwnedCommentEffect::Pins`, feeding `RhsCommentInfo::pinned`:
    /// prettier keeps such a run on the operator's line
    /// (`= /* c */ /* x⏎y */ v`, `hasLeadingOwnLineComment` false — it keys on this
    /// same trailing newline), where a bare `will_break` reading hangs the value.
    ///
    /// **on page**: an owned member of the run (`/* x⏎y */` glued to the value) is
    /// part of the glue geometry even though the gap emits nothing for it.
    pub(crate) fn comment_run_glued_through(&self, start: u32, end: u32) -> bool {
        // Per comment, against its OWN neighbours ([`Self::comment_hugs_next`]) rather
        // than against a boundary: measuring the last comment's glue by the distance to
        // `end` reads whatever the printer erases or the value's shell contributes. A
        // grouping paren between the run and the value (`= /* c */ (⏎v)`) put the
        // author's break INSIDE the shell and reported the run as not-glued, which hung a
        // value prettier keeps on the operator's line — and, once the sibling gate
        // stopped agreeing with it, made the pair non-idempotent.
        !self.any_comment_on_page(start, end, |c| !self.comment_hugs_next(c))
    }

    /// Whether a comment in the **operator→value** gap hangs the value under the
    /// operator — the one rule, for the one family that asks it.
    ///
    /// Two conjuncts, and neither is sufficient alone:
    ///
    /// - the comment does not glue to what follows it ([`Self::comment_hugs_next`],
    ///   prettier's `hasNewline(text, locEnd(comment))`), **and**
    /// - it is a kind that cannot glue to the operator behind it either
    ///   ([`Self::comment_cannot_glue_to_operator`] — a line comment, a multiline block,
    ///   or an own-line block).
    ///
    /// Dropping the first hangs the head of a run whose glue chain reaches the value
    /// (`= /* c */ /* x⏎y */ v` and `=⏎/* c */ /* x⏎y */ v` both collapse inline in both
    /// formatters). Dropping the second hangs a single-line block the author merely broke
    /// after (`= /* c */⏎v`), which prettier keeps on the operator's line.
    ///
    /// ⚠️ **The glue half asks each comment's own NEIGHBOURS, never a boundary.** Every
    /// site that spelled it as a distance to the value's span start was blind to a
    /// grouping paren standing between them: the author's break sits INSIDE the shell
    /// (`= /* c */ (⏎v)`), the printer does not reproduce it, and the gate read it as a
    /// break after the comment and hung a value prettier hugs. Six sites shared that one
    /// defect, which is why the question is stated here once rather than at each of them;
    /// the enumeration and the fixtures live in `docs/comments.md` §Own-line-ness is a
    /// SOURCE question, so this doc does not keep a second copy of the list to drift.
    ///
    /// The **operator→value** rule. Contrast [`Self::comments_force_own_line_between`],
    /// the keyword→value rule, which collapses an authored own-line single-line block
    /// where this one preserves it — each is right for its family, and prettier agrees
    /// with both.
    ///
    /// The **on-page** axis: an owned comment glued to the value is printed by the
    /// value's own doc, but it still occupies the page and still decides this layout.
    pub(crate) fn comment_hangs_value_after_operator(&self, start: u32, end: u32) -> bool {
        self.any_comment_on_page(start, end, |c| {
            !self.comment_hugs_next(c) && self.comment_cannot_glue_to_operator(c)
        })
    }

    /// Whether a comment in `(start, end)` is separated from what follows it (the
    /// next comment, or `end`) by a blank line. Used where a blank line after a
    /// comment is itself a break trigger — e.g. a ternary branch (`a ? /* c */⏎⏎b`),
    /// where prettier breaks on the blank even though the own-line comment alone
    /// does not.
    ///
    /// A **break gate must ask its emitter's question in the emitter's spelling**, so this
    /// is the STRICT scan: the ternary branch gap's own blank emitter takes it, and `end`
    /// there is a branch span that starts inside a stripped paren shell. The table-only
    /// count reads the erased `(`'s two line breaks as an author blank and opens a ternary
    /// that fits — the gate and the emitter fabricating in lockstep, which is why the
    /// output stayed a fixed point and only a prettier `compare` found it.
    pub(crate) fn comment_followed_by_blank(&self, start: u32, end: u32) -> bool {
        self.any_comment_on_page_with_next(start, end, |c, next| {
            self.has_blank_line_between_strict(c.span.end, next)
        })
    }

    /// Scan the comments in `(start, end)`, returning true if `pred(comment)` holds for
    /// any — the anchor-free sibling of [`Self::any_comment_on_page_with_next`], for a
    /// gate whose per-comment rule asks only the comment's own neighbours.
    ///
    /// ⚠️ **Prefer this whenever the rule does not need the anchor.** The anchor is the
    /// distance to the NEXT comment or to `end`, and for the last comment `end` is the
    /// value's span start — which sits inside any grouping paren the author wrote. Every
    /// glue question spelled against it was blind to that shell
    /// ([`Self::comment_hangs_value_after_operator`] carries the account), so a gate that
    /// binds `next` only to ignore it is inviting the next author to re-derive glue from
    /// the wrong thing.
    ///
    /// **on page**, like its sibling: an owned comment is printed by its node's doc but
    /// still occupies the page, so a layout gate must count it.
    fn any_comment_on_page(
        &self,
        start: u32,
        end: u32,
        pred: impl Fn(&internal::Comment) -> bool,
    ) -> bool {
        tsv_lang::comments_in_source_range(self.comments, start, end).any(pred)
    }

    /// Scan the comments in `(start, end)` with one-ahead lookahead, returning true
    /// if `pred(comment, next_start)` holds for any — where `next_start` is the
    /// following comment's start, or `end` for the last. The shared primitive behind
    /// the gap predicates above (each keys a per-comment rule on the gap to whatever
    /// follows it). `peekable`, so no allocation.
    ///
    /// **on page**: every caller is a layout gate (prettier's `hasLeadingOwnLineComment`
    /// and friends — does the comment hang the value / force the break?). An owned
    /// annotation is on the page and hangs the value exactly as any other own-line comment
    /// does, so the scan is physical.
    fn any_comment_on_page_with_next(
        &self,
        start: u32,
        end: u32,
        pred: impl Fn(&internal::Comment, u32) -> bool,
    ) -> bool {
        let mut comments = tsv_lang::comments_in_source_range(self.comments, start, end).peekable();
        while let Some(c) = comments.next() {
            let next = comments.peek().map_or(end, |n| n.span.start);
            if pred(c, next) {
                return true;
            }
        }
        false
    }

    /// Whether anything at all precedes this comment on its physical line — the
    /// **source** reading of "does this comment trail what came before it", asked of
    /// BOTH comment kinds and taking no anchor: `has_newline_before_position` walks
    /// back over spaces/tabs, so a comment following another on the same line
    /// (`/* a */ /* b */`) follows content, and one the author put on a fresh line does
    /// not. Unlike the neighbor-anchored `is_same_line(prev, …)` /
    /// `has_newline_between(prev, …)` checks it is blind to nothing: the text it sees
    /// includes what no item span covers — a **stripped paren shell**'s `)` and a
    /// list's own **comma** — which is exactly why the element-comma seam asks it
    /// ([`Self::collect_trailing_comments`], `docs/comments.md` §The element-comma seam).
    pub(crate) fn comment_follows_content_on_its_line(&self, comment: &internal::Comment) -> bool {
        !has_newline_before_position(self.source, comment.span.start)
    }

    /// Whether a **trailer** — a comment following content on its own line — is the
    /// next thing after `pos` once closing punctuation is stepped over: only whitespace
    /// and closers (`)` `]` `}` `;` `,`) between `pos` and the comment's start. The
    /// **in-source** axis (an owned comment counts: it occupies the line all the same),
    /// read against the real source, so it stays true under [`Self::set_canonical`].
    ///
    /// The question a same-line-`//` deferral must ask before taking the line end for
    /// itself: a construct that closes flat after `pos` carries the trailer onto the
    /// same output line, where it welds onto the deferred one (`// c // c1`, the second
    /// `//` becoming text of the first) or is reordered behind it (`/* c1 */ // c`), so
    /// the deferral is lossless only when this is false. Reading through closers rather
    /// than only `pos`'s source line is what makes the answer stable across passes: the
    /// expanded layout's own reprint puts `);` on a line below the member. Conservative
    /// by design — a break the enclosing layout takes between the two only makes the
    /// caller's expansion unneeded, never wrong. A trailer behind a further TOKEN is out
    /// of reach — an operator's right side (`.bar as T; // c1`) and, the commoner
    /// spelling, a following sibling in a list (`foo(fn() // c⏎.bar, z); // c1`), where
    /// the scan stops on `z`. Whether such a trailer lands on the deferred comment's line
    /// is a layout fact no build-time read can see, so the residual weld there is a
    /// tracked open item, whose structural answer would be a renderer-level flush guard.
    pub(crate) fn trailer_follows_through_closers(&self, pos: u32) -> bool {
        comments_in_source_after(self.comments, pos)
            .next()
            .is_some_and(|c| {
                self.comment_follows_content_on_its_line(c)
                    && self.source.as_bytes()[pos as usize..c.span.start as usize]
                        .iter()
                        .all(|b| {
                            b.is_ascii_whitespace() || matches!(b, b')' | b']' | b'}' | b';' | b',')
                        })
            })
    }

    /// Whether a single comment occupies its own physical line — a line comment
    /// (always runs to end-of-line, so it owns whatever line it is on), or a block
    /// comment that starts a fresh line ([`Self::comment_follows_content_on_its_line`]).
    ///
    /// ⚠️ The `!is_block` short-circuit is a **layout** answer, and the reason this is
    /// not the predicate a comment-position seam wants: asked of a line comment it says
    /// "own line" whatever the author wrote before it on that line. A seam deciding
    /// which element a `//` trails must ask the physical question directly.
    pub(crate) fn is_own_line_comment(&self, comment: &internal::Comment) -> bool {
        !comment.is_block || !self.comment_follows_content_on_its_line(comment)
    }

    /// Whether a multi-line block comment **prints as indented lines**
    /// ([`tsv_lang::is_indentable_block`]) — the form `build_comment_doc` reprints as a
    /// [`tsv_lang::doc::arena::DocNode::MultilineText`] whose newlines are hard lines.
    ///
    /// The layout question a *glued* multi-line comment poses, and **not** `multiline`:
    /// only this form carries a break out to the enclosing group.
    ///
    /// ⚠️ **Not** `DocArena::will_break` on the comment's doc, which answers `true` for
    /// both shapes: tsv emits a preserved block's interior through `literalline`s (a
    /// genuine newline in the output, so `fits` must see it), where prettier emits one
    /// opaque string. That difference is deliberate and lives in the renderer; the
    /// *layout* question is this one.
    pub(crate) fn block_comment_is_indentable(&self, comment: &internal::Comment) -> bool {
        tsv_lang::is_indentable_block(self.source, comment)
    }

    /// Whether a block comment in `(start, end)` sits **alone on its own physical
    /// line** — a newline both before it ([`Self::is_own_line_comment`]) *and* after
    /// it (toward the next comment, or `end` for the last). This is the idempotent key
    /// for "keep the comment on its own line and break the value below it" across a
    /// keyword/operator that itself breaks (the type-alias `=`): prettier preserves an
    /// isolated own-line block (`type X =⏎/* c */⏎Y`) but **collapses** a block merely
    /// glued to the value across the `=`-break (`type X =⏎/* c */ Y` → inline, then
    /// width decides). Keying on the newline *before* alone ([`Self::is_own_line_comment`])
    /// would spuriously force the break on already-broken output and lose idempotency —
    /// the same hazard [`Self::comment_hangs_next`] documents.
    pub(crate) fn block_comment_isolated_own_line_between(&self, start: u32, end: u32) -> bool {
        self.any_comment_on_page_with_next(start, end, |c, next| {
            c.is_block && self.is_own_line_comment(c) && self.has_newline_between(c.span.end, next)
        })
    }

    /// Whether a comment must occupy its own line rather than gluing inline to the
    /// operator that precedes it: a line comment, a multiline block, or an own-line
    /// block (a newline precedes it). This is exactly the negation of the single-line
    /// glued block that
    /// [`build_rhs_comments_glued_opt`](Self::build_rhs_comments_glued_opt) hugs across
    /// a source newline, so the two stay in lockstep.
    ///
    /// The **operator→value** rule (assignment `=` and friends), as distinct from the
    /// **keyword→value** rule ([`Self::comments_force_own_line_between`]): the two
    /// differ on an own-line *single-line* block, which this hangs and that collapses.
    /// Both are right for their family — assignment preserves it
    /// (`assignment/rhs_block_comment_newline`), `as`/`keyof`/`export default` reflow
    /// it, and prettier agrees with each.
    ///
    /// ⚠️ A **line** comment satisfies this via `is_own_line_comment`'s `!is_block`
    /// disjunct, not via any newline scan — so this never collapses one, and reading
    /// the name as "own line means a newline precedes it" is a mistake that has been
    /// made. Open the definition before reasoning about a line comment here.
    pub(crate) fn comment_cannot_glue_to_operator(&self, comment: &internal::Comment) -> bool {
        comment.multiline || self.is_own_line_comment(comment)
    }

    /// Whether any comment in `[start, end)` forces a break **by a property the surrounding
    /// layout cannot manufacture** — a line comment, or a block whose own text spans lines.
    ///
    /// ⚠️ The **on-page** axis, deliberately: an owned comment is printed by its node's own
    /// doc rather than by the surrounding gap, but it still occupies the page, so a glued
    /// *multiline* block leading a parameter (`(/* a⏎b */ x) => …` — owned by `x`) breaks the
    /// signature exactly like an unowned one. Asking the emit axis here reports a region clear
    /// while its output spans three lines, which is hazard 2 in `docs/comments.md` and the
    /// standing rule that a layout gate is an on-page question.
    ///
    /// ⚠️ **Own-line-ness is deliberately NOT a disjunct here, and that is a STABILITY
    /// requirement rather than a scope choice.** It is the one break-forcing property a layout
    /// can *create*: break a parameter list and every comment glued to a `(` or a comma lands
    /// at a line start, becoming own-line in the output that a hug decision had already been
    /// taken on. A refusal that asks it over a region the break relocates is therefore
    /// self-fulfilling — `fn((/* c */ a,⏎⏎b) => call(a))` hugged on pass 1, the hug's own break
    /// moved `/* c */` to a line start, and pass 2 read it as forcing and expanded. That is an
    /// **F1 violation**, not a divergence, and `blanks:audit` is what catches it.
    ///
    /// A caller that genuinely needs the own-line kind must ask it only where breaking cannot
    /// introduce it — see [`Printer::arrow_trailing_param_comment_forces_break`], whose region
    /// sits *after* the last parameter, so a comment glued there stays glued to that parameter
    /// however the list breaks.
    pub(crate) fn range_has_layout_stable_break_forcing_comment(
        &self,
        start: u32,
        end: u32,
    ) -> bool {
        tsv_lang::comments_on_page_in_range(self.comments, start, end)
            .any(|comment| comment.multiline || !comment.is_block)
    }

    /// Check if a delimited list (tuple, type params, etc.) has line comments
    /// between any elements OR after the last element.
    ///
    /// Used to determine if a list should be forced to multiline formatting.
    pub(crate) fn has_line_comments_in_delimited_list<T, F>(
        &self,
        items: &[T],
        get_span: F,
        end_boundary: u32,
    ) -> bool
    where
        F: Fn(&T) -> Span,
    {
        let between = items.windows(2).any(|pair| {
            self.has_line_comments_between(get_span(&pair[0]).end, get_span(&pair[1]).start)
        });
        let trailing = items
            .last()
            .is_some_and(|last| self.has_line_comments_between(get_span(last).end, end_boundary));
        between || trailing
    }

    /// Check if a delimited list contains own-line single-line block comments.
    ///
    /// The per-comment classification is [`Printer::block_comment_owns_its_line`] — both
    /// halves read the SOURCE, never a neighbouring item's boundary — and that function
    /// carries the argument. What is this gate's own is the SCOPE it applies it over:
    /// which comments are in range, which are the list's rather than an element's, and
    /// whether an element follows at all.
    ///
    /// ⚠️ **This gate and the element→`,` seam must ask the same question.** The array
    /// literal proved the coupling: on the item-boundary reading the gate's blindness
    /// *cancelled* the seam's, so tightening either one alone lost a comment outright. The
    /// seam keys on the same source reading and partitions its gap by a split point
    /// ([`Printer::element_gap_split`]); a future family that strips a delimiter needs both
    /// halves, not just this one.
    ///
    /// `span` is the whole construct's span and only its ENDS are read (`span.start` as the
    /// opening delimiter, `span.end - 1` as the closing one), so the delimiter pair is the
    /// caller's business, not this function's: tuples and array patterns pass `[…]`,
    /// type-parameter/argument lists `<…>`, and the object pattern and specifier list
    /// `{…}`. Every such list is one delimiter byte at each end, which is the only shape
    /// assumption here — a caller whose node span runs PAST the closer (an object pattern's
    /// `: T` annotation) trims it back to the closer before calling.
    ///
    /// ⚠️ `get_printed_span` yields the item's **PRINTED** span, not necessarily its node
    /// span: the "inside an element" test below asks whether the ELEMENT'S OWN DOC prints
    /// the comment, and where a node's span was extended over an erased closer
    /// (`docs/comments.md` §The element-comma seam) it does not. The two coincide for most
    /// callers — a grouping paren is no node, so an argument's or a type item's span never
    /// swallows one — and differ exactly where the destructuring patterns' element ends
    /// past a stripped shell, which is why they narrow it
    /// ([`crate::ast::internal::Expression::printed_end`]). Handing back the node span
    /// there counts a shell-interior comment as the element's, and it reaches no gate at
    /// all while the seam emits it.
    pub(crate) fn has_own_line_block_comments_in_bracket_list<T, F>(
        &self,
        span: Span,
        items: &[T],
        get_printed_span: F,
    ) -> bool
    where
        F: Fn(&T) -> Span,
    {
        let open_bracket = span.start;

        for comment in
            tsv_lang::comments_in_source_range(self.comments, open_bracket + 1, span.end - 1)
        {
            if !comment.is_block || comment.multiline {
                continue;
            }

            // Skip comments that are inside an element (they belong to that element, not this list)
            let inside_element = items
                .iter()
                .any(|e| get_printed_span(e).contains(comment.span));
            if inside_element {
                continue;
            }

            // Whether an element follows at all — the only thing the position of the next
            // element is still asked for, the glue half reading the source instead.
            //
            // ⚠️ `>=`, not `>`: an element GLUED to the comment starts exactly where the
            // comment ends (`[a,⏎/* c */b]`), and a strict `>` skipped it — so a comment
            // that plainly leads the next element fell through to the dangling arm and
            // expanded the list. That was **non-idempotent**, not merely a divergence: the
            // expanded print re-emits the run spaced (`/* c */ b`), which un-glues it, so
            // the second pass collapsed. All seven families carried it, and the space is
            // the only thing that ever distinguished them — authorship no reader could
            // see.
            let element_follows = items
                .iter()
                .map(|e| get_printed_span(e).start)
                .any(|start| start >= comment.span.end);

            if self.block_comment_owns_its_line(comment, element_follows) {
                return true;
            }
        }
        false
    }

    /// Whether a bracketed comma list holds a comment that forces it OPEN.
    ///
    /// The three clauses every such list asks, stated once: a line comment in the
    /// opener→first-item gap, a line comment anywhere between or after the items, or an
    /// own-line single-line block comment ([`Self::has_own_line_block_comments_in_bracket_list`]).
    /// Held by the type-argument list, the type-parameter declaration and the
    /// import/export specifier list; a caller with a further question of its own `||`s it on.
    ///
    /// ⚠️ **The opener→first-item clause is the one that has been forgotten, and forgetting
    /// it loses content.** `has_line_comments_in_delimited_list` covers only *between* and
    /// *after* the items, so a `//` trailing the opener (`<// c⏎T>`) reaches no clause; the
    /// inline path then runs, emits block comments only, and the line comment is DROPPED.
    /// That is why the question lives here rather than at each site.
    ///
    /// The zero-comment window gate is an **on-page** question: it guards a LAYOUT
    /// decision, and an emit-keyed gate would skip an owned comment and answer for a page
    /// that is not empty. Every clause below is bounded within `span`, so with nothing on
    /// the page all three are provably false — this skips them on the common bare list.
    pub(crate) fn has_expanding_comments_in_bracket_list<T, F>(
        &self,
        span: Span,
        items: &[T],
        get_span: F,
    ) -> bool
    where
        F: Fn(&T) -> Span,
    {
        // An empty list short-circuits BEFORE the own-line-block clause, which reads
        // `element_follows: false` with no items and would call a lone own-line block in
        // an empty `<>` an expansion trigger.
        if items.is_empty() {
            return false;
        }
        if !self.has_comments_on_page_between(span.start, span.end) {
            return false;
        }
        self.has_expanding_line_comments_in_bracket_list(span, items, &get_span)
            || self.has_own_line_block_comments_in_bracket_list(span, items, &get_span)
    }

    /// The two LINE-comment clauses of [`Self::has_expanding_comments_in_bracket_list`] —
    /// a `//` in the opener→first-item gap, and one anywhere between or after the items.
    ///
    /// Split out for the caller whose own-line-BLOCK clause differs: the `with { … }`
    /// import-attribute clause, which must ask
    /// [`Printer::has_own_line_attribute_comments`] instead (a glued attribute makes the
    /// bracketed-list spelling non-idempotent — see that function). It is the only
    /// legitimate reason to bypass the three-clause predicate above; a caller with an
    /// *extra* question `||`s it onto that one rather than restating any clause here.
    ///
    /// ⚠️ **Both clauses take their bounds from `span`, and that is the point.** Restating
    /// them per site means restating the closing-delimiter offset, and the attribute clause
    /// got it wrong — it reached to the statement's `;` instead of the `}`, handing the
    /// `}`→`;` gap a second emitter on top of the caller's post-`;` run and DOUBLE-PRINTING
    /// every comment there (`docs/comments.md` §The element-comma seam). Deriving
    /// `span.end - 1` in one place makes that slip unspellable.
    pub(crate) fn has_expanding_line_comments_in_bracket_list<T, F>(
        &self,
        span: Span,
        items: &[T],
        get_span: F,
    ) -> bool
    where
        F: Fn(&T) -> Span,
    {
        let Some(first) = items.first() else {
            return false;
        };
        self.has_line_comments_between(span.start + 1, get_span(first).start)
            || self.has_line_comments_in_delimited_list(items, &get_span, span.end - 1)
    }

    /// Find the closing `)` between a start position and end boundary.
    ///
    /// Scans the source to find the `)` that closes the params. Returns
    /// the position AFTER the `)` for use as a boundary.
    pub(crate) fn find_closing_paren(&self, start: u32, end: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        let end = (end as usize).min(source.len());
        let mut depth = 0;
        let mut i = start as usize;
        // Just past the last significant byte — the anchor `is_regex_start_after`
        // reads. A skipped string ends an operand; a comment leaves it alone.
        let mut operand_end = start as usize;

        while i < end {
            if let Some(past) = skip_trivia(source, i, end, TriviaProfile::JS) {
                if trivia_ends_operand(source, i) {
                    operand_end = past;
                }
                i = past;
                continue;
            }
            match source[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i + 1) as u32);
                    }
                }
                // Regex literals are the one trivia kind the shared cursor leaves
                // significant (it needs previous-token context), so they are
                // skipped here. A regex whose body holds comment bytes — `/\//`,
                // `/[//]/`, `/\/*/` — would otherwise read as a comment from the
                // inside and swallow the rest of the line, losing the `)` this
                // scan is looking for and running its range on to some unrelated
                // paren. The scan reaches the literal's OPENING `/` first, whose
                // next byte is never `/` or `*`, so `skip_trivia` can't claim it.
                b'/' => {
                    if is_regex_start_after(source, operand_end, start as usize) {
                        i = skip_regex_literal(source, i, end);
                        operand_end = i;
                        continue;
                    }
                }
                _ => {}
            }
            operand_end = operand_end_after(source, i, operand_end);
            i += 1;
        }
        None
    }

    /// Position of the `)` that closes the `(` at `open` — the index OF the `)`,
    /// located with the depth-tracked, comment-aware scan over the rest of the
    /// source. Use when only the open-paren position is known and the close lies
    /// somewhere ahead; call `find_closing_paren` directly when a tighter search
    /// bound is available. Returns `None` if no matching `)` is found.
    pub(crate) fn matching_close_paren(&self, open: u32) -> Option<u32> {
        self.find_closing_paren(open, self.source.len() as u32)
            .map(|after| after - 1)
    }

    /// Find the end position of a keyword in source text.
    ///
    /// Searches backward from `end` for `keyword` as a whole word (not part of
    /// an identifier). Returns the byte position after the last character of the keyword,
    /// or `None` if not found.
    pub(crate) fn find_keyword_end(&self, keyword: &str, start: u32, end: u32) -> Option<u32> {
        // The LAST whole-word occurrence that is not inside a comment — so a
        // keyword buried in a comment (`from /* from */ 'x'`) isn't mistaken for
        // the real one (which dropped/relocated the comment), while a later real
        // keyword still wins over an earlier identifier containing it.
        tsv_lang::source_scan::rfind_keyword(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            keyword.as_bytes(),
            TriviaProfile::JS,
        )
        .map(|i| (i + keyword.len()) as u32)
    }

    /// Find a keyword between a start position and end boundary.
    ///
    /// Returns the position of the first character of the keyword if found.
    /// Skips over comments and strings. Checks for word boundaries (keyword
    /// must not be part of a larger identifier).
    pub(crate) fn find_keyword_in_range(&self, start: u32, end: u32, keyword: &str) -> Option<u32> {
        let source = self.source.as_bytes();
        let end = (end as usize).min(source.len());
        tsv_lang::source_scan::find_keyword(
            source,
            start as usize,
            end,
            keyword.as_bytes(),
            TriviaProfile::JS,
        )
        .map(|i| i as u32)
    }

    /// Find the position of the first non-whitespace, non-comment token after `start`.
    ///
    /// Skips spaces, tabs, newlines, line comments (`//`), and block comments (`/* */`).
    /// Used to find where the first modifier keyword or identifier begins after decorators.
    pub(crate) fn find_first_token_after(&self, start: u32) -> u32 {
        let bytes = self.source.as_bytes();
        let mut pos = start as usize;
        while pos < bytes.len() {
            match bytes[pos] {
                b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
                b'/' if bytes.get(pos + 1) == Some(&b'/') => {
                    // Skip line comment
                    pos += 2;
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                b'/' if bytes.get(pos + 1) == Some(&b'*') => {
                    // Skip block comment
                    pos += 2;
                    while pos + 1 < bytes.len() {
                        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                            pos += 2;
                            break;
                        }
                        pos += 1;
                    }
                }
                _ => return pos as u32,
            }
        }
        pos as u32
    }

    /// Find the source position of a keyword that follows decorators.
    ///
    /// Searches for `keyword` in the source text after the last decorator's end.
    /// Returns `fallback` if there are no decorators or the keyword isn't found.
    pub(crate) fn find_keyword_after_decorators(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        keyword: &str,
        fallback: u32,
    ) -> u32 {
        decorators
            .and_then(|decs| decs.last())
            .and_then(|last| {
                // Comment-aware + word-boundaried, so a keyword inside a comment
                // between the decorator and the declaration (`@dec /* class */
                // class C {}`) isn't matched (which would drop the comment).
                self.find_keyword_in_range(last.span.end, self.source.len() as u32, keyword)
            })
            .unwrap_or(fallback)
    }

    /// Emit a node's source span verbatim. Used to round-trip the source of a
    /// format-ignored node (statement, block statement, object/pattern
    /// property, class/enum/interface/type-literal member) instead of
    /// reformatting it.
    /// Trailing whitespace is trimmed: a node's significant tokens never end in
    /// whitespace, and prettier never preserves it — some spans (e.g. a
    /// `TSConstructSignatureDeclaration`'s) over-extend to the next line's start.
    fn raw_source_doc(&self, span: Span) -> DocId {
        self.raw_source_range(span.start, span.end)
    }

    /// Emit `[start, end)` of the source verbatim. Like `raw_source_doc` but for a
    /// format-ignored member whose verbatim slice must exclude a separator
    /// the surrounding loop emits itself (e.g. a type-literal member's `;`), so
    /// the terminator isn't duplicated.
    ///
    /// Emitted as a `verbatim_source_span` over the whitespace-trimmed sub-span —
    /// an ignored region can be large, and the verbatim slice needs no pool copy;
    /// the variant keeps the frozen slice's embedded newlines opaque to
    /// `will_break` (source layout, not a break the enclosing group must honor).
    fn raw_source_range(&self, start: u32, end: u32) -> DocId {
        let trimmed = self.source[start as usize..end as usize].trim_end();
        let span = Span {
            start,
            end: start + trimmed.len() as u32,
        };
        // The comments inside a format-ignored node ride out in the raw slice, never
        // reaching `build_comment_doc` — tell the ledger so they don't read as dropped.
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);

        self.d().verbatim_source_span(span, self.source)
    }

    /// Emit an identifier-name doc node — the doc-side name-emission seam.
    /// Span-identity names render as verbatim source (`DocText::SourceSpan`
    /// with deferred width — identifier names are newline-free, and the lazy
    /// measure matches the zero build-time cost); the rare escaped name copies
    /// its decoded `&str` into the doc text pool (`DocText::Pooled`).
    pub(in crate::printer) fn ident_name_doc(
        &self,
        name: internal::IdentName<'_>,
        name_start: u32,
    ) -> DocId {
        let d = self.d();
        match name.escaped {
            Some(s) => d.text_pooled(s),
            None => d.source_span_ident(Span::new(name_start, name_start + name.raw_len as u32)),
        }
    }

    /// [`Self::ident_name_doc`] for an `Identifier` node (the name is the
    /// leading token of the node span).
    pub(in crate::printer) fn identifier_name_doc(&self, id: &internal::Identifier<'_>) -> DocId {
        self.ident_name_doc(id.ident_name(), id.span.start)
    }

    /// Run `f` over a name channel resolved at `name_start` — the compare/width
    /// seam. Span-identity names borrow the source slice; an escaped name compares its
    /// arena string (so an escaped name still compares decoded).
    pub(in crate::printer) fn with_ident_name_at<R>(
        &self,
        name: internal::IdentName<'_>,
        name_start: u32,
        f: impl FnOnce(&str) -> R,
    ) -> R {
        match name.escaped {
            Some(s) => f(s),
            None => {
                f(&self.source[name_start as usize..name_start as usize + name.raw_len as usize])
            }
        }
    }

    /// [`Self::with_ident_name_at`] for an `Identifier` node.
    pub(in crate::printer) fn with_ident_name<R>(
        &self,
        id: &internal::Identifier<'_>,
        f: impl FnOnce(&str) -> R,
    ) -> R {
        self.with_ident_name_at(id.ident_name(), id.span.start, f)
    }

    /// Run `f` with [`Self::expr_stmt_paren_target`] set to `target`, restoring whatever
    /// was there before.
    ///
    /// Restore, not clear: a statement-head position can nest inside another one
    /// (a `for` header inside an expression statement whose own target is still live),
    /// so clearing would strand the outer wrap. Scoped rather than open-coded because the
    /// callers have early returns above them, and a restore skipped by one leaks a paren
    /// onto the next sibling — a shape the fixtures don't reach.
    pub(in crate::printer) fn with_expr_stmt_paren_target<R>(
        &self,
        target: Option<Span>,
        f: impl FnOnce() -> R,
    ) -> R {
        let saved = self.expr_stmt_paren_target.replace(target);
        let result = f();
        self.expr_stmt_paren_target.set(saved);
        result
    }
}
