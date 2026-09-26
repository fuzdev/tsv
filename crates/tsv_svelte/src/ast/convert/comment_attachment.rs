// Comment-attach support for the Svelte wire's acorn islands.
//
// Svelte attaches an island's comments with acorn's leading/trailing DFS
// (`add_comments` in svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js).
// tsv runs that attach **online**, off the wire writer's own node opens and
// closes — `tsv_ts`'s `CommentAttach` is the state machine, and this module is
// what an island hands it (`tsv_ts`'s `IslandComments`): the comment WINDOW —
// which of the document's comments that island's canonical parse would have
// collected — plus what acorn's walk reads about a ROOT, which the emit stack
// cannot supply because a root has no frame below it.
//
// One `attach_*` builder per island kind, over the window primitives below.
// There is no second pass and no per-node map: the node that opens is the node
// acorn's walk would have entered, in the order the wire emits it. See
// `tsv_ts`'s `ast/convert/write/comments.rs`.

use crate::ast::internal;
use crate::whitespace::svelte_ws_width_at;

use tsv_lang::{AcornPrefix, Comment, Span, source_scan::skip_comment};
use tsv_ts::ast::convert::{CommentAttach, CommentMode, IslandComments};

/// The inputs every island's comment-attach builder (the `attach_*` fns below)
/// shares: the document's template comments and its source.
///
/// (`attach_script` takes the same inputs but ignores `template_comments`: it
/// queues the script's *own* comments, pairing them off the shared resolver.)
#[derive(Clone, Copy)]
pub(super) struct AttachInputs<'a> {
    pub(super) template_comments: &'a [&'a Comment],
    pub(super) source: &'a str,
    /// Every embedded acorn parse, resolved to the source Svelte handed it — what each
    /// comment's wire `value` is dedented by. The same resolver the root `comments` array
    /// reads, so an attached copy and the root entry for one comment cannot disagree.
    pub(super) acorn_prefixes: internal::AcornPrefixes<'a>,
}

impl<'a> AttachInputs<'a> {
    /// The island's queue: the template comments inside `[start, end)`, each paired with the
    /// source Svelte handed acorn for it. The shape every window-based builder below wants.
    fn window_queue(self, start: u32, end: u32) -> Vec<(&'a Comment, AcornPrefix)> {
        self.acorn_prefixes
            .pair_with(window_queue(self.template_comments, start, end))
    }
}

/// The attach for a comment-bearing island whose canonical parse is ONE acorn
/// parse ending in a trailing-comment scan — the shape shared by every island
/// below, which is why one builder serves them all:
///
/// - a template expression (`{expr}`, block test, directive expression,
///   `{@debug}` id, spread, `<svelte:element>` tag / `<svelte:component>`
///   expression, snippet name), via `write_generic_island` / `write_snippet_name`
/// - a `{@const}`'s **init**, through [`attach_const_tag_init`], which supplies
///   the binding's end as the window start
/// - a `{const …}` / `{let …}` **declaration tag**, whose canonical parse is
///   `parse_statement_at` rather than `parse_expression_at`. Only the parse
///   ENTRY POINT differs; `add_comments` is the same call on the same shape, so
///   the whole `VariableDeclaration` tree (every declarator and its id/init)
///   attaches through this one window. Hence "expression" in the name is the
///   common case, not the contract.
///
/// The window runs from the container's start (canonical filters each parse's
/// comments to `start >= index`, where `index` is where *that* parse began) to
/// the end of the trailing-comment run acorn scans past after the parsed region.
/// Leftovers trail the root, as acorn's own post-walk special case does.
///
/// ⚠️ **`parse_end` is where the PARSE ended, not where the emitted root node
/// ends** — the two differ wherever tsv discards a wrapper acorn kept. A JSDoc
/// cast is the live case: `parse_expression_at` runs with `preserveParens: true`,
/// so acorn's root for `/** @type {T} */ (x)` is a `ParenthesizedExpression`
/// ending after the `)`, and its own post-expression token scan starts there.
/// Anchoring the scan at the emitted root (the *inner* expression, which stops
/// short of the `)`) kills it on that `)` instead, so every comment past it is
/// filtered out before the walk ever runs — canonical attaches it to the inner
/// node (the `)` is in acorn's own `/^[,) \t]*$/` trailing gap class) and tsv
/// had nowhere to put it. `internal::JsdocCast`'s span covers the parens, which
/// is what makes the caller's expression the right anchor.
///
/// A **bare** grouping paren is the residual: tsv discards those at parse time and
/// keeps no span, so `{(x) /* c */}` still loses the attachment. It is not
/// format-stable (tsv and prettier both print `{x /* c */}`), so the shape cannot
/// be fixtured — see [conformance_svelte.md](../../../../../docs/conformance_svelte.md)
/// §Comment Attachment Differences.
pub(super) fn attach_expression<'a>(
    attach: AttachInputs<'a>,
    container_start: u32,
    parse_end: u32,
    range_end: u32,
) -> CommentAttach<'a> {
    let window_end = scan_past_trailing_comments(attach.source, parse_end, range_end);
    CommentAttach::new(
        attach.source,
        IslandComments {
            queue: attach.window_queue(container_start, window_end),
            root_parent_end: None,
            root_fallback: true,
            html_leading: None,
        },
    )
}

/// The attaches for a comment-bearing **block binding pattern** — the `{#each … as ctx}`
/// context, the `{:then value}` / `{:catch error}` bindings, and the `{@const}` id, which
/// takes it directly (see [`attach_const_tag_init`] for that split).
///
/// A typed binding is **two** acorn parses, so it is two islands: canonical parses a
/// destructure as a synthetic `(pattern = 1)` expression (`read_pattern`) and its trailing
/// `: T` with a second, separately padded one (`read_type_annotation`'s `_ as ` trick), and
/// `add_comments` runs once per parse over that parse's own comments. A comment inside the
/// pattern therefore attaches within the pattern subtree or nowhere — never to the
/// annotation — and one inside the annotation within the annotation or nowhere. One attach
/// over both let a comment the pattern's walk leaves unclaimed (an own-line one before the
/// closing bracket, outside acorn's single-trailing gap class) lead the `TSTypeAnnotation`
/// that opens after it, and let one the annotation's walk leaves unclaimed fall back to
/// trail the pattern root, whose span stops at the bare pattern.
///
/// Each window is its parse's own. The pattern's starts at the binding, never at the
/// enclosing head: canonical filters each parse's comments to `start >= index`, where
/// `index` is where *that* parse began, so a `{#each}` key's own parse (which begins at its
/// `(`) must not see a comment written back in the pattern. The annotation's is its span,
/// which starts where the binding ends — the gap before the `:` included, as Svelte builds
/// the node.
///
/// Both windows are fully known up front, so no trailing scan, and neither runs acorn's
/// root fallback: each parse's root is a wrapper the wire discards (the `= 1` assignment,
/// the `_ as` expression), and a comment inside a window starts before that wrapper ends.
pub(super) fn attach_binding_pattern<'a>(
    pattern: &tsv_ts::ast::internal::Expression<'_>,
    attach: AttachInputs<'a>,
) -> BindingAttach<'a> {
    let island = |span: Span| {
        CommentAttach::new(
            attach.source,
            IslandComments {
                queue: attach.window_queue(span.start, span.end),
                root_parent_end: None,
                root_fallback: false,
                html_leading: None,
            },
        )
    };
    BindingAttach {
        pattern: island(pattern.span()),
        annotation: tsv_ts::pattern_type_annotation(pattern).map(|ann| island(ann.span)),
    }
}

/// A block binding's two islands (see [`attach_binding_pattern`]): the pattern's, and the
/// annotation's when the binding carries a `: T`.
pub(super) struct BindingAttach<'a> {
    pattern: CommentAttach<'a>,
    annotation: Option<CommentAttach<'a>>,
}

impl BindingAttach<'_> {
    /// The pattern's comment mode, then the annotation's (`Off` without one).
    pub(super) fn modes(&self) -> (CommentMode<'_>, CommentMode<'_>) {
        (
            self.pattern.mode(),
            self.annotation
                .as_ref()
                .map_or(CommentMode::Off, CommentAttach::mode),
        )
    }
}

/// The region a binding pattern's comments come from — its own start through the
/// end of its annotation, if it has one: the union of [`attach_binding_pattern`]'s two
/// windows, which meet at the annotation's start.
///
/// The writer's cheap "is there anything here at all" pre-check, which must CONTAIN both
/// attach windows: a pre-check narrower than either silently DROPS a comment (the attach
/// is never built, and nothing else emits it); a wider one only wastes a build. It is
/// also where a `{@const}`'s init window begins ([`attach_const_tag_init`]).
pub(super) fn pattern_comment_window(pattern: &tsv_ts::ast::internal::Expression<'_>) -> Span {
    Span::new(pattern.span().start, tsv_ts::pattern_binding_end(pattern))
}

/// The `{@const id = init}` INIT attach — the tag's last window.
///
/// Canonical Svelte runs an acorn parse per island, each with its own comment attach — the
/// id (two islands with a `: T` annotation, the pattern then the type; see
/// [`attach_binding_pattern`]) and the init: `read_pattern` parses a destructure id as a
/// synthetic `(pattern = 1)` expression (so an id-internal comment attaches inside the
/// pattern subtree — e.g. a destructure default's literal), and `read_expression` parses
/// the init (comments from after the id through the tag close attach in the init subtree).
/// Comments *between* the pattern and the `=` are a canonical parse error, so the id and
/// init windows partition the tag. The `VariableDeclaration`/`VariableDeclarator` envelope
/// carries no comments and is reproduced at emit time.
///
/// The id windows are the shared binding-pattern ones ([`attach_binding_pattern`]) — the
/// same windows the `{#each}` / `{:then}` / `{:catch}` bindings take, which is what makes
/// the id and init windows split at the end of the BINDING rather than of its bare name.
pub(super) fn attach_const_tag_init<'a>(
    tag: &internal::ConstTag<'_>,
    attach: AttachInputs<'a>,
) -> CommentAttach<'a> {
    let binding_end = pattern_comment_window(tag.id).end;
    attach_expression(attach, binding_end, tag.init.span().end, tag.span.end)
}

/// The attach for a comment-bearing expression LIST that canonical Svelte parses
/// in ONE acorn parse — `{#snippet}` parameters (a function-parameter context)
/// and multi-identifier `{@debug}` (a `SequenceExpression`). One shared queue
/// walked through each item in turn, so an inter-item comment lands exactly
/// where acorn's single-parse walk puts it: a same-line `[,) \t]*` gap trails the
/// *preceding* item, anything else leads the *following* one.
///
/// `wrapper` is the discarded parse wrapper's own span — `{@debug}`'s
/// `SequenceExpression`, which spans first identifier to last; `None` for
/// snippet params, whose function wrapper encloses the whole list. Everything
/// the wrapper would have claimed dies with it, at both ends: its `end` drives
/// acorn's `node.end == parent.end` trailing suppression (so `{@debug}`'s last
/// identifier never claims a trailing comment), and its `start` bounds the
/// queue, so the leading run *before* the list — which acorn hands to the
/// wrapper, the outermost node opening after it — reaches no identifier. Both
/// leftovers stay unattached (no root fallback) and still emit in the root
/// `comments` array. (A single-identifier `{@debug}` has no wrapper — the
/// identifier is the parse root itself — so it takes [`attach_expression`] with
/// its root-fallback trailing, and its leading run does attach.)
pub(super) fn attach_expression_list<'a>(
    attach: AttachInputs<'a>,
    container_start: u32,
    range_end: u32,
    wrapper: Option<Span>,
) -> CommentAttach<'a> {
    let queue_start = wrapper.map_or(container_start, |w| w.start);
    CommentAttach::new(
        attach.source,
        IslandComments {
            queue: attach.window_queue(queue_start, range_end),
            root_parent_end: wrapper.map(|w| w.end),
            root_fallback: false,
            html_leading: None,
        },
    )
}

/// The attach for a comment-bearing (or preceding-HTML) `<script>` `Program`.
///
/// The queue is the script's own comments — the whole set, since acorn parsed
/// the whole body — and the preceding HTML comment rides along as the
/// `Program`'s first `leadingComments` entry (Svelte's positionless
/// `{type: "Line", value}` shape). The `options: null` non-TS quirk is
/// reproduced at emit time (schema-driven), so it never perturbs the walk.
pub(super) fn attach_script<'a>(
    script: &internal::Script<'a>,
    attach: AttachInputs<'a>,
    html_leading_comment: Option<&internal::HtmlComment>,
) -> CommentAttach<'a> {
    // The queue is the script's own comments rather than the template's, but the dedent
    // lookup reads only their positions — so `read_script`'s blanked prefix comes from the
    // one region table here too, rather than being restated from `script.content.span`.
    CommentAttach::new(
        attach.source,
        IslandComments {
            queue: attach
                .acorn_prefixes
                .pair_with(script.content.comments.iter().collect()),
            root_parent_end: None,
            root_fallback: true,
            html_leading: html_leading_comment.map(|c| c.content(attach.source)),
        },
    )
}

/// Whether a comment lies outside every `<script>` content span — i.e., it's a
/// template expression comment that the attach passes may move into the JSON tree.
pub(super) fn is_template_comment(comment: &Comment, script_spans: &[(u32, u32)]) -> bool {
    !script_spans
        .iter()
        .any(|&(s, e)| comment.span.start >= s && comment.span.end <= e)
}

/// The template comments inside an attach window `[start, end]`, as the
/// position-ordered queue the attach consumes.
///
/// The window is settled here, before the attach is built — `end` is already the
/// end of the parse's trailing-comment run where one is scanned for
/// ([`scan_past_trailing_comments`]), and the container bound where the island's
/// own bounds settle it outright (a binding pattern, an expression list).
///
/// `template_comments` is sorted ascending by `span.start` — the same fact the
/// writer's `Ctx::any_comment_in` pre-check binary-searches on — so the window is
/// a contiguous run: seek its first member, then walk while `span.start` stays
/// inside. The `span.end` bound still filters per candidate, because a comment
/// may start inside the window and run past its end.
fn window_queue<'a>(template_comments: &[&'a Comment], start: u32, end: u32) -> Vec<&'a Comment> {
    let first = template_comments.partition_point(|c| c.span.start < start);
    template_comments[first..]
        .iter()
        .copied()
        .take_while(|c| c.span.start <= end)
        .filter(|c| c.span.end <= end)
        .collect()
}

/// Scan source after an expression's end to find the effective end of comment collection.
///
/// Acorn's token scanner reads past whitespace and comments when looking for the next token.
/// This function mimics that: starting at `pos`, skip whitespace and block/line comments,
/// and return the position after the last skipped comment. If no comments are found, returns `pos`.
///
/// Called once per island by [`attach_expression`],
/// which settles the whole comment window before building the attach. It starts where the
/// **parse** ended, which is not the emitted root node's end wherever tsv discards a
/// wrapper acorn kept — a JSDoc cast emits its inner expression, whose end stops short of
/// the cast's closing paren, and anchoring here would kill the scan on that `)`.
///
/// `skip_comment` is passed `bytes.len()` (not `limit`) as its bound, and its
/// past-`end` return on an unterminated block comment is unreachable here:
/// this runs only after a successful parse, and every comment in the scanned
/// window was already lexed as terminated. Expression tags track comments in
/// their closing-brace scan (unterminated → no `}` found → parse error);
/// block tags hand their content to the TS parser, whose one-token lookahead
/// lexes all trivia after the expression and hard-errors on an unterminated
/// block comment. This scanner's trivia set is the lexer's own
/// ([`is_svelte_ws`](crate::whitespace::is_svelte_ws) + JS comments) rather than a proper
/// subset of it, so it can never walk past that validated region either.
///
/// ⚠️ The whitespace class is [`is_svelte_ws`](crate::whitespace::is_svelte_ws) — acorn's,
/// which this mimics — and NOT an
/// ASCII `b' ' | b'\t' | b'\r' | b'\n'` byte match. With that class a non-ASCII JS `\s`
/// between an expression and its trailing comment (`{expr<NBSP>/* c */}`) ends the scan
/// early, so the comment loses the `trailingComments` attachment canonical emits — and the
/// root `comments` array loses it too. Stepping by the character's WIDTH is what keeps the
/// non-ASCII arm on a character boundary.
fn scan_past_trailing_comments(source: &str, start: u32, limit: u32) -> u32 {
    let bytes = source.as_bytes();
    let mut pos = start as usize;
    let limit = (limit as usize).min(bytes.len());
    let mut last_comment_end = start;

    while pos < limit {
        // The crate's own scanning form of `is_svelte_ws` — it dispatches on the raw byte
        // and pays a decode only on the non-ASCII branch, as the lexer's cursor does, and
        // hands back the WIDTH so the step lands on a character boundary.
        if let Some(width) = svelte_ws_width_at(source, pos) {
            pos += width;
            continue;
        }
        match skip_comment(bytes, pos, bytes.len()) {
            Some(next) => {
                pos = next;
                last_comment_end = pos as u32;
            }
            // Non-whitespace, non-comment — stop scanning
            None => break,
        }
    }

    last_comment_end
}

#[cfg(all(test, feature = "convert"))]
mod tests {
    use serde_json::Value;

    /// Parse a `<script lang="ts">` body and return the public JSON AST.
    fn convert_ts(body: &str) -> Value {
        let source = format!("<script lang=\"ts\">\n{body}\n</script>");
        // Test inputs are hardcoded valid sources; a parse failure should panic
        let arena = bumpalo::Bump::new();
        #[expect(clippy::expect_used)]
        let root = crate::parse(&source, &arena).expect("parse");
        #[expect(clippy::expect_used)]
        serde_json::from_slice(&crate::convert_ast_json_bytes(&root, &source)).expect("wire")
    }

    /// The first statement's expression in the instance `<script>`.
    fn first_expression(ast: &Value) -> &Value {
        &ast["instance"]["content"]["body"][0]["expression"]
    }

    /// The single leading comment value on a node, if any.
    fn leading_comment(node: &Value) -> Option<&str> {
        node.get("leadingComments")?
            .as_array()?
            .first()?
            .get("value")?
            .as_str()
    }

    /// Parse a whole component and return the public JSON AST.
    fn convert_component(source: &str) -> Value {
        let arena = bumpalo::Bump::new();
        #[expect(clippy::expect_used)]
        let root = crate::parse(source, &arena).expect("parse");
        #[expect(clippy::expect_used)]
        serde_json::from_slice(&crate::convert_ast_json_bytes(&root, source)).expect("wire")
    }

    /// `scan_past_trailing_comments` mimics acorn's post-expression token scan, so its
    /// whitespace class has to be acorn's — JS `\s`, not the ASCII bytes it once matched.
    ///
    /// Not a fixture: the format side of an expression-tag comment is governed by tsv's
    /// (deliberate) comment-PRESERVATION divergence — prettier deletes the comment outright
    /// — so no format claim can carry this, exactly as with the lone-`<CR>` half of the
    /// `loc` model. Every expectation below is transcribed from `canonical_parse`.
    ///
    /// One case per class boundary: an ASCII gap (the control) and the four non-ASCII
    /// members the byte match missed. U+0085 NEL is the NULL CONTROL and is asserted
    /// separately below — it is Unicode `White_Space` but NOT JS `\s`, so widening to
    /// Rust's class instead of Svelte's would have swept it in.
    #[test]
    fn expression_tag_trailing_comment_attaches_across_any_js_whitespace() {
        for (label, gap) in [
            ("space", " "),
            ("nbsp U+00A0", "\u{a0}"),
            ("zwnbsp U+FEFF", "\u{feff}"),
            ("ideographic U+3000", "\u{3000}"),
            ("line separator U+2028", "\u{2028}"),
        ] {
            let ast = convert_component(&format!("<div>{{expr{gap}/* c */}}</div>"));
            let expression = &ast["fragment"]["nodes"][0]["fragment"]["nodes"][0]["expression"];
            assert_eq!(expression["name"], "expr", "{label}: expression tag shape");
            let trailing = expression
                .get("trailingComments")
                .and_then(|c| c.as_array())
                .and_then(|c| c.first())
                .and_then(|c| c.get("value"))
                .and_then(|v| v.as_str());
            assert_eq!(
                trailing,
                Some(" c "),
                "{label}: trailingComments attachment"
            );
            let root_comments = ast["comments"].as_array().map_or(0, Vec::len);
            assert_eq!(root_comments, 1, "{label}: root `comments` array");
        }
    }

    /// The null control for the test above: U+0085 NEL is Unicode `White_Space` but not JS
    /// `\s`, so it is not a gap acorn crosses — it is not valid there at all, and BOTH
    /// parsers reject the document (verified against `canonical_parse`). A fix that reached
    /// for `char::is_whitespace` rather than Svelte's own class would be indistinguishable
    /// from the correct one without this case.
    #[test]
    fn expression_tag_nel_gap_is_rejected_not_crossed() {
        let arena = bumpalo::Bump::new();
        let source = "<div>{expr\u{85}/* c */}</div>";
        assert!(
            crate::parse(source, &arena).is_err(),
            "U+0085 is not JS `\\s`, so it cannot open a gap acorn scans across"
        );
    }

    // For `new Foo< // c\n A, B>(x)`, acorn (`parseNew` sets `callee`,
    // `typeArguments`, then `arguments`) walks the type arguments before the call
    // arguments, so the `<`-trailing line comment attaches as a leadingComment of
    // the FIRST type argument — never the call argument.
    #[test]
    fn new_expression_type_arg_open_angle_comment_attaches_to_first_type_arg() {
        let ast = convert_ts("new Foo< // c\n\tA,\n\tB\n>(x);");
        let expr = first_expression(&ast);
        assert_eq!(expr["type"], "NewExpression");

        let first_type_arg = &expr["typeArguments"]["params"][0];
        assert_eq!(first_type_arg["typeName"]["name"], "A");
        assert_eq!(
            leading_comment(first_type_arg),
            Some(" c"),
            "comment trailing `<` should attach to the first type argument"
        );
        assert_eq!(
            leading_comment(&expr["arguments"][0]),
            None,
            "comment must not land on the call argument for a `new` expression"
        );
    }

    // Sibling parity (already correct): for a CALL expression, acorn (`parseSubscript`
    // sets `callee`, `arguments`, then `typeArguments`) walks the call arguments first,
    // so the same comment attaches to the call ARGUMENT, not the type argument.
    #[test]
    fn call_expression_type_arg_open_angle_comment_attaches_to_call_arg() {
        let ast = convert_ts("foo< // c\n\tA,\n\tB\n>(x);");
        let expr = first_expression(&ast);
        assert_eq!(expr["type"], "CallExpression");

        assert_eq!(leading_comment(&expr["arguments"][0]), Some(" c"));
        assert_eq!(leading_comment(&expr["typeArguments"]["params"][0]), None);
    }

    // For a class method `m< // c\n T>(p) {}`, acorn-typescript sets the
    // MethodDefinition's `key`, then `typeParameters`, then `value`, so the
    // `<`-trailing line comment walks onto the first type PARAMETER — not the
    // method's `value` FunctionExpression (whose span begins after the comment).
    #[test]
    fn class_method_type_param_open_angle_comment_attaches_to_first_type_param() {
        let ast = convert_ts("class C {\n\tm< // c\n\t\tT\n\t>(p: T) {}\n}");
        let method = &ast["instance"]["content"]["body"][0]["body"]["body"][0];
        assert_eq!(method["type"], "MethodDefinition");

        let first_type_param = &method["typeParameters"]["params"][0];
        assert_eq!(first_type_param["name"], "T");
        assert_eq!(
            leading_comment(first_type_param),
            Some(" c"),
            "comment trailing the method type-param `<` should attach to the first type parameter"
        );
        assert_eq!(
            leading_comment(&method["value"]),
            None,
            "comment must not land on the method's FunctionExpression value"
        );
    }

    // Sibling parity (already correct): an interface method is a TSMethodSignature
    // whose `typeParameters` already precede the rest, so the same comment attaches
    // to the first type parameter — confirming the class-method gap is localized to
    // MethodDefinition's child-walk order, not the type-parameter path itself.
    #[test]
    fn interface_method_type_param_open_angle_comment_attaches_to_first_type_param() {
        let ast = convert_ts("interface I {\n\tm< // c\n\t\tT\n\t>(p: T): void;\n}");
        let sig = &ast["instance"]["content"]["body"][0]["body"]["body"][0];
        assert_eq!(sig["type"], "TSMethodSignature");
        assert_eq!(
            leading_comment(&sig["typeParameters"]["params"][0]),
            Some(" c")
        );
    }

    /// The online attach's **subtree skip** at its boundaries.
    ///
    /// `tsv_ts`'s `CommentAttach` skips a subtree when nothing in it can take the queue's
    /// front comment (its module doc states the test and the proof): the queue is empty,
    /// or the front comment starts past the subtree's end *and* past the run of `,` `)`
    /// space tab bytes in front of it, and a last-in-body subtree's parent ends before
    /// it. Each case sits on one edge of that test, where a gate one byte too eager would
    /// move a comment — a descendant ending inside the gap-class run, a last-in-body child
    /// whose parent is still open, the drained queue, the preceding-HTML entry with
    /// nothing queued.
    ///
    /// Every expected placement is transcribed from `canonical_parse`. The other half of
    /// the guard runs underneath: a debug build (which `cargo test` is) asserts at every
    /// skipped node that acorn's rules would have claimed nothing there.
    mod subtree_skip {
        use serde_json::Value;

        const LEADING: &str = "leadingComments";
        const TRAILING: &str = "trailingComments";

        /// One attached comment: the carrying node's `type` / `start` / `end`, the list
        /// it sits in, and the comment's `value`.
        type Placement = (String, u64, u64, &'static str, String);

        /// Every `leadingComments` / `trailingComments` entry in the wire, in document
        /// order.
        #[expect(clippy::expect_used)]
        fn placements(v: &Value, out: &mut Vec<Placement>) {
            match v {
                Value::Object(map) => {
                    for key in [LEADING, TRAILING] {
                        let comments = map.get(key).and_then(Value::as_array);
                        for comment in comments.into_iter().flatten() {
                            out.push((
                                map["type"].as_str().expect("node type").to_owned(),
                                map["start"].as_u64().expect("node start"),
                                map["end"].as_u64().expect("node end"),
                                key,
                                comment["value"].as_str().expect("value").to_owned(),
                            ));
                        }
                    }
                    for (key, child) in map {
                        if key != LEADING && key != TRAILING {
                            placements(child, out);
                        }
                    }
                }
                Value::Array(items) => items.iter().for_each(|item| placements(item, out)),
                _ => {}
            }
        }

        /// Assert `source`'s placements on both wires — the attach knows nothing about
        /// line and column, so the `no-locations` variant must place every comment
        /// identically.
        #[expect(clippy::expect_used)]
        fn assert_placements(source: &str, expected: &[(&str, u64, u64, &'static str, &str)]) {
            let arena = bumpalo::Bump::new();
            let root = crate::parse(source, &arena).expect("parse");
            let expected: Vec<Placement> = expected
                .iter()
                .map(|&(node, start, end, key, value)| {
                    (node.to_owned(), start, end, key, value.to_owned())
                })
                .collect();
            for (wire_name, bytes) in [
                ("default", crate::convert_ast_json_bytes(&root, source)),
                (
                    "no-locations",
                    crate::convert_ast_json_bytes_no_locations(&root, source),
                ),
            ] {
                let wire: Value = serde_json::from_slice(&bytes).expect("wire");
                let mut found = Vec::new();
                placements(&wire, &mut found);
                assert_eq!(found, expected, "{wire_name} wire: {source:?}");
            }
        }

        #[test]
        fn a_descendant_ending_inside_the_gap_class_run_takes_the_comment() {
            // `x` ends where the `), ` run before the comment begins, so its gap to the
            // comment is all class bytes and it claims, two levels below the argument
            // `g(x)`. A test that asked only whether the comment starts past the argument
            // would skip `g(x)` and lose the claim.
            assert_placements(
                "{f(g(x), /* c */ y)}",
                &[("Identifier", 5, 6, TRAILING, " c ")],
            );
            // The run begins exactly at the node's end.
            assert_placements("{[a, /* c */ b]}", &[("Identifier", 2, 3, TRAILING, " c ")]);
            // A tab is in the class too.
            assert_placements(
                "{f(a,\t/* c */ b)}",
                &[("Identifier", 3, 4, TRAILING, " c ")],
            );
        }

        #[test]
        fn a_gap_outside_the_class_lets_the_comment_lead_the_next_node() {
            // The `+` stops the run, so `f(a)` and everything in it is skipped and the
            // comment leads `b`.
            assert_placements(
                "{f(a) + /* c */ b}",
                &[("Identifier", 16, 17, LEADING, " c ")],
            );
        }

        #[test]
        fn a_last_in_body_child_takes_a_comment_before_its_parent_closes() {
            // The comment is on its own line, so no single-trailing claim reaches it — but
            // `a();` is its block's last statement, and that run stops only at the `}`.
            assert_placements(
                "<script>\n\tfunction f() {\n\t\ta();\n\t\t// c\n\t}\n</script>\n",
                &[("ExpressionStatement", 27, 31, TRAILING, " c")],
            );
            // Once the block has closed, the whole function is skipped and the comment
            // leads the next statement.
            assert_placements(
                "<script>\n\tfunction f() {\n\t\ta();\n\t}\n\t// c\n\tb();\n</script>\n",
                &[("ExpressionStatement", 42, 46, LEADING, " c")],
            );
        }

        #[test]
        fn nested_last_in_body_children_each_see_their_own_parent() {
            // The inner object's last property claims; the outer property holding it is
            // itself last-in-body and contains the comment, so it is never a skip candidate.
            assert_placements(
                "<script>\n\tconst o = {\n\t\ta: {\n\t\t\tb: 1\n\t\t\t// c\n\t\t}\n\t};\n</script>\n",
                &[("Property", 32, 36, TRAILING, " c")],
            );
            // The script's last statement claims the comment below it; every last-in-body
            // container inside it (the object's `a`, the array's `{ b: 2 }`) is bounded by
            // a parent that closes before the comment, so the declarator's whole subtree
            // is skipped.
            assert_placements(
                "<script>\n\tconst o = { a: [1, { b: 2 }] }\n\t/* c */\n</script>\n",
                &[("VariableDeclaration", 10, 40, TRAILING, " c ")],
            );
        }

        #[test]
        fn a_drained_queue_skips_everything_after_it() {
            assert_placements(
                "<script>\n\t// c\n\tconst a = { b: [1, 2] };\n\tfoo(a);\n</script>\n",
                &[("VariableDeclaration", 16, 40, LEADING, " c")],
            );
        }

        #[test]
        fn a_preceding_html_comment_with_an_empty_queue_still_reaches_the_program() {
            // The island is built for the HTML comment alone, so its queue is empty from
            // the first open: every node below the root is skipped, and the root still
            // emits the entry.
            assert_placements(
                "<!-- note -->\n<script>\n\tconst a = { b: 1 };\n</script>\n",
                &[("Program", 22, 44, LEADING, " note ")],
            );
        }
    }
}
