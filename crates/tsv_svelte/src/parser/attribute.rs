// Attribute parsing

use bumpalo::collections::Vec as BumpVec;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::whitespace::{
    brace_interior_start, char_at, is_svelte_ws, name_run_end, skip_svelte_ws,
};
use tsv_lang::{ParseError, Span};
use tsv_ts::ast::internal::{Expression, IdentName, Identifier};

use super::expression_tag::{SequenceLocation, scan_to_matching_brace};
use super::parser_impl::SvelteParser;

// In an attribute value there is no block DISPATCH — blocks (`{#if}`, `{:else}`, `{/if}`)
// and tags (`{@html}`) are *fragment* constructs, and no `{` here opens one. But the four
// markers do not all reach the same answer, and reading them as one rule is what let a
// block live in an attribute value for a release:
//
//   - `{:` and `{/` are simply unguarded. Svelte's `read_sequence` hands everything after
//     the `{` to the JS parser, so `{/a}` is not a block close but the expression `/a}`,
//     which fails to parse — as `{:a}` does. The comment forms (`{/* c */ x}`, `{// c⏎ x}`)
//     need no special case: they are simply valid JS.
//   - `{#` and `{@` are GUARDED, *before* the expression is read
//     (`SvelteParser::check_sequence_placement`, Svelte's `block_invalid_placement` /
//     `tag_invalid_placement`). Assuming they fail as JS is the false step: `{#x in y}` is
//     the ergonomic brand check, the one production where a private name is an operand, so
//     it PARSES — the guard is the only thing standing between it and an over-acceptance.
//
// A helper mirroring the *lexer's* brace dispatch and reading the four as literal text
// over-accepts (the canonical parser rejects all four), and the literal text then
// round-trips into output tsv's own parser rejects: an unquoted `a={/a` re-emitted quoted
// as `a="{/a"`, where the `{` reopens as an expression and runs unterminated.

/// Svelte's `regex_token_ending_character = /[\s=/>"']/` — the characters that end an
/// attribute/directive name run (`read_tag` in `1-parse/state/element.js`). Everything else
/// (including `%`, `&`, `#`, digits, …) is part of the name; the lexer's identifier scan is
/// narrower, so the name reader extends past its token end up to one of these.
///
/// A `char` question, not a byte one: the `\s` arm is Unicode ([`is_svelte_ws`]), so no lone
/// byte can answer it.
const fn is_attr_name_terminator(c: char) -> bool {
    is_svelte_ws(c) || matches!(c, '=' | '/' | '>' | '"' | '\'')
}

/// Whether byte offset `i` in `source` holds an attribute-name character — i.e. the run does
/// not end there (`regex_token_ending_character` match, or EOF).
fn is_attr_name_char_at(source: &str, i: usize) -> bool {
    char_at(source, i).is_some_and(|(c, _)| !is_attr_name_terminator(c))
}

/// Byte offset of the first attribute-name terminator at/after `start`, mirroring Svelte's
/// `read_tag(regex_token_ending_character)`. The name is `source[start..end]`.
///
/// The attribute-name twin of [`tag_name_end`](super::element::tag_name_end): same scan
/// ([`name_run_end`]), different terminator class, and free rather than a method for the
/// same reason — so it can be graded directly by a unit test.
fn attr_name_end(source: &str, start: usize) -> usize {
    name_run_end(source, start, is_attr_name_terminator)
}

/// Which of Svelte's two attribute readers a tag head runs (`1-parse/state/element.js`).
///
/// A **top-level** `<script>` / `<style>` uses `read_static_attribute`, every other tag
/// `read_attribute`. The two are not "the same reader with expressions turned off" — the
/// static one is strictly a name run plus an optional raw value, and each construct the
/// element reader adds is a separate `if` there that the static reader simply does not have.
/// Whenever this enum is consulted, it is that structural difference being asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttributeReader {
    /// `read_attribute`: JS comments between attributes, `{...spread}` / `{shorthand}` /
    /// `{@attach}`, directives, `{expr}` values, and whitespace before the `=`.
    Element,
    /// `read_static_attribute`: a `read_tag` name run, then — only if `=` follows it with no
    /// gap — a `regex_attribute_value` raw value. A `{` is an ordinary name/value character,
    /// a `:` name is a plain attribute, and a comment is not a token.
    Static,
}

/// Directive prefix types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectiveType {
    On,
    Bind,
    Class,
    Style,
    Use,
    Transition,
    In,
    Out,
    Animate,
    Let,
}

impl DirectiveType {
    fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "on" => Some(Self::On),
            "bind" => Some(Self::Bind),
            "class" => Some(Self::Class),
            "style" => Some(Self::Style),
            "use" => Some(Self::Use),
            "transition" => Some(Self::Transition),
            "in" => Some(Self::In),
            "out" => Some(Self::Out),
            "animate" => Some(Self::Animate),
            "let" => Some(Self::Let),
            _ => None,
        }
    }

    /// The `TransitionDirective` direction this prefix carries, or `None` for a
    /// prefix that isn't a transition. `transition:` runs both ways; `in:`/`out:`
    /// are the same node type with one side (Svelte has no In/OutDirective).
    const fn transition_direction(self) -> Option<TransitionDirection> {
        match self {
            Self::Transition => Some(TransitionDirection::Both),
            Self::In => Some(TransitionDirection::In),
            Self::Out => Some(TransitionDirection::Out),
            _ => None,
        }
    }
}

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Return `end + 1` if the byte at `end` is a quote character, else `end`.
    ///
    /// Used when the last value part of a quoted attribute is an ExpressionTag:
    /// the `}` is included in the ExpressionTag span but the closing `"` is not.
    fn end_past_optional_quote(&self, end: usize) -> usize {
        if end < self.source.len() && matches!(self.source.as_bytes()[end], b'"' | b'\'') {
            end + 1
        } else {
            end
        }
    }

    /// Parse attribute list (e.g., `lang="ts" class="foo"`)
    /// Consumes tokens until we hit `>` or `/>`
    ///
    /// Supports:
    /// - Standard attributes: `name="value"` or `name={expr}`
    /// - Boolean attributes: `disabled`
    /// - Directives: `on:click`, `bind:value`, `class:class1`, etc.
    /// - Attach tags: `{@attach expr}` (Svelte 5.29+)
    /// - Spread attributes: `{...obj}` (Svelte 3+)
    /// - Shorthand attributes: `{name}` (equivalent to `name={name}`)
    pub(crate) fn parse_attributes(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeNode<'arena>>, ParseError> {
        self.parse_attributes_inner(AttributeReader::Element)
    }

    /// Parse the attribute list of a top-level `<script>` / `<style>` tag head, which Svelte
    /// reads with [`AttributeReader::Static`] — no comments, no spread/shorthand/attach, no
    /// directives, and a raw value in which `{a: A}` is literal text rather than an expression.
    pub(crate) fn parse_attributes_literal(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeNode<'arena>>, ParseError> {
        self.parse_attributes_inner(AttributeReader::Static)
    }

    fn parse_attributes_inner(
        &mut self,
        reader: AttributeReader,
    ) -> Result<BumpVec<'arena, AttributeNode<'arena>>, ParseError> {
        let mut attributes = self.bvec();

        loop {
            // Skip JS comments (// and /* */) between attributes. `read_static_attribute` has
            // no `read_comment` loop, so in a top-level `<script>`/`<style>` head a comment is
            // not trivia: the `/` ends the attribute list and the required `>` is missing.
            if reader == AttributeReader::Element {
                while self.check(TokenKind::Slash) {
                    if !self.try_read_js_comment()? {
                        break; // Regular slash (self-closing />)
                    }
                }
            }

            // Stop at > or />
            if self.check(TokenKind::RightAngle) || self.check(TokenKind::Slash) {
                break;
            }

            if reader == AttributeReader::Element && self.current_token_opens_a_brace_attribute() {
                // Element attribute reader: `{@attach}`, `{...spread}`, or `{shorthand}`.
                if self.check(TokenKind::TagOpen) {
                    attributes.push(AttributeNode::AttachTag(self.parse_attach_tag()?));
                } else {
                    // Peek ahead to distinguish spread `{...obj}` from shorthand `{name}`.
                    let next_char = self.peek_char_after_brace();
                    if next_char == Some('.') {
                        attributes.push(AttributeNode::SpreadAttribute(
                            self.parse_spread_attribute()?,
                        ));
                    } else {
                        attributes
                            .push(AttributeNode::Attribute(self.parse_shorthand_attribute()?));
                    }
                }
            } else if self.current_token_starts_attribute_name() {
                // A name run. What the lexer made of its first character is not the question —
                // an Identifier token, a symbol the lexer tokenized alone (`<p }>`), or the
                // `{`/`<` a static head reads as an ordinary name character all reach here, and
                // `parse_attribute_or_directive` reads the raw run from `current_start` either
                // way. `>`/`/` (tag close) and, for the element reader, `{`/`<` are peeled off
                // above.
                attributes.push(self.parse_attribute_or_directive(reader)?);
            } else {
                return Err(self.error_expected_found("attribute name or '>'"));
            }
        }

        Ok(attributes)
    }

    /// Does the current token open a `{`-led attribute — `{@attach}`, `{...spread}`, or
    /// `{shorthand}`?
    ///
    /// ⚠️ **Every `{` does, the block markers included.** Svelte has no such token: its
    /// `read_attribute` eats the brace and runs `read_identifier`, so `{#`, `{:` and `{/`
    /// simply leave an interior that is not an identifier and the shorthand reader rejects
    /// them (`attribute_empty_shorthand`). tsv's lexer classifies those braces *first*
    /// ([`TokenKind::BlockOpen`] and friends), so testing only `LeftBrace` here dropped them
    /// through to the attribute-**name** run below — and that is not an over-acceptance but
    /// FABRICATION: `<div {#if a}>` came back as a `RegularElement` carrying two boolean
    /// attributes named `{#if` and `a}`, a shape Svelte's AST never contains.
    ///
    /// `{/* … */}` is not among them — the lexer keeps a comment-led brace a `LeftBrace`, so
    /// it reaches the shorthand reader by the ordinary route (and is rejected there, for the
    /// ordinary reason).
    ///
    /// So the question is exactly [`TokenKind::starts_with_brace`], which the enum answers
    /// under an exhaustive match — re-listing the brace kinds here is what let three of them
    /// slip past in the first place.
    fn current_token_opens_a_brace_attribute(&self) -> bool {
        self.current_kind.starts_with_brace()
    }

    /// Peek at the first non-whitespace character after the opening brace — Svelte's
    /// `parser.eat('{')` + `allow_whitespace()` before the spread/shorthand split.
    fn peek_char_after_brace(&self) -> Option<char> {
        let pos = brace_interior_start(self.source, self.current_start);
        char_at(self.source, pos).map(|(c, _)| c)
    }

    /// Whether the current token begins a (possibly symbol-led) attribute-name run — its first
    /// character is not one of Svelte's name terminators (`/[\s=/>"']/`). The dispatch peels off
    /// `{`/`<` (spread/shorthand/attach) and `>`/`/` (tag close) first, so a non-terminator here
    /// is a leading-symbol name like `<p }>` (Svelte's `read_static_attribute` raw run).
    fn current_token_starts_attribute_name(&self) -> bool {
        is_attr_name_char_at(self.source, self.current_start)
    }

    /// End of the current attribute/directive name run. The lexer already scanned the
    /// leading identifier into `current`; Svelte folds any trailing non-terminator chars
    /// (`%`, `&`, `#`, …) into the same name. In the common case the token already ends
    /// at a terminator or EOF, so the scan stops on its first check.
    //
    // TODO: this covers names that *start* with a lexer-identifier char (the realistic
    // case). Svelte also accepts attribute names starting with a symbol (`<div %foo>`),
    // where the lexer chokes before this dispatch runs — handling that needs the in-tag
    // lexer to stop erroring on symbol-led names. The tag-name half of the guard this once
    // also required now exists — `element.rs`'s `is_valid_tag_name` rejects a symbol-led
    // *tag* name (`<%foo>`) directly, so a lexer change can't turn that into an
    // over-acceptance. Off-frontier (no corpus/real occurrence), deferred deliberately.
    fn attribute_name_run_end(&self) -> usize {
        attr_name_end(self.source, self.current_end)
    }

    /// Parse an attribute or directive
    ///
    /// Detects if the attribute name contains a colon (`:`) indicating a directive,
    /// and routes to the appropriate parser.
    fn parse_attribute_or_directive(
        &mut self,
        reader: AttributeReader,
    ) -> Result<AttributeNode<'arena>, ParseError> {
        // Read the full attribute/directive name as Svelte's `read_tag` does — a raw run up
        // to a token-ending char (`/[\s=/>"']/`). The lexer only scanned the leading
        // identifier; extend it past any special chars (`ysc%%gibberish`) before the
        // directive `:` split so both paths see the whole name. `&'a str` borrows the
        // source, so it survives the `&mut self` calls below.
        let name_start = self.current_start;
        let name_end = self.attribute_name_run_end();
        let name_str = &self.source[name_start..name_end];

        // Check if this is a directive (contains colon). A static head has no directive
        // split at all — `read_static_attribute` never looks at the name's `:`, so
        // `<script on:click={fn}>` is a plain attribute named `on:click` whose value is the
        // literal text `{fn}`, and a nameless `on:` is an attribute rather than an error.
        if reader == AttributeReader::Element
            && let Some(colon_idx) = name_str.find(':')
            && let Some(directive_type) = DirectiveType::from_prefix(&name_str[..colon_idx])
        {
            return self.parse_directive(directive_type, name_str, colon_idx, name_end);
        }

        // Not a directive, parse as regular attribute
        Ok(AttributeNode::Attribute(
            self.parse_attribute_inner(name_start, name_end, reader)?,
        ))
    }

    /// Parse a directive (on:, bind:, class:, style:, use:, transition:, in:, out:, animate:, let:)
    fn parse_directive(
        &mut self,
        directive_type: DirectiveType,
        full_name: &str,
        colon_idx: usize,
        name_end: usize,
    ) -> Result<AttributeNode<'arena>, ParseError> {
        let start = self.current_start;
        let head_span = Span {
            start: start as u32,
            end: name_end as u32,
        };

        // Extract directive name and modifiers from: prefix:name|mod1|mod2
        let after_colon = &full_name[colon_idx + 1..];
        let mut parts = after_colon.split('|');
        // The name is a verbatim source slice (HTML/Svelte attribute names are never
        // entity-decoded), so it's stored as a span — not an arena copy. `start` is the
        // attribute-name token start and `full_name` is that raw token, so the name occupies
        // `source[start + colon_idx + 1 .. + name.len()]`. The borrow lives only for this
        // method (the AST stores `name_span`, not the string).
        let directive_name: &str = parts.next().unwrap_or("");
        let name_start = start + colon_idx + 1;
        let name_span = Span {
            start: name_start as u32,
            end: (name_start + directive_name.len()) as u32,
        };
        let mut modifiers_vec = self.bvec();
        for m in parts {
            let m: &'arena str = self.alloc_str_in(m);
            modifiers_vec.push(m);
        }
        let modifiers: &'arena [&'arena str] = modifiers_vec.into_bump_slice();

        if directive_name.is_empty() {
            return Err(self.error_msg_at(
                &format!("Directive '{}' is missing a name", &full_name[..=colon_idx]),
                start,
            ));
        }

        self.advance_past_name(name_end)?; // consume the (possibly special-char-extended) name

        // Style directives accept expression OR string values, handle separately
        if directive_type == DirectiveType::Style {
            return self.parse_style_directive(name_span, modifiers, start, name_end, head_span);
        }

        // Check for = (directive with value)
        let (expression, expression_tag_span) = if self.check(TokenKind::Equals) {
            self.advance()?; // consume =
            let (expr, tag_span) = self.parse_directive_expression()?;
            (Some(expr), Some(tag_span))
        } else {
            (None, None)
        };

        // Calculate end position
        // For the quoted mustache form ("{expr}") the tag span ends at `}` but the
        // directive includes the closing quote (matching Svelte)
        let end = if let Some(tag_span) = &expression_tag_span {
            self.end_past_optional_quote(tag_span.end as usize)
        } else {
            name_end
        };

        let span = Span {
            start: start as u32,
            end: end as u32,
        };

        // `transition:`/`in:`/`out:` are ONE node type differing only by direction —
        // Svelte has no In/OutDirective — so they share a single construction.
        if let Some(direction) = directive_type.transition_direction() {
            return Ok(AttributeNode::TransitionDirective(TransitionDirective {
                name_span,
                expression,
                modifiers,
                direction,
                span,
                head_span,
                expression_tag_span,
            }));
        }

        // A valueless `bind:`/`class:` synthesizes its expression from its own name.
        let name_expression =
            || self.make_shorthand_identifier(directive_name, colon_idx + 1 + start, name_end);

        match directive_type {
            DirectiveType::On => Ok(AttributeNode::OnDirective(OnDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Bind => Ok(AttributeNode::BindDirective(BindDirective {
                name_span,
                expression: expression.unwrap_or_else(name_expression),
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Class => Ok(AttributeNode::ClassDirective(ClassDirective {
                name_span,
                expression: expression.unwrap_or_else(name_expression),
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Use => Ok(AttributeNode::UseDirective(UseDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Animate => Ok(AttributeNode::AnimateDirective(AnimateDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            DirectiveType::Let => Ok(AttributeNode::LetDirective(LetDirective {
                name_span,
                expression,
                modifiers,
                span,
                head_span,
                expression_tag_span,
            })),
            #[expect(clippy::unreachable)] // Style returns early; transitions are built above
            DirectiveType::Style
            | DirectiveType::Transition
            | DirectiveType::In
            | DirectiveType::Out => {
                unreachable!("Style returns early; transitions are built above")
            }
        }
    }

    /// The `{…}` placement guard for the two directive arms.
    ///
    /// A directive value is an attribute value in Svelte's model — `read_attribute_value`
    /// runs `read_sequence` for every attribute, directive or not — but tsv reaches the
    /// `{…}` form through the **token stream** (`parse_expression_tag`) rather than through
    /// the sequence readers, so it needs the same question asked here or the rule splits by
    /// route: `on:click="{#x in y}"` would reject and `on:click={#x in y}` would answer with
    /// a generic "not an expression".
    ///
    /// The separated spelling asks the same question: the lexer tokenizes `{ #if}` as
    /// `BlockOpen` and [`SvelteParser::check_sequence_placement`] skips the gap too, so both
    /// spellings reach the placement error rather than the arm's generic "not an expression".
    /// A quoted directive value (`on:click="{ #x in y}"`) went further still and *parsed*,
    /// since the brand check is a valid expression once the marker stops being read as one.
    ///
    /// ⚠️ **The token test is load-bearing, not a fast path.** `check_sequence_placement`
    /// reads the byte after `current_start` and so needs `current_start` to BE a `{`; the
    /// only thing that knows it is the token kind. On a `String` token `current_start` is the
    /// opening quote, and `<div style:color="#fff">` — valid Svelte — would have its `#`
    /// read as a block marker and be rejected.
    fn check_directive_value_placement(&self) -> Result<(), ParseError> {
        if matches!(self.current_kind, TokenKind::BlockOpen | TokenKind::TagOpen) {
            self.check_sequence_placement(self.current_start, SequenceLocation::AttributeValue)?;
        }
        Ok(())
    }

    /// Parse directive expression (the part after `=`)
    /// Returns the expression and the span of the expression tag (for comment lookup)
    ///
    /// Accepts both `{expr}` and `"{expr}"` (quoted mustache) forms.
    /// Svelte's parser accepts quoted expressions in directives; prettier strips the quotes.
    fn parse_directive_expression(&mut self) -> Result<(Expression<'arena>, Span), ParseError> {
        self.check_directive_value_placement()?;
        if self.check(TokenKind::LeftBrace) {
            // Standard form: {expr}
            let expr_tag = self.parse_expression_tag()?;
            Ok((expr_tag.expression, expr_tag.span))
        } else if self.check(TokenKind::String) {
            // Quoted mustache form: "{expr}" — must be exactly one ExpressionTag
            // with no text parts. Popping the lone element extracts it owned.
            let mut parts = self.parse_attribute_value()?;
            if parts.len() == 1
                && let Some(AttributeValue::ExpressionTag(expr_tag)) = parts.pop()
            {
                Ok((expr_tag.expression, expr_tag.span))
            } else {
                Err(self.error_msg(
                    "Quoted directive value must contain a single expression, e.g. \"{expr}\"",
                ))
            }
        } else {
            Err(self.error_msg("Directive value must be an expression wrapped in {}"))
        }
    }

    /// Create an identifier expression for shorthand directives (bind:value, class:class1)
    fn make_shorthand_identifier(
        &self,
        name: &str,
        start: usize,
        end: usize,
    ) -> Expression<'arena> {
        let span = Span {
            start: start as u32,
            end: end as u32,
        };
        Expression::Identifier(Identifier::simple(
            self.synthesized_ident_name(name, span),
            span,
        ))
    }

    /// Name channel for a synthesized TS `Identifier` covering `span`:
    /// span-identity when the name is exactly the source slice — which every
    /// caller's span is built to be — else the decoded name arena-copied as the
    /// `&'arena str` escape hatch, so a future caller whose name isn't a verbatim
    /// run can't silently emit the wrong text.
    fn synthesized_ident_name(&self, name: &str, span: Span) -> IdentName<'arena> {
        let slice = &self.source[span.start as usize..span.end as usize];
        if slice == name && u16::try_from(name.len()).is_ok() {
            IdentName {
                escaped: None,
                raw_len: name.len() as u16,
            }
        } else {
            IdentName {
                escaped: Some(self.alloc_str_in(name)),
                raw_len: 0,
            }
        }
    }

    /// Parse a style directive (style:property={value} or style:property="value")
    /// Style directives can have expression values OR string values
    fn parse_style_directive(
        &mut self,
        name_span: Span,
        modifiers: &'arena [&'arena str],
        start: usize,
        name_end: usize,
        head_span: Span,
    ) -> Result<AttributeNode<'arena>, ParseError> {
        // Check for = (directive with value)
        let value = if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            self.check_directive_value_placement()?;
            // Style directive can have either expression {value} or string "value"
            if self.check(TokenKind::LeftBrace) {
                let expr_tag = self.parse_expression_tag()?;
                StyleDirectiveValue::ExpressionTag(expr_tag)
            } else if self.check(TokenKind::String) {
                // Parse string value like "red" or quoted mustache like "{value}"
                let mut parts = self.parse_attribute_value()?;
                // A lone quoted mustache "{expr}" becomes an ExpressionTag (quotes
                // stripped); any other shape (text, multiple parts) stays as raw
                // value parts. Pop the last element to test/extract it owned.
                match parts.pop() {
                    Some(AttributeValue::ExpressionTag(expr_tag)) if parts.is_empty() => {
                        StyleDirectiveValue::ExpressionTag(expr_tag)
                    }
                    popped => {
                        // Not a lone ExpressionTag: restore the popped part and keep
                        // everything as raw value parts.
                        if let Some(part) = popped {
                            parts.push(part);
                        }
                        StyleDirectiveValue::Parts(parts.into_bump_slice())
                    }
                }
            } else if self.check(TokenKind::Identifier) {
                // Unquoted value: style:background=green
                let parts = self.parse_unquoted_attribute_value()?;
                StyleDirectiveValue::Parts(parts.into_bump_slice())
            } else {
                return Err(
                    self.error_msg("Style directive value must be an expression or quoted string")
                );
            }
        } else {
            // Shorthand: style:color (no value, uses variable with same name)
            StyleDirectiveValue::True
        };

        // Calculate end position
        // For ExpressionTag from quoted mustache ("{expr}"), skip past the closing quote
        let end = match &value {
            StyleDirectiveValue::ExpressionTag(et) => {
                self.end_past_optional_quote(et.span.end_usize())
            }
            StyleDirectiveValue::Parts(parts) => parts.last().map_or(name_end, |p| match p {
                AttributeValue::Text(t) => self.end_past_optional_quote(t.span.end_usize()),
                AttributeValue::ExpressionTag(et) => {
                    self.end_past_optional_quote(et.span.end_usize())
                }
            }),
            StyleDirectiveValue::True => name_end,
        };

        let span = Span {
            start: start as u32,
            end: end as u32,
        };

        Ok(AttributeNode::StyleDirective(StyleDirective {
            name_span,
            value,
            modifiers,
            span,
            head_span,
        }))
    }

    /// Parse an {@attach expr} tag inside element attributes
    ///
    /// Syntax: {@attach expression}
    ///
    /// The expression can be:
    /// - An identifier: {@attach fn}
    /// - A call expression: {@attach tooltip("hi")}
    /// - A conditional: {@attach a ? fn1 : fn2}
    /// - An arrow function: {@attach (el) => el.focus()}
    pub(crate) fn parse_attach_tag(&mut self) -> Result<AttachTag<'arena>, ParseError> {
        let start = self.current_start;

        // We're at '{@', scan forward to find the closing '}'
        // The content is: {@attach expr}
        let brace_start = self.current_start;

        // Svelte's `read_attribute` runs `allow_whitespace()` after `eat('{')` before it tries
        // `eat('@attach')`, so the marker need not be glued: `brace_start + 2` read the
        // author's space as the keyword's first byte and `<div { @attach fn}>` — which
        // prettier formats — came back `Expected 'attach' keyword`. The sibling
        // `{...spread}` / `{shorthand}` split already skips the gap, via
        // `peek_char_after_brace`; this arm was the one re-deriving the offset by hand.
        let marker_pos = brace_interior_start(self.source, brace_start);
        let content_start = marker_pos + 1; // past the `@`

        // Find the matching closing `}` (skips strings/comments/regex).
        let Some(content_end) = scan_to_matching_brace(self.source.as_bytes(), content_start)
        else {
            return Err(self.error_unclosed_at("{@attach} tag", start));
        };
        let end = content_end + 1; // Include the closing '}'

        // Extract content: "attach expr"
        let content = &self.source[content_start..content_end];

        // Parse: "attach expr". The keyword must be followed by whitespace — Svelte's
        // `require_whitespace()`, which accepts any of it, so a newline or a tab
        // separates the keyword from its expression just as a space does.
        //
        // Deliberately NOT `strip_keyword_value` (the `{@…}` tags' shared spelling of this
        // rule): those reach their parser through a keyword dispatch that has already proven
        // the keyword, so the helper can report a missing space specifically. This reader is
        // dispatched on a bare `{@`, so a wrong keyword (`<div {@html x}>`) arrives here too
        // and the two failures share one error.
        let Some(after_attach) = content
            .strip_prefix("attach")
            .filter(|rest| rest.starts_with(is_svelte_ws))
        else {
            return Err(self.error_expected_at("'attach' keyword", content_start));
        };
        let expr_str = after_attach.trim_matches(is_svelte_ws);

        if expr_str.is_empty() {
            return Err(self.error_msg_at("{@attach} requires an expression", content_start));
        }

        // Calculate the offset of the expression in the source
        let expr_offset = content_start + super::subslice_offset(content, expr_str);

        // Parse the expression using the TypeScript parser
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // Advance the lexer past the entire {@attach ...} construct
        // We need to update the lexer position to after the closing '}'
        self.advance_to_position(end)?;

        Ok(AttachTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse a spread attribute: {...expr}
    ///
    /// Syntax: {...expression}
    ///
    /// The expression can be:
    /// - An identifier: {...obj}
    /// - A call expression: {...getProps()}
    /// - A member expression: {...obj.nested}
    fn parse_spread_attribute(&mut self) -> Result<SpreadAttribute<'arena>, ParseError> {
        let start = self.current_start;

        // We're at '{', scan forward to find the closing '}'
        let brace_start = self.current_start;

        let content_start = brace_start + 1; // Skip "{"

        // Find the matching closing `}` (skips strings/comments/regex).
        let Some(content_end) = scan_to_matching_brace(self.source.as_bytes(), content_start)
        else {
            return Err(self.error_unclosed_at("spread attribute", start));
        };
        let end = content_end + 1; // Include the closing '}'

        // Extract content: "...expr" or " ...expr " (with whitespace)
        let content = &self.source[content_start..content_end];
        let trimmed = content.trim_start_matches(is_svelte_ws);

        // Parse: "...expr"
        let Some(after_dots) = trimmed.strip_prefix("...") else {
            return Err(self.error_expected_at("'...' in spread attribute", content_start));
        };

        if after_dots.trim_matches(is_svelte_ws).is_empty() {
            return Err(self.error_msg_at("Spread attribute requires an expression", content_start));
        }

        // The offset of the byte just past `...`. `after_dots` is passed UNTRIMMED and the TS
        // parser's lexer skips any whitespace between `...` and the expression — exactly as the
        // expression-tag reader (`parse_expression_tag_at`) does — so the expression's span lands
        // on its real first token, not the intervening whitespace. Trimming the string here while
        // leaving the offset at `...` shifted every span by the gap width, so a span-identity
        // identifier re-sliced the wrong bytes and dropped the expression (`{...\n\nb}` → `{...\n}`).
        let leading_ws = content.len() - trimmed.len();
        let expr_offset = content_start + leading_ws + "...".len();

        // Parse the expression using the TypeScript parser
        let expression = self.parse_ts_expression(after_dots, expr_offset)?;

        // Advance the lexer past the entire {...} construct
        self.advance_to_position(end)?;

        Ok(SpreadAttribute {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// Parse a shorthand attribute: {name}
    ///
    /// Syntax: {identifier}
    /// Equivalent to: name={name}
    ///
    /// The content must be a valid identifier.
    fn parse_shorthand_attribute(&mut self) -> Result<Attribute<'arena>, ParseError> {
        let start = self.current_start;

        // We're at '{', scan forward to find the closing '}'
        let brace_start = self.current_start;

        // Find the closing brace
        let content_start = brace_start + 1; // Skip "{"
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        // For shorthand, we don't expect nested braces - just find the closing one
        while pos < self.source.len() && source_bytes[pos] != b'}' {
            pos += 1;
        }

        if pos >= self.source.len() {
            return Err(self.error_unclosed_at("shorthand attribute", start));
        }

        // pos is now at the closing '}'
        let content_end = pos;
        let end = pos + 1; // Include the closing '}'

        // Extract content: the identifier name
        let interior = &self.source[content_start..content_end];
        let name_str = interior.trim_matches(is_svelte_ws);

        if name_str.is_empty() {
            return Err(
                self.error_msg_at("Shorthand attribute requires an identifier", content_start)
            );
        }

        // Every span below is the identifier's own — the braces and any padding are outside
        // all of them. Svelte eats `{`, runs `allow_whitespace()`, then `read_identifier()`,
        // and threads that identifier's position through as the `ExpressionTag`'s span
        // (`{start: id.start, end: id.end}`) and the attribute's `name_loc`
        // (`create_attribute(id.name, id.loc, …)`). Pinned by
        // `tests/svelte_shorthand_attribute_span.rs` — `format` normalizes `{ x }` → `{x}`,
        // so no fixture can hold the padded trigger.
        let ident_start =
            content_start + (interior.len() - interior.trim_start_matches(is_svelte_ws).len());

        // The interior IS a `read_identifier` position, so it takes that reader rather than
        // a local character test: `{123}` / `{1a}` / `{²}` read an empty name (canonical's
        // "Attribute shorthand cannot be empty") and `{this}` is a reserved word. The local
        // test this replaced spelled the class as `is_alphanumeric() || '_' || '$'`, which
        // is neither `ID_Start` nor `ID_Continue` and so missed in BOTH directions — `{℘}`
        // rejected where canonical accepts, `{a²}` accepted where canonical stops the
        // identifier at the `²` and then fails to eat `}`.
        let name = match self.read_identifier(name_str, ident_start)? {
            Some(name) if name.len() == name_str.len() => name,
            // A name shorter than the interior is trailing junk canonical's `eat('}', true)`
            // rejects; no name at all is its empty-shorthand error. One message for both —
            // the interior is what the author must fix either way.
            _ => {
                return Err(self.error_msg_at(
                    &format!("Invalid shorthand attribute: '{name_str}'"),
                    content_start,
                ));
            }
        };

        let ident_span = Span {
            start: ident_start as u32,
            end: (ident_start + name.len()) as u32,
        };
        let identifier =
            Identifier::simple(self.synthesized_ident_name(name, ident_span), ident_span);

        let expression_tag = ExpressionTag {
            expression: Expression::Identifier(identifier),
            span: ident_span,
        };

        // Advance the lexer past the entire {name} construct
        self.advance_to_position(end)?;

        let mut value_vec = self.bvec();
        value_vec.push(AttributeValue::ExpressionTag(expression_tag));

        Ok(Attribute {
            value: Some(value_vec.into_bump_slice()),
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            name_span: ident_span,
        })
    }

    fn parse_attribute_inner(
        &mut self,
        name_start: usize,
        name_end: usize,
        reader: AttributeReader,
    ) -> Result<Attribute<'arena>, ParseError> {
        // The name was already read as a Svelte `read_tag` run by the caller; it starts at
        // an Identifier token but may extend past it over special chars (`a%b`). The name is
        // span-identity (`source[name_span]`); just resync the lexer past it.
        let start = name_start;
        self.advance_past_name(name_end)?;

        if reader == AttributeReader::Static {
            return self.parse_static_attribute_tail(start, name_end);
        }

        // Check for = (attribute with value)
        if self.check(TokenKind::Equals) {
            self.advance()?; // consume =

            // Parse attribute value (string or expression)
            let value = self.parse_attribute_value()?;

            // Find the end position from the last value part
            let value_end = if let Some(last_part) = value.last() {
                match last_part {
                    AttributeValue::Text(text) => {
                        // For quoted strings, Text span covers content only (without quotes),
                        // so skip past the closing quote. For unquoted values, the span
                        // already covers the full value (no quote to skip).
                        self.end_past_optional_quote(text.span.end_usize())
                    }
                    AttributeValue::ExpressionTag(tag) => {
                        self.end_past_optional_quote(tag.span.end_usize())
                    }
                }
            } else {
                return Err(self.error_msg("Attribute value is empty"));
            };

            Ok(Attribute {
                value: Some(value.into_bump_slice()),
                span: Span {
                    start: start as u32,
                    end: value_end as u32,
                },
                name_span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
            })
        } else {
            // Boolean attribute (no value) - ends where the name ends
            Ok(Attribute {
                value: None,
                span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
                name_span: Span {
                    start: start as u32,
                    end: name_end as u32,
                },
            })
        }
    }

    /// Finish a [`AttributeReader::Static`] attribute once its name run is read — Svelte's
    /// `read_static_attribute` minus the name (`1-parse/state/element.js`).
    ///
    /// The `=` must abut the name: the static reader runs no `allow_whitespace()` between the
    /// two, so a gap is not a value separator at all — the attribute is boolean and the `=`
    /// is left where the caller's `parser.eat('>', true)` will reject it. (The element reader
    /// *does* allow the gap, which is why `<div a = "b">` is fine and `<script a = "b">` is
    /// not.) Reading the gap as a separator here was how a mangled tag head swallowed the
    /// script body: `<script lang="ts"}⏎type T = {…>` re-emitted as `type T="{"`, whose `{`
    /// reopens as an expression and runs unterminated.
    fn parse_static_attribute_tail(
        &mut self,
        start: usize,
        name_end: usize,
    ) -> Result<Attribute<'arena>, ParseError> {
        let name_span = Span {
            start: start as u32,
            end: name_end as u32,
        };
        if !(self.check(TokenKind::Equals) && self.current_start == name_end) {
            return Ok(Attribute {
                value: None,
                span: name_span,
                name_span,
            });
        }

        let (text, value_end) = self.read_static_attribute_value(name_end + 1)?;
        let mut parts = self.bvec();
        parts.push(AttributeValue::Text(text));
        self.advance_to_position(value_end)?;

        Ok(Attribute {
            value: Some(parts.into_bump_slice()),
            span: Span {
                start: start as u32,
                end: value_end as u32,
            },
            name_span,
        })
    }

    /// Read a static attribute's raw value starting at `eq_end` (just past the `=`), returning
    /// the value `Text` and the offset the attribute ends at.
    ///
    /// Svelte allows whitespace here (`parser.eat('=')` is followed by `allow_whitespace()`),
    /// then matches `regex_attribute_value` — `"([^"]*)"`, `'([^']*)'`, or `[^>\s]+`, in that
    /// order. Only the last alternative is shared with the element reader's terminator set, and
    /// it is far laxer: `<`, `` ` ``, `'` and `=` are all ordinary value characters here
    /// (`<script a=<b>` is a value of `<b`), and no `{` is an expression.
    ///
    /// The one divergence: on an unterminated quote neither quoted alternative matches, and
    /// Svelte's third one takes the run (`"b`) and then decides it was quoted from its FIRST
    /// character alone, slicing a character off each end — so `<script a="b>` silently loses
    /// the `b`. tsv rejects instead; see `conformance_svelte.md` §Static Attribute Reader
    /// Corrections.
    fn read_static_attribute_value(&self, eq_end: usize) -> Result<(Text, usize), ParseError> {
        let source = self.source;
        let bytes = source.as_bytes();
        let value_start = skip_svelte_ws(source, eq_end);

        let text_of = |content: Span, raw_end: usize| {
            (
                Text::new(content, TextDecoding::AttributeValue, content, source),
                raw_end,
            )
        };

        if let Some(quote @ (b'"' | b'\'')) = bytes.get(value_start).copied() {
            let content_start = value_start + 1;
            let Some(offset) = bytes[content_start..].iter().position(|&b| b == quote) else {
                return Err(
                    self.error_msg_at("Unterminated string literal in template", value_start)
                );
            };
            let content_end = content_start + offset;
            return Ok(text_of(
                Span {
                    start: content_start as u32,
                    end: content_end as u32,
                },
                content_end + 1,
            ));
        }

        // `[^>\s]+`
        let end = name_run_end(source, value_start, |c| c == '>' || is_svelte_ws(c));
        if end == value_start {
            return Err(self.error_msg_at("Expected attribute value", value_start));
        }
        Ok(text_of(
            Span {
                start: value_start as u32,
                end: end as u32,
            },
            end,
        ))
    }

    /// Parse attribute value (e.g., `"ts"`, `{expr}`, or unquoted `value`)
    /// Returns a `Vec<AttributeValue>` to support mixed text/expressions
    pub(crate) fn parse_attribute_value(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeValue<'arena>>, ParseError> {
        // Any value not starting with a quote is unquoted, read as a Svelte
        // `read_sequence` — a run of Text + {expr} chunks to the terminator regex.
        // Covers a bare identifier (`data-attr=value`), a single expression
        // (`prop={a}`), concatenations (`prop={a}{b}`, `src={a}//cdn`), and
        // slash-led paths (`href=/path`).
        if !self.check(TokenKind::String) {
            return self.parse_unquoted_attribute_value();
        }

        let mut parts = self.bvec();

        // Extract string content (without quotes)
        let (token_start, token_end) = self.current_pos();

        // Remove quotes: "ts" -> ts
        let content_start = token_start + 1;
        let content_end = token_end - 1;

        // Advance past the string token now, before we start parsing expression tags
        self.advance()?;

        // Scan the quoted value as a sequence of Text and {expr} chunks. Each
        // `{expr}` goes through `parse_sequence_expression_tag_at` — the placement guard
        // plus the shared `parse_expression_tag_at`, which skips nested braces, strings,
        // comments, and regex literals — so a `}` inside one (`"{/* } */ x}"`,
        // `"{f(/[}]/)}"`) doesn't desync brace matching.
        // Example: "delete {'\"'}" contains text "delete " and expression {'\"'}.
        let mut pos = content_start;
        let source_bytes = self.source.as_bytes();

        while pos < content_end {
            // Accumulate text up to the next `{`.
            let text_start = pos;
            while pos < content_end && source_bytes[pos] != b'{' {
                pos += 1;
            }
            if pos > text_start {
                let span = Span {
                    start: text_start as u32,
                    end: pos as u32,
                };
                parts.push(AttributeValue::Text(Text::new(
                    span,
                    TextDecoding::AttributeValue,
                    span,
                    self.source,
                )));
            }

            if pos < content_end && source_bytes[pos] == b'{' {
                let tag =
                    self.parse_sequence_expression_tag_at(pos, SequenceLocation::AttributeValue)?;
                pos = tag.span.end as usize;
                parts.push(AttributeValue::ExpressionTag(tag));
            }
        }

        // If no parts were created (empty string or quote mismatch), create empty text.
        // `raw` is empty here even when the node span covers a stray byte (e.g. a
        // literal `{`), so `raw_span` is an empty span, not the node span.
        if parts.is_empty() {
            parts.push(AttributeValue::Text(Text::new(
                Span {
                    start: content_start as u32,
                    end: content_start as u32,
                },
                TextDecoding::AttributeValue,
                Span {
                    start: content_start as u32,
                    end: content_end as u32,
                },
                self.source,
            )));
        }

        Ok(parts)
    }

    /// Parse an unquoted attribute value as a Svelte `read_sequence`.
    ///
    /// An unquoted value is a run of `Text` and `{expr}` chunks terminated by
    /// `regex_invalid_unquoted_attribute_value` — `/>` or one of whitespace, `"`,
    /// `'`, `=`, `<`, `>`, `` ` ``. So `prop={a}{b}` is one value `[{a}, {b}]`,
    /// `src={a}//cdn` is `[{a}, "//cdn"]`, and `href=/path` is `["/path"]`. A bare
    /// `/` (only `/>`) does not terminate, so protocol-relative and root-relative
    /// URLs read as plain text. The `/>` terminator is suppressed at the value
    /// start, matching Svelte: `href=/>` reads the leading `/` as the value (`/`)
    /// and lets the `>` close the tag, rather than self-closing on an empty value.
    ///
    /// We scan raw bytes because the lexer's identifier token doesn't span `/`,
    /// `:`, and the like. `Text` chunks decode with attribute-context rules to
    /// match Svelte (`decode_character_references(raw, true)`).
    ///
    /// The [`AttributeReader::Static`] twin of this is
    /// [`read_static_attribute_value`](Self::read_static_attribute_value), whose terminator
    /// set is far laxer and whose `{` is never an expression.
    pub(crate) fn parse_unquoted_attribute_value(
        &mut self,
    ) -> Result<BumpVec<'arena, AttributeValue<'arena>>, ParseError> {
        // `src`/`bytes` borrow the source data (lifetime `'a`), so they stay valid
        // across the `&mut self` `parse_expression_tag_at` call below.
        let src = self.source;
        let bytes = src.as_bytes();
        let start = self.current_start;
        let mut parts: BumpVec<'arena, AttributeValue<'arena>> = self.bvec();
        let mut text_start = start;
        let mut pos = start;

        let flush_text =
            |parts: &mut BumpVec<'arena, AttributeValue<'arena>>, from: usize, to: usize| {
                if to > from {
                    let span = Span {
                        start: from as u32,
                        end: to as u32,
                    };
                    parts.push(AttributeValue::Text(Text::new(
                        span,
                        TextDecoding::AttributeValue,
                        span,
                        src,
                    )));
                }
            };

        loop {
            // Terminator regex: `/>` or one of `\s` " ' = < > `
            // (`/>` only past the value start — a leading `/` is value, not close).
            //
            // ⚠️ A `char` question, not a byte one, exactly like `is_attr_name_terminator`
            // and the static twin's `[^>\s]+` run: the `\s` arm is Unicode
            // ([`is_svelte_ws`]), so an ASCII-byte spelling
            // (`b' ' | b'\t' | b'\n' | b'\r' | b'\x0C'`) is too NARROW by twenty of the
            // class's twenty-five code points — every non-ASCII member, plus the VT it
            // spelled its ASCII half without. One fell to the non-terminator arm and was absorbed
            // into the value — which both changed the wire (`value` became an array where
            // canonical ends the attribute) and made the printer re-emit an expression
            // attribute as a QUOTED one, output `svelte compile` rejects. Stepping by
            // `width` is what keeps `char_at` on a character boundary.
            let Some((c, width)) = char_at(src, pos) else {
                flush_text(&mut parts, text_start, pos);
                break;
            };
            let terminated = match c {
                '/' => pos > start && bytes.get(pos + 1) == Some(&b'>'),
                '"' | '\'' | '=' | '<' | '>' | '`' => true,
                _ => is_svelte_ws(c),
            };
            if terminated {
                flush_text(&mut parts, text_start, pos);
                break;
            }

            // An `{expr}` chunk starts a new part.
            if c == '{' {
                flush_text(&mut parts, text_start, pos);
                // Parse the `{expr}` without disturbing the lexer (it handles nested
                // braces, strings, comments, and regex that a raw byte scan cannot);
                // we own the cursor and sync the lexer once below.
                let tag =
                    self.parse_sequence_expression_tag_at(pos, SequenceLocation::AttributeValue)?;
                pos = tag.span.end as usize;
                text_start = pos;
                parts.push(AttributeValue::ExpressionTag(tag));
                continue;
            }

            pos += width;
        }

        if pos == start {
            return Err(self.error_msg("Expected attribute value"));
        }

        // Sync the lexer to the value terminator for the element parser. The loop
        // never touched the lexer, so `inside_tag` is still set (we're inside the
        // tag) and `advance_to_position` re-lexes the terminator in tag mode.
        self.advance_to_position(pos)?;

        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::attr_name_end;

    /// The attribute-name run ends at `[\s=/>"']` and at nothing else. Wider than the
    /// tag-name class ([`is_tag_name_terminator`](super::super::element)), which keeps `=`,
    /// `"` and `'` *inside* the name — two questions, two predicates, and collapsing them
    /// would break one of the two.
    ///
    /// Each case is `(name, tail)`: the expected end is `name.len()`, so a position is built
    /// from a prefix length rather than searched for.
    #[test]
    fn attr_name_run_ends_at_the_token_ending_characters() {
        for (name, tail) in [
            // every terminator, and EOF
            ("data-attr", "=\"value\""),
            ("data-attr", " x"),
            ("data-attr", "/>"),
            ("data-attr", ">"),
            ("data-attr", "\"value\""),
            ("data-attr", "'value'"),
            ("data-attr", ""),
            // the whitespace arm is JS `\s`, so it is Unicode-wide
            ("data-attr", "\u{a0}x"),
            ("data-attr", "\u{feff}x"),
            // U+0085 NEL is not JS `\s`, so it stays inside the name
            ("data-attr\u{85}x", "="),
            // symbols Svelte folds into the name, and a directive's whole head token
            ("ysc%%gibberish", "="),
            ("on:click|preventDefault", "={fn}"),
        ] {
            let source = format!("{name}{tail}");
            assert_eq!(attr_name_end(&source, 0), name.len(), "{source:?}");
        }
    }
}
