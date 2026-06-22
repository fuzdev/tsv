// Paren-free @type comment is not a cast: no parens added
const a = /** @type {A} */ expr;

// Non-@type block comment is not a cast: redundant parens are stripped
const b = /* grouping */ (expr);

// Inner needs parens regardless, so the cast keeps a single pair (no double-wrap)
function f() {
	return /** @type {T} */ (x = y);
}
