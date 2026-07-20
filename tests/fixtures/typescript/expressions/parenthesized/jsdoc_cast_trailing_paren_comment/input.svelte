<script>
	// A comment between a JSDoc cast's inner expression and its closing `)`. The cast
	// builder synthesizes its own parens, so it has to emit this gap itself — nothing
	// else can, and a comment left unclaimed here is silently dropped. A block comment
	// trails the inner inline; a line comment must break, or it swallows the `)`.

	// block comment trails the inner
	const a = /** @type {T} */ (inner /* c1 */);

	// object inner (the hug path) keeps the comment too
	const b = /** @type {T} */ ({ a: 1 } /* c1 */);

	// array inner
	const c = /** @type {T} */ ([1, 2] /* c1 */);

	// two block comments stay in order on one line
	const d = /** @type {T} */ (inner /* c1 */ /* c2 */);

	// a line comment forces the broken layout so it can't swallow the `)`
	const e = /** @type {T} */ (
		inner // c1
	);

	// both gaps at once — leading hugs the inner, trailing follows it
	const f = /** @type {T} */ (/* c1 */ inner /* c2 */);

	// a nested cast's outer gap
	const g = /** @type {A} */ (/** @type {B} */ (inner) /* c1 */);

	// CONTROL: no trailing comment
	const h = /** @type {T} */ (inner);
</script>
