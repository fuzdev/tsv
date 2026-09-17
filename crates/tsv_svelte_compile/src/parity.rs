//! Comment-position-tolerant parity between a compiled canonical JS string and
//! the oracle's canonical JS string.
//!
//! The compiler's parity bar is the canonical reprint (`canonicalize_js`), but a
//! byte-exact bar over-constrains **comment position**: tsv preserves the author's
//! comment placement (its comment philosophy — a deliberate, cataloged divergence
//! from prettier), while Svelte's printer (esrap) relocates comments across
//! operator/conditional boundaries the way prettier does. The two then place the
//! *same* comment on *different* AST nodes — genuinely different bytes, but not a
//! difference in the compiled **code**.
//!
//! Comment position in compiled (machine-consumed) output carries no correctness
//! signal, so pinning it flags cosmetic differences as bugs. This comparator
//! relaxes the bar to what actually matters: **the code and the comment sequence
//! must match exactly; only comment position may differ.** That keeps every real
//! signal — a dropped, doubled, reordered, or content-changed comment is still a
//! [`Parity::Divergent`] — while tolerating tsv's own comment placement.
//!
//! Runs **only on the failure path** (the callers try byte-equality first), so its
//! two extra parses + reprints never touch the common case.
//!
//! ## The one hazard: semantic comments
//!
//! A **bundler annotation** (`/* @__PURE__ */`, `/* @__NO_SIDE_EFFECTS__ */`, a
//! webpack/vite magic comment) is *not* position-neutral — moving it changes
//! tree-shaking / bundling. So if either side carries one, the comparator requires
//! byte-exactness (never tolerates), a conservative fall-back to the strict bar.
//! JSDoc casts are NOT a hazard here: the compiler's erase pass unwraps every
//! `JsdocCast` to its inner expression, so a cast survives into the emitted program
//! as a plain comment with no node to bind — cosmetic, like any other comment.
//!
//! ## JSDoc-cast parens are comment position
//!
//! The oracle's printer (esrap) re-adds the parens acorn strips from a JSDoc cast by
//! guessing: a `/** @type {…} */` comment its per-node hook flushes before an
//! expression gets `(`…`)` around that expression. Whether the hook is what flushes
//! the comment depends on source LAYOUT, not code — a comment starting on the line an
//! array element, argument or statement ended on is written as that item's trailing
//! comment first, and one before an arrow's expression body is flushed into the
//! parameter list — so `f(a, /** @type {T} */ b)` gets no parens on one line and
//! `(b)` with `b` on its own. The parens are a function of where the comment sat, so
//! the code comparison erases every `JsdocCast` on both sides and they compare as
//! comment position.

use bumpalo::Bump;
use tsv_ts::Goal;

/// The outcome of comparing two canonical JS strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    /// Byte-identical.
    Exact,
    /// Differ only in comment *position* — same code, same comment sequence, no
    /// bundler annotation involved. Tolerated as parity (tsv's comment placement).
    CommentPosition,
    /// A real difference: the code differs, the comment sequence differs (a drop /
    /// double-print / reorder / content change), or a bundler annotation is present.
    Divergent,
}

impl Parity {
    /// Whether this outcome counts as parity (exact or comment-position-tolerated).
    #[inline]
    pub fn is_parity(self) -> bool {
        matches!(self, Parity::Exact | Parity::CommentPosition)
    }
}

/// Compare `ours` against `oracle` (both already canonical JS) tolerating only
/// comment-position differences. See the module docs.
pub fn compare_canonical(ours: &str, oracle: &str) -> Parity {
    if ours == oracle {
        return Parity::Exact;
    }
    // Both strings are already canonical, so they parse; a parse failure here is a
    // genuine divergence (a corrupt reprint), not a comment-position difference.
    let ours_arena = Bump::new();
    let oracle_arena = Bump::new();
    let (Ok(mut ours_program), Ok(mut oracle_program)) = (
        tsv_ts::parse_with_goal(ours, Goal::Module, &ours_arena),
        tsv_ts::parse_with_goal(oracle, Goal::Module, &oracle_arena),
    ) else {
        return Parity::Divergent;
    };

    // A bundler annotation's placement is semantic (tree-shaking / bundling), so it
    // is NOT position-neutral. If either side carries one, only byte-exactness (which
    // already failed) counts — fall back to the strict bar.
    let has_annotation = ours_program
        .comments
        .iter()
        .any(|c| is_bundler_annotation(c.content(ours)))
        || oracle_program
            .comments
            .iter()
            .any(|c| is_bundler_annotation(c.content(oracle)));
    if has_annotation {
        return Parity::Divergent;
    }

    // The comment SEQUENCE (output order, exact content) must match — a dropped,
    // doubled, reordered, or content-changed comment is a real difference, not
    // position. (Comparing sequences, not a multiset, so a reorder is caught.)
    let ours_comments: Vec<&str> = ours_program
        .comments
        .iter()
        .map(|c| c.content(ours))
        .collect();
    let oracle_comments: Vec<&str> = oracle_program
        .comments
        .iter()
        .map(|c| c.content(oracle))
        .collect();
    if ours_comments != oracle_comments {
        return Parity::Divergent;
    }

    // The CODE must be identical: reprint both with comments cleared and byte-compare.
    // A comment-forced line break vanishes with its comment, so two same-code programs
    // reprint identically here regardless of where their comments sat. A JSDoc cast's
    // parens go with it (see the module docs): the erase pass unwraps every `JsdocCast`.
    ours_program.comments = &[];
    oracle_program.comments = &[];
    ours_program.body = without_jsdoc_casts(&ours_arena, ours, ours_program.body);
    oracle_program.body = without_jsdoc_casts(&oracle_arena, oracle, oracle_program.body);
    let ours_code = tsv_ts::format_canonical(&ours_program, ours);
    let oracle_code = tsv_ts::format_canonical(&oracle_program, oracle);
    if ours_code == oracle_code {
        Parity::CommentPosition
    } else {
        Parity::Divergent
    }
}

/// `body` with every `JsdocCast` unwrapped to its inner expression, through the
/// compiler's erase pass. Canonical JS carries no TypeScript for it to erase, so a
/// refusal cannot come from a cast; should one come from anything else, the body is
/// compared as it stands.
fn without_jsdoc_casts<'arena>(
    arena: &'arena Bump,
    source: &str,
    body: &'arena [tsv_ts::ast::internal::Statement<'arena>],
) -> &'arena [tsv_ts::ast::internal::Statement<'arena>] {
    crate::erase::erase_statements(arena, source, body).map_or(body, |erased| erased.body)
}

/// A bundler annotation whose placement is semantic (moving it changes
/// tree-shaking / bundling), so it must never be treated as position-neutral. The
/// tree-shaking hints (`@__PURE__`, `@__NO_SIDE_EFFECTS__`, and the `#`-spelled
/// variants) plus the webpack/vite magic-comment markers. Conservative by design —
/// a false positive only costs this one comparison its position tolerance.
fn is_bundler_annotation(content: &str) -> bool {
    let content = content.trim();
    content.contains("__PURE__")
        || content.contains("__NO_SIDE_EFFECTS__")
        || content.contains("@vite-")
        || content.contains("webpack")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert_eq!(
            compare_canonical("let a = 1;\n", "let a = 1;\n"),
            Parity::Exact
        );
    }

    #[test]
    fn comment_position_only_is_tolerated() {
        // Same ternary code, the comment on the test vs leading the consequent — the
        // earbetter shape. Same code, same one comment → tolerated.
        let ours = "let x = a // c\n\t? b\n\t: d;\n";
        let oracle = "let x = a\n\t? // c\n\t\tb\n\t: d;\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::CommentPosition);
    }

    #[test]
    fn dropped_comment_is_divergent() {
        let ours = "let a = 1;\nlet b = 2;\n";
        let oracle = "let a = 1;\n// note\nlet b = 2;\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::Divergent);
    }

    #[test]
    fn doubled_comment_is_divergent() {
        // The double-print class the fictional-span fix guards against — a comment
        // count difference, NOT position. Must be flagged.
        let ours = "// note\nlet a = 1;\n// note\nlet b = 2;\n";
        let oracle = "let a = 1;\n// note\nlet b = 2;\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::Divergent);
    }

    #[test]
    fn content_change_is_divergent() {
        let ours = "let a = 1; // one\n";
        let oracle = "let a = 1; // two\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::Divergent);
    }

    #[test]
    fn code_difference_is_divergent() {
        // Same comment, different code (a trailing space in a string — the dealt shape).
        let ours = "let a = `x `; // c\n";
        let oracle = "let a = `x`; // c\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::Divergent);
    }

    #[test]
    fn jsdoc_cast_parens_are_comment_position() {
        // esrap's guessed cast parens, present on one side only.
        let ours = "const a = /** @type {number} */ b;\n";
        let oracle = "const a = /** @type {number} */ (b);\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::CommentPosition);
        // The parens are transparent, the code inside them is not.
        let oracle_other = "const a = /** @type {number} */ (c);\n";
        assert_eq!(compare_canonical(ours, oracle_other), Parity::Divergent);
        // A cast comment that only one side carries is still a comment difference.
        assert_eq!(
            compare_canonical("const a = b;\n", oracle),
            Parity::Divergent
        );
    }

    #[test]
    fn bundler_annotation_falls_back_to_strict() {
        // A repositioned `@__PURE__` changes tree-shaking, so an annotation present +
        // not byte-exact is Divergent even if code + comment sequence match.
        let ours = "let a = /* @__PURE__ */ f();\n";
        let oracle = "let a = /* @__PURE__ */ f();\n";
        // (identical here → Exact; the guard only matters when they differ)
        assert_eq!(compare_canonical(ours, oracle), Parity::Exact);
        let ours_moved = "let a = /* @__PURE__ */ f(); // c\n";
        let oracle_moved = "// c\nlet a = /* @__PURE__ */ f();\n";
        assert_eq!(
            compare_canonical(ours_moved, oracle_moved),
            Parity::Divergent
        );
    }

    #[test]
    fn reordered_comments_are_divergent() {
        let ours = "// a\n// b\nlet x = 1;\n";
        let oracle = "// b\n// a\nlet x = 1;\n";
        assert_eq!(compare_canonical(ours, oracle), Parity::Divergent);
    }
}
