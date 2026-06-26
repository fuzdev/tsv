//! Byte→char offset translation as a mutating walk over the typed public CSS AST.
//!
//! Counterpart to `translate_byte_to_char_offsets` (the `serde_json::Value`
//! walk in `convert/mod.rs`): same translation semantics, but applied to the
//! typed tree so `convert_ast_json_string` can serialize multibyte sources
//! directly — no intermediate `Value` materialization on the wire hot path.
//!
//! Parity contract: output must be byte-identical to the `Value` walk. CSS
//! public nodes carry only `start`/`end` (no `loc`/columns, unlike `tsv_ts`),
//! so each position is translated independently — order is irrelevant, and
//! every position-bearing field must be visited exactly once. A missed field
//! means silently untranslated offsets. Gates: the fixture suite's string-path
//! identity check, the CSS typed-walk parity probe (a synthesized multibyte
//! variant per `.css` fixture), and `corpus:compare:parse --multibyte-only`.

use super::super::public;

/// Byte→char offset translation over the typed standalone AST, in place.
///
/// For ASCII-only sources this is a no-op (byte == char offset).
pub fn translate_byte_to_char_offsets_typed(
    root: &mut public::StyleSheetFile,
    map: &tsv_lang::ByteToCharMap,
) {
    if !map.has_multibyte() {
        return;
    }
    let t = Translator { map };
    t.stylesheet(root);
}

struct Translator<'a> {
    map: &'a tsv_lang::ByteToCharMap,
}

impl Translator<'_> {
    #[inline]
    fn pos(&self, p: &mut u32) {
        *p = self.map.byte_to_char(*p);
    }

    fn stylesheet(&self, n: &mut public::StyleSheetFile) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        for c in &mut n.children {
            self.node(c);
        }
    }

    fn node(&self, n: &mut public::CssNodePublic) {
        match n {
            public::CssNodePublic::Rule(r) => self.rule(r),
            public::CssNodePublic::Atrule(a) => self.atrule(a),
            public::CssNodePublic::Declaration(d) => {
                self.pos(&mut d.start);
                self.pos(&mut d.end);
            }
        }
    }

    fn rule(&self, n: &mut public::Rule) {
        self.selector_list(&mut n.prelude);
        self.pos(&mut n.block.start);
        self.pos(&mut n.block.end);
        for c in &mut n.block.children {
            self.node(c);
        }
        self.pos(&mut n.start);
        self.pos(&mut n.end);
    }

    fn atrule(&self, n: &mut public::Atrule) {
        if let Some(block) = &mut n.block {
            self.pos(&mut block.start);
            self.pos(&mut block.end);
            for c in &mut block.children {
                self.node(c);
            }
        }
        self.pos(&mut n.start);
        self.pos(&mut n.end);
    }

    fn selector_list(&self, n: &mut public::SelectorList) {
        self.pos(&mut n.start);
        self.pos(&mut n.end);
        for c in &mut n.children {
            self.pos(&mut c.start);
            self.pos(&mut c.end);
            for r in &mut c.children {
                self.relative(r);
            }
        }
    }

    fn relative(&self, n: &mut public::RelativeSelector) {
        if let Some(comb) = &mut n.combinator {
            self.pos(&mut comb.start);
            self.pos(&mut comb.end);
        }
        for s in &mut n.selectors {
            self.simple(s);
        }
        self.pos(&mut n.start);
        self.pos(&mut n.end);
    }

    fn simple(&self, n: &mut public::SimpleSelector) {
        match n {
            public::SimpleSelector::Named(s) => {
                self.pos(&mut s.start);
                self.pos(&mut s.end);
            }
            public::SimpleSelector::Attribute(s) => {
                self.pos(&mut s.start);
                self.pos(&mut s.end);
            }
            public::SimpleSelector::PseudoClass(s) => {
                if let Some(args) = &mut s.args {
                    self.selector_list(args);
                }
                self.pos(&mut s.start);
                self.pos(&mut s.end);
            }
            public::SimpleSelector::PseudoElement(s) => {
                self.pos(&mut s.start);
                self.pos(&mut s.end);
            }
            public::SimpleSelector::Percentage(s) => {
                self.pos(&mut s.start);
                self.pos(&mut s.end);
            }
            public::SimpleSelector::Nth(s) => {
                if let Some(selector) = &mut s.selector {
                    self.selector_list(selector);
                }
                self.pos(&mut s.start);
                self.pos(&mut s.end);
            }
        }
    }
}
