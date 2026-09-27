// Nested `<script>` / `<style>` glue analysis, computed once per fragment.
//
// A nested `<script>` / `<style>` is an ordinary element — the compiler does not hoist it — and it
// renders no box (`display: none`). The printer gives one with a body a line of its own, which is
// render-free only where a line break beside it disappears: where it merges into whitespace that
// already renders, or where it sits at the edge of a line box. Everywhere else the break renders a
// space, and the element keeps its glue — laid out as a glued inline element whose body breaks.
//
// The answer depends on the whole run of render-invisible nodes around the element and on
// whether the fragment's own edges are line-box edges, which depends on the parent. So the
// fragment EDGES are indexed once per document, top down ([`Printer::index_fragment_edges`]),
// and each node slice a reader holds gets its flags computed once, in one forward and one
// backward pass ([`FragmentGlue`]), then read by position.

use super::helpers::is_control_flow_block;
use crate::ast::internal::{ElementKind, FragmentNode, Root};
use crate::printer::Printer;
use std::rc::Rc;

/// Whether a line break at each content edge of a fragment disappears — the edge of a line box,
/// so whitespace there is not rendered whatever stands beside it.
///
/// Only a **block** parent gives both edges that answer: the content of a block box starts and
/// ends lines. An inline element's or a component's edge is not one — the line goes on past the
/// parent, so `text1<span>text2<script>…</script></span>text3` renders `text1text2text3`, and a
/// break after `text2` renders a space. A block tag's body answers from where the tag sits in its
/// own fragment (the whitespace or edge outside it); an `{#each}` body repeats against itself and
/// a `{#snippet}` body renders wherever it is rendered, so neither has an edge that disappears.
/// The root does not either: a component can be rendered inside a line.
///
/// Nor do `<pre>` / `<textarea>` (their content is whitespace-sensitive, so no trim is licensed
/// there at all) or the elements the formatter treats as inline though the UA stylesheet gives
/// them a box of their own — table cells, `<button>`: the set is the formatter's block set, not
/// the render model's. Every such miss answers "not a line-box edge", which only ever keeps a
/// glue that a break would not have changed: the layout errs toward the glued inline form,
/// never toward a break that renders.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct EdgeFree {
    pub(crate) start: bool,
    pub(crate) end: bool,
}

/// One fragment's entry in the document's edge index ([`Printer::index_fragment_edges`]),
/// keyed by the address of its first node.
#[derive(Clone, Copy)]
pub(crate) struct IndexedFragment {
    /// The fragment's node count — the extent a run of it must fall inside.
    len: usize,
    /// How many whitespace-only nodes open and close it: what a boundary-trimmed run of it
    /// drops at each end.
    ws_ends: (usize, usize),
    edges: EdgeFree,
}

/// Glue flags of one node slice, indexed like its nodes.
pub(crate) struct FragmentGlue {
    /// A nested `<script>` / `<style>` with a body that keeps its glue — a line break on at least
    /// one side of it would render ([`Printer::is_glued_raw_text_element`]).
    inline_laid: Box<[bool]>,
    /// [`Printer::glued_to_content`] answered backward and forward — the declaration and global
    /// element glue scan.
    glued_before: Box<[bool]>,
    glued_after: Box<[bool]>,
    /// Whether whitespace that reaches gap `g` (before node `g`) from INSIDE a block tag there
    /// disappears looking backward / forward — the edges of that tag's bodies. The compiler's
    /// trim at this fragment's own edge does not reach it (it trims this fragment's first and
    /// last node, not a body's), so these are the scans past a kept node.
    edge_back: Box<[bool]>,
    edge_fwd: Box<[bool]>,
}

/// The render class of a node for the break-freeness scans.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Removed before the compiler's whitespace rules run: a comment, a hoisted node.
    Transparent,
    /// Whitespace-only text.
    Space,
    /// A nested `<script>` / `<style>` with a body: renders nothing, but is not removed, so the
    /// compiler's edge trim stops at it.
    Boxless,
    /// A block box: whitespace beside it is not rendered.
    Block,
    /// Content text, whose facing edge carries (`true`) or lacks whitespace.
    Text { before_ws: bool, after_ws: bool },
    /// Anything else that renders (or may) inline.
    Content,
}

/// Scan state: bit 0 — whitespace seen; bit 1 — a boxless element seen.
const PENDING: usize = 1;
const SEEN: usize = 2;

impl FragmentGlue {
    /// One scan step past a node of class `c`, given the answer `prev[s]` one node further out
    /// for every state `s`. `facing_ws` is the text's edge that faces the scan's origin.
    fn step(c: Class, prev: [bool; 4], facing_ws: impl Fn(Class) -> bool) -> [bool; 4] {
        let mut out = [false; 4];
        for (s, slot) in out.iter_mut().enumerate() {
            let pending = s & PENDING != 0;
            *slot = match c {
                Class::Transparent => prev[s],
                Class::Space => prev[s | PENDING],
                // Whitespace between the origin and a boxless element is interior (the element
                // is not removed), so it renders and the break merges into it.
                Class::Boxless => pending || prev[s | SEEN],
                Class::Block => true,
                Class::Text { .. } => pending || facing_ws(c),
                Class::Content => pending,
            };
        }
        out
    }

    fn edge(free: bool) -> [bool; 4] {
        // Reaching the edge with no boxless element on the way, the inserted whitespace is the
        // fragment's first (last) node and the compiler trims it; otherwise it stays, and only a
        // line-box edge absorbs it.
        std::array::from_fn(|s| free || s & SEEN == 0)
    }
}

impl<'a> Printer<'a> {
    /// The render class of a fragment node for the break-freeness scans.
    fn boxless_class(&self, node: &FragmentNode<'_>) -> Class {
        match node {
            FragmentNode::Comment(_) => Class::Transparent,
            n if n.is_compiler_hoisted() => Class::Transparent,
            FragmentNode::Text(t) if t.is_collapsible_ws_only => Class::Space,
            FragmentNode::Text(t) => {
                let raw = t.raw(self.source);
                Class::Text {
                    before_ws: !Self::text_glued_before(raw),
                    after_ws: !Self::text_glued_after(raw),
                }
            }
            n if self.is_boxless_raw_text_element(n) => Class::Boxless,
            n if self.is_block_element_node(n) => Class::Block,
            _ => Class::Content,
        }
    }

    /// Compute a fragment's [`FragmentGlue`] from its nodes and edges — one forward and one
    /// backward pass for each scan.
    fn compute_fragment_glue(&self, nodes: &[FragmentNode<'_>], edges: EdgeFree) -> FragmentGlue {
        let n = nodes.len();
        let classes: Vec<Class> = nodes.iter().map(|node| self.boxless_class(node)).collect();

        // back[g][s]: a break at gap g (before node g), scanning backward from state s.
        let mut back: Vec<[bool; 4]> = Vec::with_capacity(n + 1);
        back.push(FragmentGlue::edge(edges.start));
        for g in 1..=n {
            let next = FragmentGlue::step(classes[g - 1], back[g - 1], |c| {
                matches!(c, Class::Text { after_ws: true, .. })
            });
            back.push(next);
        }
        // fwd[g][s]: the same, scanning forward from gap g.
        let mut fwd: Vec<[bool; 4]> = vec![[false; 4]; n + 1];
        fwd[n] = FragmentGlue::edge(edges.end);
        for g in (0..n).rev() {
            fwd[g] = FragmentGlue::step(classes[g], fwd[g + 1], |c| {
                matches!(
                    c,
                    Class::Text {
                        before_ws: true,
                        ..
                    }
                )
            });
        }
        let gap_free = |g: usize| back[g][0] || fwd[g][0];
        let inline_laid: Box<[bool]> = (0..n)
            .map(|i| classes[i] == Class::Boxless && !(gap_free(i) && gap_free(i + 1)))
            .collect();

        // The declaration / global element glue scan: a hoisted node is stepped over; a nested
        // `<script>` / `<style>` that takes its own line answers as the block it then is. One that
        // keeps its glue lays out inline and renders nothing, so the neighbour past it is not
        // what decides: whether a break beside the declaration renders is then the same question
        // the element's own layout asked, and the scan defers to that answer (`None`) — which is
        // what keeps a declaration and such an element in one run agreeing.
        let glue_step = |j: usize, next: Option<bool>, facing_ws: bool| -> Option<bool> {
            match classes[j] {
                Class::Space | Class::Block => Some(false),
                Class::Text { .. } => Some(!facing_ws),
                Class::Transparent if matches!(nodes[j], FragmentNode::Comment(_)) => Some(true),
                Class::Transparent => next,
                Class::Boxless if inline_laid[j] => None,
                Class::Boxless => Some(false),
                Class::Content => Some(true),
            }
        };
        let mut before: Vec<Option<bool>> = Vec::with_capacity(n);
        for i in 0..n {
            let at = if i == 0 {
                Some(false)
            } else {
                let facing_ws = matches!(classes[i - 1], Class::Text { after_ws: true, .. });
                glue_step(i - 1, before[i - 1], facing_ws)
            };
            before.push(at);
        }
        let mut after: Vec<Option<bool>> = vec![Some(false); n];
        for i in (0..n.saturating_sub(1)).rev() {
            let facing_ws = matches!(
                classes[i + 1],
                Class::Text {
                    before_ws: true,
                    ..
                }
            );
            after[i] = glue_step(i + 1, after[i + 1], facing_ws);
        }
        let glued_before: Box<[bool]> = (0..n)
            .map(|i| before[i].unwrap_or_else(|| !gap_free(i)))
            .collect();
        let glued_after: Box<[bool]> = (0..n)
            .map(|i| after[i].unwrap_or_else(|| !gap_free(i + 1)))
            .collect();
        FragmentGlue {
            inline_laid,
            glued_before,
            glued_after,
            edge_back: back.iter().map(|b| b[SEEN]).collect(),
            edge_fwd: fwd.iter().map(|f| f[SEEN]).collect(),
        }
    }

    /// The [`FragmentGlue`] of the node slice `nodes`, computed on first ask and kept for the
    /// document.
    ///
    /// A reader may hold a whole fragment or a run of one — the boundary-trimmed children, or a
    /// root segment the section comments and ignore ranges split off — and the run is what it
    /// prints, so the flags are the run's own: its ends are its edges. Any run of an indexed
    /// fragment reads that fragment's edges ([`EdgeFree`], registered by the document index) at
    /// both of its ends, found by address: exact for the fragment itself and for its
    /// boundary-trimmed children, since the whitespace a trimmed run drops at its ends is the
    /// compiler's to trim, and moot for a root segment, since the root's edges absorb nothing.
    /// No reader holds any other run that stops short of its fragment's ends — a debug
    /// assertion checks it, since such a run would be handed an edge it does not have. A run
    /// no index covers — a document the index skipped, or a printer that formatted no
    /// document — reads edges that absorb nothing.
    fn fragment_glue(&self, nodes: &[FragmentNode<'_>]) -> Rc<FragmentGlue> {
        let addr = nodes.as_ptr() as usize;
        let key = (addr, nodes.len());
        if let Some(glue) = self.fragment_glue_cache.borrow().get(&key) {
            return Rc::clone(glue);
        }
        let size = size_of::<FragmentNode<'_>>();
        let index = self.fragment_edges.borrow();
        let fragment = index
            .range(..=addr)
            .next_back()
            .map(|(&start, &entry)| ((addr - start) / size, entry))
            .filter(|&(offset, entry)| offset + nodes.len() <= entry.len);
        debug_assert!(
            nodes.is_empty() || index.is_empty() || fragment.is_some(),
            "a glue slice lies outside every indexed fragment"
        );
        debug_assert!(
            fragment.is_none_or(|(offset, entry)| {
                let (lead, trail) = entry.ws_ends;
                (!entry.edges.start && !entry.edges.end)
                    || (offset <= lead && entry.len - offset - nodes.len() <= trail)
            }),
            "a run that stops short of its fragment's ends reads the fragment's edges"
        );
        let edges = fragment.map_or_else(EdgeFree::default, |(_, entry)| entry.edges);
        drop(index);
        let glue = Rc::new(self.compute_fragment_glue(nodes, edges));
        self.fragment_glue_cache
            .borrow_mut()
            .insert(key, Rc::clone(&glue));
        glue
    }

    /// Whether the node at `i` is glued to the nearest **content** before (`before`) or after
    /// it — the neighbour the compiler's whitespace rules actually see, which is what decides
    /// whether breaking there would inject a rendered space.
    ///
    /// Four things make a neighbour not-content. Three are the compiler's own answer: a
    /// **hoisted** sibling vanishes from those rules, so the scan steps over it (a run of
    /// `{@const}`s is not glued to itself — stepping over hoisted neighbours can only end at a
    /// text, whose edges answer, or at the fragment edge, which is not content). "Hoisted" is the
    /// compiler's WHOLE list here ([`FragmentNode::is_compiler_hoisted`]), the four
    /// global `svelte:*` elements included — the trim readers' narrower set leaves them out for a
    /// layout reason that is no answer to this question, and asking it made the scan stop at a
    /// global element as at a block one, so `a{const x = 1}<svelte:window />b` split around both.
    /// Further: a
    /// **whitespace-only** text is the separator, not the content; and a content **text** counts
    /// only when its facing edge carries no collapsible whitespace, since that whitespace is the
    /// separator instead. The fourth is the default display's: a **block element** owns its own
    /// line ([`Self::owns_own_line`]), so the boundary beside it is a break whatever the author
    /// wrote, and whitespace at a block-level boundary is not rendered under the default display
    /// of a block box (a closed `<dialog>` is the known exception; the four global `svelte:*`
    /// elements, block-classified too, never reach this arm, since the hoist arm above steps over
    /// them). A nested `<script>` / `<style>` with a body is block-classified as well, but it
    /// renders **no box**. One that takes its own line answers as the block it then is; one that
    /// keeps its glue ([`Self::is_glued_raw_text_element`]) lays out inline and renders nothing,
    /// so the answer on that side is the element's own — whether a break beside it would
    /// render — and `a{const x = 1}<script>…</script>b` stays on one line. Either way the
    /// declaration agrees with the element's layout. Anything else is content and glues.
    ///
    /// Read from the slice's [`FragmentGlue`], computed in one pass each way.
    ///
    /// The block-element answer is what keeps the declaration's own line a one-pass fixed point:
    /// the layout must not read a signal its own output rewrites. Counted as glued content, a
    /// block element made `<div>{y}</div>{@const z = 1}t` glued on both sides, so pass 1 gave the
    /// block element its own line and kept `{@const z = 1}t` glued; pass 2 read the declaration as
    /// glued on one side only and split the `t` off onto a line of its own.
    ///
    /// ⚠️ There is deliberately **no byte-adjacency test** here, and adding one was a render
    /// bug. Sibling spans tile every fragment except the ROOT, where `<script>` / `<style>` /
    /// `<svelte:options>` are lifted out of `Fragment::nodes` and leave a byte gap — but the
    /// compiler removes exactly those before its whitespace rules run, so the survivors around
    /// the gap are adjacent to it: `a<script>…</script>{const y = 2}b` renders `ab`, and a
    /// break injected at the gap is a rendered space. A real separator always materializes as
    /// its own whitespace text node (or a text's own edge), which the arms above answer — a
    /// byte gap between consecutive fragment nodes is never render-whitespace. Pinned by
    /// [root_script_gap](../../../../../tests/fixtures/svelte/tags/root_script_gap/).
    pub(super) fn glued_to_content(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
        before: bool,
    ) -> bool {
        let glue = self.fragment_glue(nodes);
        if before {
            glue.glued_before[i]
        } else {
            glue.glued_after[i]
        }
    }

    /// Whether `node` is a nested `<script>` / `<style>` that is block-classified — one with a
    /// body ([`Self::is_block_element`]'s raw-text overlay) — and so renders **no box** while the
    /// printer would otherwise give it a line of its own. (An empty one is already inline.)
    ///
    /// The UA stylesheet gives both `display: none`, so the whitespace on either side of one stays
    /// in the same inline formatting context: `a<script>…</script>b` renders `ab`, and a break on
    /// either side renders a space. The block-boundary licence a block box earns does not reach
    /// it.
    pub(super) fn is_boxless_raw_text_element(&self, node: &FragmentNode<'_>) -> bool {
        matches!(node, FragmentNode::Element(el)
            if el.facts.is_raw_text() && self.is_block_element(el))
    }

    /// Whether the node at `i` is a nested `<script>` / `<style>` with a body that **keeps its
    /// glue**: a line break on at least one side of it would render, so it lays out as a glued
    /// inline element whose body breaks rather than taking its own line.
    ///
    /// A break beside it is render-free only where it merges into whitespace that already
    /// renders, or where it disappears at the edge of a line box — a block element beside it, or
    /// a block parent's content edge, with only render-invisible nodes (comments, hoisted nodes,
    /// other such elements) between. The compiler trims whitespace at a fragment's content edge
    /// too, but only up to the first node it keeps, and this element is one it keeps. See
    /// [`EdgeFree`] for which edges are line-box edges.
    pub(super) fn is_glued_raw_text_element(&self, nodes: &[FragmentNode<'_>], i: usize) -> bool {
        self.is_boxless_raw_text_element(&nodes[i]) && self.fragment_glue(nodes).inline_laid[i]
    }

    /// Index every fragment's [`FragmentGlue`] once per document, top down — a block tag's body
    /// reads its edges from where the tag sits in its parent, so the parent is analysed first.
    /// Only a fragment holding a node that asks is analysed: a nested `<script>` / `<style>`
    /// with a body, a declaration, a global element, or a block tag whose bodies need edges.
    ///
    /// A document with no nested `<script>` / `<style>` ([`Root::holds_nested_raw_text`]) is not
    /// indexed at all: every flag of a slice's [`FragmentGlue`] reads its edges only through a
    /// boxless element in that slice (`inline_laid` marks nothing else, and the glue scans
    /// defer to the edge scans only past one), and a block tag's body edges feed nothing but
    /// that body's own flags. Without one, the edges that absorb nothing — what an unindexed
    /// slice reads — give every flag the answer the index would.
    pub(crate) fn index_fragment_edges(&self, root: &Root<'_>) {
        if root.holds_nested_raw_text {
            self.index_fragment(root.fragment.nodes, EdgeFree::default());
        }
    }

    fn index_fragment(&self, nodes: &[FragmentNode<'_>], edges: EdgeFree) {
        if nodes.is_empty() {
            return;
        }
        let asks = nodes.iter().any(|node| {
            is_control_flow_block(node)
                || node.is_declaration()
                || matches!(node, FragmentNode::SpecialElement(se) if se.kind.is_global())
                || self.is_boxless_raw_text_element(node)
        });
        let ws_lead = nodes
            .iter()
            .take_while(|n| n.is_whitespace_only_text())
            .count();
        let ws_trail = nodes
            .iter()
            .rev()
            .take_while(|n| n.is_whitespace_only_text())
            .count();
        self.fragment_edges.borrow_mut().insert(
            nodes.as_ptr() as usize,
            IndexedFragment {
                len: nodes.len(),
                ws_ends: (ws_lead, ws_trail),
                edges,
            },
        );
        let glue = asks.then(|| self.fragment_glue(nodes));
        let outside = |i: usize| -> EdgeFree {
            glue.as_ref().map_or_else(EdgeFree::default, |g| EdgeFree {
                start: g.edge_back[i],
                end: g.edge_fwd[i + 1],
            })
        };
        let none = EdgeFree::default();
        for (i, node) in nodes.iter().enumerate() {
            match node {
                FragmentNode::Element(el) => {
                    let facts = el.facts;
                    let block = el.kind != ElementKind::Component
                        && facts.is_block()
                        && !facts.is_ws_sensitive();
                    let e = EdgeFree {
                        start: block,
                        end: block,
                    };
                    self.index_fragment(el.fragment.nodes, e);
                }
                FragmentNode::SpecialElement(se) => {
                    // A global element's content never joins the page's line boxes.
                    let global = se.kind.is_global();
                    let e = EdgeFree {
                        start: global,
                        end: global,
                    };
                    self.index_fragment(se.fragment.nodes, e);
                }
                FragmentNode::IfBlock(b) => {
                    // An `{:else if}` is an `IfBlock` inside the alternate, whose edges it reads
                    // from there.
                    let e = outside(i);
                    self.index_fragment(b.consequent.nodes, e);
                    if let Some(alt) = &b.alternate {
                        self.index_fragment(alt.nodes, e);
                    }
                }
                FragmentNode::AwaitBlock(b) => {
                    let e = outside(i);
                    for f in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                        self.index_fragment(f.nodes, e);
                    }
                }
                FragmentNode::KeyBlock(b) => self.index_fragment(b.fragment.nodes, outside(i)),
                FragmentNode::EachBlock(b) => {
                    self.index_fragment(b.body.nodes, none);
                    if let Some(f) = &b.fallback {
                        self.index_fragment(f.nodes, outside(i));
                    }
                }
                FragmentNode::SnippetBlock(b) => self.index_fragment(b.body.nodes, none),
                _ => {}
            }
        }
    }
}
