//! Byte→UTF-16 offset translation as a mutating walk over the typed public AST.
//!
//! Counterpart to the `Value` walk (`tsv_ts::ast::convert::translate_byte_to_char_offsets`,
//! which the `Value` oracle path — `convert_ast_json` — runs over the whole
//! serialized document): same translation semantics, but applied to the typed
//! tree so `convert_ast_json_string` can serialize multibyte sources directly —
//! no intermediate `Value` materialization on the wire hot path.
//!
//! Unlike the `tsv_ts`/`tsv_css` typed walks this is a **hybrid** walk, because
//! the Svelte public AST embeds three kinds of position-bearing content:
//!
//! - **Typed Svelte nodes** — visited field-by-field here. They carry
//!   `start`/`end` plus (on elements, attributes, directives) a `name_loc`
//!   whose positions hold a byte `character` and a byte-derived `column`.
//! - **Embedded typed subtrees** — `ExpressionIsland` fields (template
//!   `{expr}` tags, block tests, snippet parameters, …) whose `Typed` arm
//!   delegates to `tsv_ts`'s expression-level typed walk; the `css` envelope
//!   delegates to `tsv_css`'s `StyleSheet` typed walk (typed parts only — its
//!   `Value` islands are handled here, see below).
//! - **`serde_json::Value` islands** — directive shorthand expressions,
//!   block patterns, root `comments`, the `<style>` envelope's
//!   `attributes`/`content.comment`, `ProgramIsland::Attached` script
//!   contents, and any `ExpressionIsland::Attached` arm (an expression
//!   carrying attached template comments) — delegate to the `Value` walk
//!   itself, the exact function the `Value` oracle path applies to these
//!   same subtrees, so island semantics are identical by construction.
//!
//! Parity contract: output must be byte-identical to the `Value` walk. The
//! `name_loc` rule ported from there: each position's byte offset is its own
//! `character` field; the `column` translates delta-preserving (via
//! `tsv_ts::ast::convert::translate_column` — one definition of the column
//! math) and `character` becomes the absolute UTF-16 offset. `line` is
//! byte-independent and untouched.
//!
//! Every struct with positions must be visited and every node-bearing field
//! recursed into; a missed field means silently untranslated offsets. Gates:
//! the fixture suite's string-path identity check plus its typed-walk parity
//! probes (`fixtures_validate` — a synthesized multibyte variant of every
//! `.svelte` fixture), `json_profile`'s per-file `direct == value` comparison,
//! and `corpus:compare:parse --multibyte-only` against Svelte's parser.

use tsv_lang::{ByteToCharMap, LocationTracker};

use super::super::public::*;

/// Translate all byte-based positions in a typed public AST to UTF-16
/// code-unit positions, in place.
///
/// For ASCII-only sources this is a no-op (byte == UTF-16 offset).
pub fn translate_byte_to_char_offsets_typed(
    root: &mut Root<'_>,
    map: &ByteToCharMap,
    tracker: &LocationTracker,
) {
    if !map.has_multibyte() {
        return;
    }
    let t = Translator { map, tracker };
    t.root(root);
}

struct Translator<'a> {
    map: &'a ByteToCharMap,
    tracker: &'a LocationTracker,
}

impl Translator<'_> {
    #[inline]
    fn pos(&self, p: &mut u32) {
        *p = self.map.byte_to_char(*p);
    }

    /// Translate a `serde_json::Value` island with the `Value` walk — the same
    /// function the `Value` oracle path runs over the whole document.
    fn value(&self, v: &mut serde_json::Value) {
        tsv_ts::ast::convert::translate_byte_to_char_offsets(v, self.map, self.tracker);
    }

    /// Translate an expression island: typed arm via `tsv_ts`'s typed
    /// expression walk, attached arm (comments injected) via the `Value` walk.
    fn island(&self, i: &mut ExpressionIsland<'_>) {
        match i {
            ExpressionIsland::Typed(e) => {
                tsv_ts::ast::convert::translate_expression_byte_to_char_offsets_typed(
                    e,
                    self.map,
                    self.tracker,
                );
            }
            ExpressionIsland::Attached(v) => self.value(v),
        }
    }

    fn name_loc(&self, n: &mut NameLocation) {
        self.name_position(&mut n.start);
        self.name_position(&mut n.end);
    }

    /// Mirror of the `Value` walk's `name_loc` rule: the byte offset is the
    /// position's own `character`; `column` translates delta-preserving,
    /// `character` becomes the absolute UTF-16 offset.
    #[allow(clippy::cast_possible_truncation)]
    fn name_position(&self, p: &mut NamePosition) {
        let byte = p.character;
        p.column =
            tsv_ts::ast::convert::translate_column(byte, p.column as u64, self.map, self.tracker)
                as usize;
        p.character = self.map.byte_to_char(byte);
    }

    fn root(&self, n: &mut Root<'_>) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        if let Some(css) = &mut n.css {
            self.style_sheet(css);
        }
        for js in &mut n.js {
            self.value(js);
        }
        self.fragment(&mut n.fragment);
        if let Some(options) = &mut n.options {
            self.svelte_options(options);
        }
        for comment in &mut n.comments {
            self.value(comment);
        }
        if let Some(script) = &mut n.instance {
            self.script(script);
        }
        if let Some(script) = &mut n.module {
            self.script(script);
        }
    }

    /// The `<style>` envelope: `tsv_css`'s typed walk covers the typed parts
    /// (`start`/`end`, CSS `children`, `content.start`/`content.end`); its
    /// `Value` islands (`attributes` — Svelte attribute JSON — and
    /// `content.comment`) are this crate's to translate.
    fn style_sheet(&self, n: &mut tsv_css::ast::public::StyleSheet<'_>) {
        tsv_css::ast::convert::translate_style_sheet_byte_to_char_offsets_typed(n, self.map);
        for attr in &mut n.attributes {
            self.value(attr);
        }
        if let Some(comment) = &mut n.content.comment {
            self.value(comment);
        }
    }

    fn script(&self, n: &mut Script<'_>) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        match &mut n.content {
            ProgramIsland::Typed(program) => {
                tsv_ts::ast::convert::translate_byte_to_char_offsets_typed(
                    program,
                    self.map,
                    self.tracker,
                );
            }
            ProgramIsland::Attached(value) => self.value(value),
        }
        for attr in &mut n.attributes {
            self.attribute_node(attr);
        }
    }

    fn svelte_options(&self, n: &mut SvelteOptions<'_>) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        for attr in &mut n.attributes {
            self.attribute_node(attr);
        }
        if let Some(custom_element) = &mut n.custom_element {
            self.value(custom_element);
        }
    }

    fn fragment(&self, n: &mut Fragment<'_>) {
        for node in &mut n.nodes {
            self.fragment_node(node);
        }
    }

    fn fragment_node(&self, n: &mut FragmentNode<'_>) {
        match n {
            FragmentNode::Component(e) | FragmentNode::RegularElement(e) => self.element(e),
            FragmentNode::SpecialElement(e) => self.special_element(e),
            FragmentNode::ExpressionTag(t) => self.expression_tag(t),
            FragmentNode::Text(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
            }
            FragmentNode::Comment(c) => {
                self.pos(&mut c.start);
                self.pos(&mut c.end);
            }
            FragmentNode::IfBlock(b) => {
                self.pos(&mut b.start);
                self.pos(&mut b.end);
                self.island(&mut b.test);
                self.fragment(&mut b.consequent);
                if let Some(alternate) = &mut b.alternate {
                    self.fragment(alternate);
                }
            }
            FragmentNode::EachBlock(b) => {
                self.pos(&mut b.start);
                self.pos(&mut b.end);
                self.island(&mut b.expression);
                self.fragment(&mut b.body);
                if let Some(context) = &mut b.context {
                    self.value(context);
                }
                if let Some(key) = &mut b.key {
                    self.island(key);
                }
                if let Some(fallback) = &mut b.fallback {
                    self.fragment(fallback);
                }
            }
            FragmentNode::AwaitBlock(b) => {
                self.pos(&mut b.start);
                self.pos(&mut b.end);
                self.island(&mut b.expression);
                if let Some(value) = &mut b.value {
                    self.value(value);
                }
                if let Some(error) = &mut b.error {
                    self.value(error);
                }
                if let Some(pending) = &mut b.pending {
                    self.fragment(pending);
                }
                if let Some(then_block) = &mut b.then_block {
                    self.fragment(then_block);
                }
                if let Some(catch_block) = &mut b.catch_block {
                    self.fragment(catch_block);
                }
            }
            FragmentNode::KeyBlock(b) => {
                self.pos(&mut b.start);
                self.pos(&mut b.end);
                self.island(&mut b.expression);
                self.fragment(&mut b.fragment);
            }
            FragmentNode::SnippetBlock(b) => {
                self.pos(&mut b.start);
                self.pos(&mut b.end);
                self.island(&mut b.expression);
                for param in &mut b.parameters {
                    self.island(param);
                }
                self.fragment(&mut b.body);
            }
            FragmentNode::HtmlTag(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
                self.island(&mut t.expression);
            }
            FragmentNode::ConstTag(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
                self.value(&mut t.declaration);
            }
            FragmentNode::DeclarationTag(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
                self.value(&mut t.declaration);
            }
            FragmentNode::DebugTag(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
                for identifier in &mut t.identifiers {
                    self.island(identifier);
                }
            }
            FragmentNode::RenderTag(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
                self.island(&mut t.expression);
            }
        }
    }

    fn element(&self, n: &mut Element<'_>) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        self.name_loc(&mut n.name_loc);
        for attr in &mut n.attributes {
            self.attribute_node(attr);
        }
        self.fragment(&mut n.fragment);
    }

    fn special_element(&self, n: &mut SpecialElement<'_>) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        self.name_loc(&mut n.name_loc);
        for attr in &mut n.attributes {
            self.attribute_node(attr);
        }
        self.fragment(&mut n.fragment);
        if let Some(tag) = &mut n.tag {
            self.value(tag);
        }
        if let Some(expression) = &mut n.expression {
            self.island(expression);
        }
    }

    fn expression_tag(&self, n: &mut ExpressionTag<'_>) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        self.island(&mut n.expression);
    }

    fn attribute_value_field(&self, f: &mut AttributeValueField<'_>) {
        match f {
            AttributeValueField::True(_) => {}
            AttributeValueField::Single(part) => self.attribute_value_part(part),
            AttributeValueField::Sequence(parts) => {
                for part in parts {
                    self.attribute_value_part(part);
                }
            }
        }
    }

    fn attribute_value_part(&self, p: &mut AttributeValue<'_>) {
        match p {
            AttributeValue::Text(t) => {
                self.pos(&mut t.start);
                self.pos(&mut t.end);
            }
            AttributeValue::ExpressionTag(t) => self.expression_tag(t),
        }
    }

    fn attribute_node(&self, n: &mut AttributeNode<'_>) {
        match n {
            AttributeNode::Attribute(a) => {
                self.pos(&mut a.start);
                self.pos(&mut a.end);
                self.name_loc(&mut a.name_loc);
                self.attribute_value_field(&mut a.value);
            }
            AttributeNode::SpreadAttribute(a) => {
                self.pos(&mut a.start);
                self.pos(&mut a.end);
                self.island(&mut a.expression);
            }
            AttributeNode::AttachTag(a) => {
                self.pos(&mut a.start);
                self.pos(&mut a.end);
                self.island(&mut a.expression);
            }
            AttributeNode::OnDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                if let Some(expression) = &mut d.expression {
                    self.island(expression);
                }
            }
            AttributeNode::BindDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                self.value(&mut d.expression);
            }
            AttributeNode::ClassDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                self.value(&mut d.expression);
            }
            AttributeNode::StyleDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                self.attribute_value_field(&mut d.value);
            }
            AttributeNode::UseDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                if let Some(expression) = &mut d.expression {
                    self.island(expression);
                }
            }
            AttributeNode::TransitionDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                if let Some(expression) = &mut d.expression {
                    self.island(expression);
                }
            }
            AttributeNode::AnimateDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                if let Some(expression) = &mut d.expression {
                    self.island(expression);
                }
            }
            AttributeNode::LetDirective(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
                self.name_loc(&mut d.name_loc);
                if let Some(expression) = &mut d.expression {
                    self.island(expression);
                }
            }
        }
    }
}
