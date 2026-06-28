// Svelte fragment conversions
//
// Converts internal fragment and element nodes to public format.
// The fragment is the core container for template content.

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};
use tsv_ts::ast::convert::convert_expression;

use super::{
    convert_attribute_node, convert_await_block, convert_const_tag, convert_debug_tag,
    convert_declaration_tag, convert_each_block, convert_html_tag, convert_if_block,
    convert_key_block, convert_render_tag, convert_snippet_block, convert_special_element,
    span_to_name_loc,
};

pub(super) fn convert_fragment<'src>(
    fragment: &internal::Fragment<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Fragment<'src> {
    public::Fragment {
        node_type: "Fragment",
        nodes: fragment
            .nodes
            .iter()
            .map(|node| convert_fragment_node(node, source, loc, interner))
            .collect(),
    }
}

pub(super) fn convert_fragment_node<'src>(
    node: &internal::FragmentNode<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::FragmentNode<'src> {
    match node {
        internal::FragmentNode::Element(elem) => {
            let converted = convert_element(elem, source, loc, interner);
            // Return appropriate variant based on element kind
            match elem.kind {
                internal::ElementKind::Component => public::FragmentNode::Component(converted),
                internal::ElementKind::Html => public::FragmentNode::RegularElement(converted),
            }
        }
        internal::FragmentNode::ExpressionTag(tag) => {
            public::FragmentNode::ExpressionTag(convert_expression_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::Text(text) => {
            public::FragmentNode::Text(convert_text(text, source))
        }
        internal::FragmentNode::Comment(comment) => {
            public::FragmentNode::Comment(convert_comment(comment, source))
        }
        internal::FragmentNode::IfBlock(block) => {
            public::FragmentNode::IfBlock(convert_if_block(block, source, loc, interner))
        }
        internal::FragmentNode::EachBlock(block) => {
            public::FragmentNode::EachBlock(convert_each_block(block, source, loc, interner))
        }
        internal::FragmentNode::AwaitBlock(block) => {
            public::FragmentNode::AwaitBlock(convert_await_block(block, source, loc, interner))
        }
        internal::FragmentNode::KeyBlock(block) => {
            public::FragmentNode::KeyBlock(convert_key_block(block, source, loc, interner))
        }
        internal::FragmentNode::SnippetBlock(block) => {
            public::FragmentNode::SnippetBlock(convert_snippet_block(block, source, loc, interner))
        }
        internal::FragmentNode::HtmlTag(tag) => {
            public::FragmentNode::HtmlTag(convert_html_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::ConstTag(tag) => {
            public::FragmentNode::ConstTag(convert_const_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::DeclarationTag(tag) => public::FragmentNode::DeclarationTag(
            convert_declaration_tag(tag, source, loc, interner),
        ),
        internal::FragmentNode::DebugTag(tag) => {
            public::FragmentNode::DebugTag(convert_debug_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::RenderTag(tag) => {
            public::FragmentNode::RenderTag(convert_render_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::SpecialElement(elem) => public::FragmentNode::SpecialElement(
            convert_special_element(elem, source, loc, interner),
        ),
    }
}

fn convert_comment(comment: &internal::HtmlComment, source: &str) -> public::Comment {
    // Note: internal uses `content`, public uses `data` (Svelte's naming)
    public::Comment {
        node_type: "Comment",
        start: comment.span.start,
        end: comment.span.end,
        data: comment.content(source).to_string(),
    }
}

fn convert_element<'src>(
    elem: &internal::Element<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Element<'src> {
    // Set node_type based on element kind
    let node_type = match elem.kind {
        internal::ElementKind::Component => "Component",
        internal::ElementKind::Html => "RegularElement",
    };

    public::Element {
        node_type,
        start: elem.span.start,
        end: elem.span.end,
        name: interner.resolve_infallible(elem.name).to_string(),
        name_loc: span_to_name_loc(elem.name_span, loc),
        kind: elem.kind,
        attributes: elem
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, loc, interner))
            .collect(),
        fragment: convert_fragment(&elem.fragment, source, loc, interner),
    }
}

pub(super) fn convert_expression_tag<'src>(
    tag: &internal::ExpressionTag<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ExpressionTag<'src> {
    // Delegate to tsv_ts for expression conversion
    let ts_expr = convert_expression(&tag.expression, source, loc, interner, 0);

    public::ExpressionTag {
        node_type: "ExpressionTag",
        start: tag.span.start,
        end: tag.span.end,
        expression: ts_expr,
    }
}

pub(super) fn convert_text(text: &internal::Text, source: &str) -> public::Text {
    // raw contains original source with entities (&lt;, &#65;, etc.)
    // data contains decoded text (<, A, etc.)
    public::Text {
        node_type: "Text",
        start: text.span.start,
        end: text.span.end,
        raw: text.raw(source).to_string(),
        data: text.data(source).into_owned(),
    }
}
